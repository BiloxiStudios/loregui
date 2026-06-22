// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlProvider.h"
#include "LoreSourceControlCommand.h"
#include "LoreSourceControlOperations.h"
#include "LoreSourceControlModule.h"
#include "LoreSourceControlLog.h"
#include "Ffi/LorevmFfi.h"

#include "SourceControlOperations.h"
#include "ISourceControlModule.h"
#include "SourceControlHelpers.h"
#include "SourceControlOperationBase.h"
#include "Misc/Paths.h"
#include "Misc/QueuedThreadPool.h"
#include "Misc/MessageDialog.h"
#include "Widgets/Text/STextBlock.h"

#define LOCTEXT_NAMESPACE "LoreSourceControl.Provider"

FLoreSourceControlProvider::FLoreSourceControlProvider()
	: Ffi(MakeUnique<FLorevmFfi>())
{
}

FLoreSourceControlProvider::~FLoreSourceControlProvider() = default;

// ---------------------------------------------------------------------------
// Worker registration / creation
// ---------------------------------------------------------------------------

void FLoreSourceControlProvider::RegisterWorker(const FName& InName, const FGetLoreSourceControlWorker& InDelegate)
{
	WorkersMap.Add(InName, InDelegate);
}

TSharedPtr<ILoreSourceControlWorker, ESPMode::ThreadSafe> FLoreSourceControlProvider::CreateWorker(const FName& InOperationName) const
{
	if (const FGetLoreSourceControlWorker* Factory = WorkersMap.Find(InOperationName))
	{
		return Factory->Execute();
	}
	return nullptr;
}

// ---------------------------------------------------------------------------
// Init / Close — bridge lifecycle
// ---------------------------------------------------------------------------

void FLoreSourceControlProvider::Init(bool /*bForceConnection*/)
{
	const FLoreSourceControlModule& Module = FLoreSourceControlModule::Get();
	const FLoreSourceControlSettings& Settings = Module.GetSettings();

	// Repository root = the project directory by default (lore "dir").
	PathToRepositoryRoot = FPaths::ConvertRelativePathToFull(FPaths::ProjectDir());
	Identity = Settings.GetIdentity();

	// 1. Load the lorevm-ffi cdylib.
	FString LoadError;
	if (!Ffi->Load(Settings.GetLorevmFfiLibPath(), LoadError))
	{
		bLoreAvailable = false;
		LastError = LoadError;
		UE_LOG(LogSourceControl, Warning, TEXT("[Lore] %s"), *LoadError);
		return;
	}

	// 2. Open the warm handle for the repository working dir.
	FString OpenError;
	if (!Ffi->Open(PathToRepositoryRoot, Settings.GetUseInMemory(), Settings.GetOffline(), Identity, OpenError))
	{
		bLoreAvailable = false;
		LastError = OpenError;
		UE_LOG(LogSourceControl, Warning, TEXT("[Lore] %s"), *OpenError);
		return;
	}

	bLoreAvailable = true;
	UE_LOG(LogSourceControl, Log, TEXT("[Lore] provider initialised (ABI %s, root %s)"),
		*Ffi->GetAbiVersion(), *PathToRepositoryRoot);
}

void FLoreSourceControlProvider::Close()
{
	// Drain any queued commands.
	for (FLoreSourceControlCommand* Command : CommandQueue)
	{
		Command->Cancel();
		if (Command->bAutoDelete)
		{
			delete Command;
		}
	}
	CommandQueue.Empty();

	Ffi->Unload();
	bLoreAvailable = false;
	StateCache.Empty();
}

// ---------------------------------------------------------------------------
// Identity / status surface
// ---------------------------------------------------------------------------

const FName& FLoreSourceControlProvider::GetName() const { return ProviderName; }

FText FLoreSourceControlProvider::GetStatusText() const
{
	FFormatNamedArguments Args;
	Args.Add(TEXT("Available"), IsAvailable() ? LOCTEXT("Yes", "Yes") : LOCTEXT("No", "No"));
	Args.Add(TEXT("Root"), FText::FromString(PathToRepositoryRoot));
	Args.Add(TEXT("Branch"), FText::FromString(BranchName));
	Args.Add(TEXT("Abi"), FText::FromString(Ffi->GetAbiVersion()));
	return FText::Format(
		LOCTEXT("StatusText", "Lore (lorevm-ffi {Abi})\nAvailable: {Available}\nRoot: {Root}\nBranch: {Branch}"),
		Args);
}

TMap<ISourceControlProvider::EStatus, FString> FLoreSourceControlProvider::GetStatus() const
{
	TMap<EStatus, FString> Result;
	Result.Add(EStatus::Enabled, IsEnabled() ? TEXT("Yes") : TEXT("No"));
	Result.Add(EStatus::Connected, IsAvailable() ? TEXT("Yes") : TEXT("No"));
	Result.Add(EStatus::Repository, PathToRepositoryRoot);
	Result.Add(EStatus::Branch, BranchName);
	return Result;
}

bool FLoreSourceControlProvider::IsEnabled() const { return true; }
bool FLoreSourceControlProvider::IsAvailable() const { return bLoreAvailable && Ffi->IsOpen(); }

void FLoreSourceControlProvider::SetRepositoryInfo(bool bInAvailable, const FString& InRepositoryRoot, const FString& InBranchName, const FString& InRepositoryId)
{
	bLoreAvailable = bInAvailable;
	if (!InRepositoryRoot.IsEmpty()) PathToRepositoryRoot = InRepositoryRoot;
	BranchName = InBranchName;
	RepositoryId = InRepositoryId;
}

// ---------------------------------------------------------------------------
// State cache + GetState
// ---------------------------------------------------------------------------

TSharedRef<FLoreSourceControlState, ESPMode::ThreadSafe> FLoreSourceControlProvider::GetStateInternal(const FString& InFilename)
{
	if (TSharedRef<FLoreSourceControlState, ESPMode::ThreadSafe>* Existing = StateCache.Find(InFilename))
	{
		return *Existing;
	}
	TSharedRef<FLoreSourceControlState, ESPMode::ThreadSafe> New =
		MakeShared<FLoreSourceControlState, ESPMode::ThreadSafe>(InFilename);
	StateCache.Add(InFilename, New);
	return New;
}

ECommandResult::Type FLoreSourceControlProvider::GetState(const TArray<FString>& InFiles, TArray<FSourceControlStateRef>& OutState, EStateCacheUsage::Type InStateCacheUsage)
{
	if (!IsEnabled())
	{
		return ECommandResult::Failed;
	}

	TArray<FString> AbsoluteFiles = SourceControlHelpers::AbsoluteFilenames(InFiles);

	if (InStateCacheUsage == EStateCacheUsage::ForceUpdate)
	{
		// Synchronously refresh the requested files before returning their state.
		TSharedRef<FUpdateStatus, ESPMode::ThreadSafe> Operation = ISourceControlOperation::Create<FUpdateStatus>();
		Execute(Operation, nullptr, AbsoluteFiles, EConcurrency::Synchronous);
	}

	for (const FString& File : AbsoluteFiles)
	{
		OutState.Add(GetStateInternal(File));
	}
	return ECommandResult::Succeeded;
}

ECommandResult::Type FLoreSourceControlProvider::GetState(const TArray<FSourceControlChangelistRef>&, TArray<FSourceControlChangelistStateRef>&, EStateCacheUsage::Type)
{
	// MVP does not model changelists.
	return ECommandResult::Failed;
}

TArray<FSourceControlStateRef> FLoreSourceControlProvider::GetCachedStateByPredicate(TFunctionRef<bool(const FSourceControlStateRef&)> Predicate) const
{
	TArray<FSourceControlStateRef> Result;
	for (const auto& Pair : StateCache)
	{
		FSourceControlStateRef State = Pair.Value;
		if (Predicate(State))
		{
			Result.Add(State);
		}
	}
	return Result;
}

FDelegateHandle FLoreSourceControlProvider::RegisterSourceControlStateChanged_Handle(const FSourceControlStateChanged::FDelegate& SourceControlStateChanged)
{
	return OnSourceControlStateChanged.Add(SourceControlStateChanged);
}

void FLoreSourceControlProvider::UnregisterSourceControlStateChanged_Handle(FDelegateHandle Handle)
{
	OnSourceControlStateChanged.Remove(Handle);
}

// ---------------------------------------------------------------------------
// Execute / Tick — command machinery
// ---------------------------------------------------------------------------

void FLoreSourceControlProvider::PrimeCommand(FLoreSourceControlCommand& InCommand) const
{
	InCommand.Ffi = Ffi.Get();
	InCommand.PathToRepositoryRoot = PathToRepositoryRoot;
	InCommand.BranchName = BranchName;
	InCommand.Identity = Identity;
}

bool FLoreSourceControlProvider::CanExecuteOperation(const FSourceControlOperationRef& InOperation) const
{
	return WorkersMap.Contains(InOperation->GetName());
}

ECommandResult::Type FLoreSourceControlProvider::Execute(
	const FSourceControlOperationRef& InOperation,
	FSourceControlChangelistPtr InChangelist,
	const TArray<FString>& InFiles,
	EConcurrency::Type InConcurrency,
	const FSourceControlOperationComplete& InOperationCompleteDelegate)
{
	if (!IsEnabled() && InOperation->GetName() != "Connect")
	{
		InOperationCompleteDelegate.ExecuteIfBound(InOperation, ECommandResult::Failed);
		return ECommandResult::Failed;
	}

	// Find the worker that handles this operation.
	TSharedPtr<ILoreSourceControlWorker, ESPMode::ThreadSafe> Worker = CreateWorker(InOperation->GetName());
	if (!Worker.IsValid())
	{
		UE_LOG(LogSourceControl, Warning, TEXT("[Lore] operation '%s' not supported by the Lore provider"),
			*InOperation->GetName().ToString());
		InOperationCompleteDelegate.ExecuteIfBound(InOperation, ECommandResult::Failed);
		return ECommandResult::Failed;
	}

	FLoreSourceControlCommand* Command = new FLoreSourceControlCommand(InOperation, Worker.ToSharedRef(), InOperationCompleteDelegate);
	PrimeCommand(*Command);
	Command->Files = SourceControlHelpers::AbsoluteFilenames(InFiles);
	Command->Changelist = InChangelist;

	if (InConcurrency == EConcurrency::Synchronous)
	{
		return ExecuteSynchronousCommand(*Command, InOperation->GetInProgressString());
	}
	return IssueCommand(*Command);
}

ECommandResult::Type FLoreSourceControlProvider::ExecuteSynchronousCommand(FLoreSourceControlCommand& InCommand, const FText& /*Task*/)
{
	InCommand.bAutoDelete = false;
	InCommand.DoWork();
	const ECommandResult::Type Result = InCommand.ReturnResults();
	OnSourceControlStateChanged.Broadcast();
	delete &InCommand;
	return Result;
}

ECommandResult::Type FLoreSourceControlProvider::IssueCommand(FLoreSourceControlCommand& InCommand)
{
	if (GThreadPool != nullptr)
	{
		InCommand.bAutoDelete = true;
		CommandQueue.Add(&InCommand);
		GThreadPool->AddQueuedWork(&InCommand);
		return ECommandResult::Succeeded;
	}

	// No thread pool: run inline.
	return ExecuteSynchronousCommand(InCommand, FText::GetEmpty());
}

void FLoreSourceControlProvider::Tick()
{
	bool bStatesUpdated = false;
	for (int32 Index = 0; Index < CommandQueue.Num(); ++Index)
	{
		FLoreSourceControlCommand* Command = CommandQueue[Index];
		if (Command->bExecuteProcessed.GetValue() > 0)
		{
			CommandQueue.RemoveAt(Index, 1, EAllowShrinking::No);
			--Index;

			Command->ReturnResults();
			bStatesUpdated = true;

			if (Command->bAutoDelete)
			{
				delete Command;
			}
		}
	}

	if (bStatesUpdated)
	{
		OnSourceControlStateChanged.Broadcast();
	}
}

// ---------------------------------------------------------------------------
// Cancel / labels / changelists — minimal MVP behaviour
// ---------------------------------------------------------------------------

bool FLoreSourceControlProvider::CanCancelOperation(const FSourceControlOperationRef&) const { return false; }
void FLoreSourceControlProvider::CancelOperation(const FSourceControlOperationRef&) {}

TArray<TSharedRef<ISourceControlLabel>> FLoreSourceControlProvider::GetLabels(const FString&) const
{
	return TArray<TSharedRef<ISourceControlLabel>>();
}

TArray<FSourceControlChangelistRef> FLoreSourceControlProvider::GetChangelists(EStateCacheUsage::Type)
{
	return TArray<FSourceControlChangelistRef>();
}

// ---------------------------------------------------------------------------
// Capability queries
// ---------------------------------------------------------------------------

bool FLoreSourceControlProvider::UsesLocalReadOnlyState() const { return true; }  // locking workflow
bool FLoreSourceControlProvider::UsesChangelists() const { return false; }
bool FLoreSourceControlProvider::UsesUncontrolledChangelists() const { return false; }
bool FLoreSourceControlProvider::UsesCheckout() const { return true; }            // lock == check out
bool FLoreSourceControlProvider::UsesFileRevisions() const { return true; }
bool FLoreSourceControlProvider::UsesSnapshots() const { return false; }
bool FLoreSourceControlProvider::AllowsDiffAgainstDepot() const { return true; }

TOptional<bool> FLoreSourceControlProvider::IsAtLatestRevision() const { return TOptional<bool>(); }
TOptional<int> FLoreSourceControlProvider::GetNumLocalChanges() const { return TOptional<int>(); }

bool FLoreSourceControlProvider::QueryStateBranchConfig(const FString&, const FString&) { return false; }
void FLoreSourceControlProvider::RegisterStateBranches(const TArray<FString>&, const FString&) {}
int32 FLoreSourceControlProvider::GetStateBranchIndex(const FString&) const { return INDEX_NONE; }

#if SOURCE_CONTROL_WITH_SLATE
TSharedRef<SWidget> FLoreSourceControlProvider::MakeSettingsWidget() const
{
	// A real settings widget (lib path, in-memory/offline toggles, identity) is a
	// follow-up; the editor still functions with ini-driven defaults.
	return SNew(STextBlock)
		.Text(LOCTEXT("SettingsPlaceholder", "Lore source control settings are configured in SourceControlSettings.ini (MVP)."));
}
#endif

#undef LOCTEXT_NAMESPACE
