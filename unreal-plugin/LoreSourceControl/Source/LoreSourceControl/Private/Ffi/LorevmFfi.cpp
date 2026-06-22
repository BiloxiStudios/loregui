// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "Ffi/LorevmFfi.h"
#include "Ffi/lorevm_ffi.h"
#include "LoreSourceControlLog.h"

#include "HAL/PlatformProcess.h"
#include "Misc/Paths.h"
#include "Misc/FileHelper.h"
#include "Interfaces/IPluginManager.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

// ---------------------------------------------------------------------------
// Construction / teardown
// ---------------------------------------------------------------------------

FLorevmFfi::~FLorevmFfi()
{
	Unload();
}

// ---------------------------------------------------------------------------
// Library discovery + loading
// ---------------------------------------------------------------------------

FString FLorevmFfi::DefaultLibFileName()
{
#if PLATFORM_WINDOWS
	return TEXT("lorevm_ffi.dll");
#elif PLATFORM_MAC
	return TEXT("liblorevm_ffi.dylib");
#else
	return TEXT("liblorevm_ffi.so");
#endif
}

FString FLorevmFfi::PluginBinariesCandidate()
{
	// Binaries/ThirdParty/LorevmFfi/<Platform>/<libname>, the conventional place
	// to drop a third-party shared lib in a UE plugin. Build.cs is documented to
	// stage the cdylib here as a RuntimeDependency.
	TSharedPtr<IPlugin> Plugin = IPluginManager::Get().FindPlugin(TEXT("LoreSourceControl"));
	if (!Plugin.IsValid())
	{
		return FString();
	}

	const FString BaseDir = Plugin->GetBaseDir();
#if PLATFORM_WINDOWS
	const FString PlatformDir = TEXT("Win64");
#elif PLATFORM_MAC
	const FString PlatformDir = TEXT("Mac");
#else
	const FString PlatformDir = TEXT("Linux");
#endif

	return FPaths::Combine(BaseDir, TEXT("Binaries"), TEXT("ThirdParty"),
	                       TEXT("LorevmFfi"), PlatformDir, DefaultLibFileName());
}

template <typename FnPtr>
bool FLorevmFfi::ResolveSymbol(const ANSICHAR* Name, FnPtr& OutPtr)
{
	OutPtr = reinterpret_cast<FnPtr>(FPlatformProcess::GetDllExport(LibHandle, ANSI_TO_TCHAR(Name)));
	if (OutPtr == nullptr)
	{
		UE_LOG(LogSourceControl, Error, TEXT("[Lore] lorevm-ffi: missing symbol '%s'"), ANSI_TO_TCHAR(Name));
		return false;
	}
	return true;
}

bool FLorevmFfi::Load(const FString& InExplicitLibPath, FString& OutError)
{
	if (bLoaded)
	{
		return true;
	}

	// Resolve the library path in priority order.
	TArray<FString> Candidates;
	if (!InExplicitLibPath.IsEmpty())
	{
		Candidates.Add(InExplicitLibPath);
	}
	const FString EnvPath = FPlatformMisc::GetEnvironmentVariable(TEXT("LOREVM_FFI_LIB"));
	if (!EnvPath.IsEmpty())
	{
		Candidates.Add(EnvPath);
	}
	const FString PluginCandidate = PluginBinariesCandidate();
	if (!PluginCandidate.IsEmpty())
	{
		Candidates.Add(PluginCandidate);
	}
	// Last resort: bare library name, letting the OS loader search its paths.
	Candidates.Add(DefaultLibFileName());

	FString TriedList;
	for (const FString& Candidate : Candidates)
	{
		TriedList += FString::Printf(TEXT("\n  - %s"), *Candidate);
		LibHandle = FPlatformProcess::GetDllHandle(*Candidate);
		if (LibHandle != nullptr)
		{
			UE_LOG(LogSourceControl, Log, TEXT("[Lore] lorevm-ffi loaded from %s"), *Candidate);
			break;
		}
	}

	if (LibHandle == nullptr)
	{
		OutError = FString::Printf(
			TEXT("could not load lorevm-ffi shared library. Tried:%s\nBuild it with `cargo build -p lorevm-ffi --release` and stage it under the plugin's Binaries/ThirdParty/LorevmFfi/<Platform>/, or set LOREVM_FFI_LIB."),
			*TriedList);
		return false;
	}

	// Resolve every symbol; on any failure, unload and bail.
	bool bOk = true;
	bOk &= ResolveSymbol("lorevm_ffi_open", reinterpret_cast<lorevm_ffi_open_fn&>(FnOpen));
	bOk &= ResolveSymbol("lorevm_ffi_call", reinterpret_cast<lorevm_ffi_call_fn&>(FnCall));
	bOk &= ResolveSymbol("lorevm_ffi_close", reinterpret_cast<lorevm_ffi_close_fn&>(FnClose));
	bOk &= ResolveSymbol("lorevm_ffi_string_free", reinterpret_cast<lorevm_ffi_string_free_fn&>(FnStringFree));
	bOk &= ResolveSymbol("lorevm_ffi_abi_version", reinterpret_cast<lorevm_ffi_abi_version_fn&>(FnAbiVersion));

	if (!bOk)
	{
		OutError = TEXT("lorevm-ffi loaded but one or more expected symbols are missing — version mismatch?");
		FPlatformProcess::FreeDllHandle(LibHandle);
		LibHandle = nullptr;
		return false;
	}

	// Read + check ABI version.
	{
		const char* RawAbi = reinterpret_cast<lorevm_ffi_abi_version_fn>(FnAbiVersion)();
		AbiVersion = RawAbi ? FString(ANSI_TO_TCHAR(RawAbi)) : FString();
	}

	// Expect "lorevm-ffi/<major>"; refuse a different major.
	const FString Expected = FString::Printf(TEXT("lorevm-ffi/%d"), LOREVM_FFI_ABI_MAJOR_EXPECTED);
	if (!AbiVersion.StartsWith(Expected))
	{
		OutError = FString::Printf(
			TEXT("lorevm-ffi ABI mismatch: library reports '%s', plugin expects major %d ('%s'). Rebuild one to match."),
			*AbiVersion, LOREVM_FFI_ABI_MAJOR_EXPECTED, *Expected);
		FPlatformProcess::FreeDllHandle(LibHandle);
		LibHandle = nullptr;
		return false;
	}

	bLoaded = true;
	UE_LOG(LogSourceControl, Log, TEXT("[Lore] lorevm-ffi ABI %s ready"), *AbiVersion);
	return true;
}

void FLorevmFfi::Unload()
{
	Close();
	if (LibHandle != nullptr)
	{
		FPlatformProcess::FreeDllHandle(LibHandle);
		LibHandle = nullptr;
	}
	FnOpen = FnCall = FnClose = FnStringFree = FnAbiVersion = nullptr;
	bLoaded = false;
}

// ---------------------------------------------------------------------------
// Warm-handle lifecycle
// ---------------------------------------------------------------------------

bool FLorevmFfi::Open(const FString& WorkingDir, bool bInMemory, bool bOffline, const FString& Identity, FString& OutError)
{
	if (!bLoaded)
	{
		OutError = TEXT("lorevm-ffi not loaded; call Load() first");
		return false;
	}

	// Build the open-request JSON: { "dir", "in_memory", "offline", "identity" }.
	const TSharedRef<FJsonObject> Request = MakeShared<FJsonObject>();
	Request->SetStringField(TEXT("dir"), WorkingDir);
	Request->SetBoolField(TEXT("in_memory"), bInMemory);
	Request->SetBoolField(TEXT("offline"), bOffline);
	if (!Identity.IsEmpty())
	{
		Request->SetStringField(TEXT("identity"), Identity);
	}

	FString RequestJson;
	{
		const TSharedRef<TJsonWriter<>> Writer = TJsonWriterFactory<>::Create(&RequestJson);
		FJsonSerializer::Serialize(Request, Writer);
	}

	FScopeLock Lock(&HandleCS);

	// Close any previous handle before opening a new one.
	if (Handle != nullptr)
	{
		reinterpret_cast<lorevm_ffi_close_fn>(FnClose)(Handle);
		Handle = nullptr;
	}

	const FTCHARToUTF8 RequestUtf8(*RequestJson);
	Handle = reinterpret_cast<lorevm_ffi_open_fn>(FnOpen)(RequestUtf8.Get());
	if (Handle == nullptr)
	{
		OutError = FString::Printf(
			TEXT("lorevm_ffi_open returned NULL for dir '%s' (bad request or runtime-build failure)"),
			*WorkingDir);
		return false;
	}

	UE_LOG(LogSourceControl, Log, TEXT("[Lore] warm handle open for %s"), *WorkingDir);
	return true;
}

bool FLorevmFfi::IsOpen() const
{
	FScopeLock Lock(&HandleCS);
	return Handle != nullptr;
}

void FLorevmFfi::Close()
{
	FScopeLock Lock(&HandleCS);
	if (Handle != nullptr && FnClose != nullptr)
	{
		reinterpret_cast<lorevm_ffi_close_fn>(FnClose)(Handle);
	}
	Handle = nullptr;
}

// ---------------------------------------------------------------------------
// The call path
// ---------------------------------------------------------------------------

FLorevmResult FLorevmFfi::Call(const FString& OpId, const TSharedRef<FJsonObject>& Args)
{
	FString ArgsJson;
	const TSharedRef<TJsonWriter<>> Writer = TJsonWriterFactory<>::Create(&ArgsJson);
	FJsonSerializer::Serialize(Args, Writer);
	return Call(OpId, ArgsJson);
}

FLorevmResult FLorevmFfi::Call(const FString& OpId, const FString& Args)
{
	if (!bLoaded)
	{
		return FLorevmResult::MakeError(TEXT("ffi"), TEXT("lorevm-ffi not loaded"));
	}

	// Snapshot the handle under the lock so a concurrent Close() can't free it
	// mid-call. The Rust handle is Send+Sync; the lock only guards the pointer.
	LorevmHandle* LocalHandle = nullptr;
	{
		FScopeLock Lock(&HandleCS);
		LocalHandle = Handle;
	}
	if (LocalHandle == nullptr)
	{
		return FLorevmResult::MakeError(TEXT("ffi"), TEXT("no warm handle open; call Open() first"));
	}

	const FTCHARToUTF8 OpUtf8(*OpId);
	const FTCHARToUTF8 ArgsUtf8(*Args);

	// THE HOT PATH. Blocks this (background) thread for the op's duration.
	char* Raw = reinterpret_cast<lorevm_ffi_call_fn>(FnCall)(LocalHandle, OpUtf8.Get(), ArgsUtf8.Get());
	if (Raw == nullptr)
	{
		// NULL only on a NUL/invalid-UTF-8 pointer per the ABI — should not happen
		// with the UTF-8 conversions above, but report it rather than crash.
		return FLorevmResult::MakeError(TEXT("ffi"), TEXT("lorevm_ffi_call returned NULL"));
	}

	// Copy the Rust-owned C string into an FString, then free it with the
	// matching Rust free fn (never C free). Ownership crossed the ABI to us.
	const FString ResponseJson = FString(UTF8_TO_TCHAR(Raw));
	reinterpret_cast<lorevm_ffi_string_free_fn>(FnStringFree)(Raw);

	// Parse the response. Two shapes:
	//   success -> the op's typed result object
	//   failure -> {"error":{"kind","message"}}
	TSharedPtr<FJsonObject> Root;
	const TSharedRef<TJsonReader<>> Reader = TJsonReaderFactory<>::Create(ResponseJson);
	if (!FJsonSerializer::Deserialize(Reader, Root) || !Root.IsValid())
	{
		return FLorevmResult::MakeError(TEXT("ffi"),
			FString::Printf(TEXT("non-JSON response for op '%s': %s"), *OpId, *ResponseJson));
	}

	// Error envelope?
	const TSharedPtr<FJsonObject>* ErrorObj = nullptr;
	if (Root->TryGetObjectField(TEXT("error"), ErrorObj) && ErrorObj && ErrorObj->IsValid())
	{
		FString Kind, Message;
		(*ErrorObj)->TryGetStringField(TEXT("kind"), Kind);
		(*ErrorObj)->TryGetStringField(TEXT("message"), Message);
		if (Kind.IsEmpty())
		{
			Kind = TEXT("unknown");
		}
		return FLorevmResult::MakeError(Kind, Message);
	}

	FLorevmResult Ok;
	Ok.bSuccess = true;
	Ok.Result = Root;
	return Ok;
}
