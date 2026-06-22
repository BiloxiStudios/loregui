// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlUtils.h"
#include "Ffi/LorevmFfi.h"
#include "LoreSourceControlLog.h"

#include "Misc/Paths.h"
#include "Dom/JsonObject.h"
#include "Dom/JsonValue.h"

namespace LoreSourceControlUtils
{

FString ToRepoRelative(const FString& RepositoryRoot, const FString& AbsoluteFilename)
{
	FString Abs = FPaths::ConvertRelativePathToFull(AbsoluteFilename);
	FString Root = FPaths::ConvertRelativePathToFull(RepositoryRoot);
	FPaths::NormalizeDirectoryName(Root);

	FString Relative = Abs;
	if (FPaths::MakePathRelativeTo(Relative, *(Root / TEXT(""))))
	{
		// MakePathRelativeTo already yields '/'-separated; ensure no leading slash.
		Relative.RemoveFromStart(TEXT("/"));
		return Relative;
	}
	// Not under the root — return the absolute path as a last resort.
	return Abs;
}

FString ToAbsolute(const FString& RepositoryRoot, const FString& RelativePath)
{
	return FPaths::ConvertRelativePathToFull(RepositoryRoot, RelativePath);
}

ELoreWorkingCopyState::Type MapWorkingCopyState(const FString& Action, bool bConflict)
{
	if (bConflict)
	{
		return ELoreWorkingCopyState::Conflicted;
	}
	// `repository.status` serialises StatusFileAction as lowercase: keep/add/delete/move/copy.
	if (Action == TEXT("add"))    return ELoreWorkingCopyState::Added;
	if (Action == TEXT("delete")) return ELoreWorkingCopyState::Deleted;
	if (Action == TEXT("move") || Action == TEXT("copy")) return ELoreWorkingCopyState::Modified;
	if (Action == TEXT("keep"))   return ELoreWorkingCopyState::Modified; // listed-but-kept == locally dirty
	return ELoreWorkingCopyState::Unchanged;
}

bool QueryFileStates(FLorevmFfi& Ffi,
                     const FString& RepositoryRoot,
                     const FString& BranchName,
                     const FString& Identity,
                     const TArray<FString>& AbsoluteFiles,
                     TArray<FLoreSourceControlState>& OutStates,
                     TArray<FString>& OutErrorMessages)
{
	// Build a path map: repo-relative -> absolute, so we can fold both queries back
	// onto the caller's file list, and seed an Unchanged/NotLocked default per file.
	TMap<FString, FLoreSourceControlState> ByRelative;
	for (const FString& Abs : AbsoluteFiles)
	{
		const FString Rel = ToRepoRelative(RepositoryRoot, Abs);
		FLoreSourceControlState State(Abs);
		State.WorkingCopyState = ELoreWorkingCopyState::Unchanged;
		State.LockState = ELoreLockState::NotLocked;
		State.TimeStamp = FDateTime::Now();
		ByRelative.Add(Rel, State);
	}

	// ---- 1) repository.status (working-copy changes) -----------------------
	{
		const TSharedRef<FJsonObject> Args = MakeShared<FJsonObject>();
		Args->SetBoolField(TEXT("check_dirty"), true);
		// Limit to the requested paths when small; empty checks all.
		TArray<TSharedPtr<FJsonValue>> Paths;
		for (const auto& Pair : ByRelative)
		{
			Paths.Add(MakeShared<FJsonValueString>(Pair.Key));
		}
		Args->SetArrayField(TEXT("paths"), Paths);

		const FLorevmResult R = Ffi.Call(TEXT("repository.status"), Args);
		if (!R.bSuccess)
		{
			OutErrorMessages.Add(FString::Printf(TEXT("repository.status: [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
			// Non-fatal: continue to lock query so overlays still get lock info.
		}
		else if (R.Result.IsValid())
		{
			const TArray<TSharedPtr<FJsonValue>>* Files = nullptr;
			if (R.Result->TryGetArrayField(TEXT("files"), Files) && Files)
			{
				for (const TSharedPtr<FJsonValue>& V : *Files)
				{
					const TSharedPtr<FJsonObject> File = V->AsObject();
					if (!File.IsValid()) continue;
					FString Path, Action;
					File->TryGetStringField(TEXT("path"), Path);
					File->TryGetStringField(TEXT("action"), Action);
					bool bConflict = false;
					File->TryGetBoolField(TEXT("conflict"), bConflict);
					if (FLoreSourceControlState* State = ByRelative.Find(Path))
					{
						State->WorkingCopyState = MapWorkingCopyState(Action, bConflict);
					}
				}
			}
		}
	}

	// ---- 2) lock.file_status (advisory locks) ------------------------------
	{
		const TSharedRef<FJsonObject> Args = MakeShared<FJsonObject>();
		Args->SetStringField(TEXT("branch"), BranchName);
		TArray<TSharedPtr<FJsonValue>> Paths;
		for (const auto& Pair : ByRelative)
		{
			Paths.Add(MakeShared<FJsonValueString>(Pair.Key));
		}
		Args->SetArrayField(TEXT("paths"), Paths);

		const FLorevmResult R = Ffi.Call(TEXT("lock.file_status"), Args);
		if (!R.bSuccess)
		{
			OutErrorMessages.Add(FString::Printf(TEXT("lock.file_status: [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
		}
		else if (R.Result.IsValid())
		{
			const TArray<TSharedPtr<FJsonValue>>* Locks = nullptr;
			if (R.Result->TryGetArrayField(TEXT("locks"), Locks) && Locks)
			{
				for (const TSharedPtr<FJsonValue>& V : *Locks)
				{
					const TSharedPtr<FJsonObject> Lock = V->AsObject();
					if (!Lock.IsValid()) continue;
					FString Path, Owner;
					Lock->TryGetStringField(TEXT("path"), Path);
					Lock->TryGetStringField(TEXT("owner"), Owner);
					if (FLoreSourceControlState* State = ByRelative.Find(Path))
					{
						State->LockUser = Owner;
						State->LockState = (!Identity.IsEmpty() && Owner == Identity)
							? ELoreLockState::LockedByMe
							: ELoreLockState::LockedByOther;
					}
				}
			}
		}
	}

	for (auto& Pair : ByRelative)
	{
		OutStates.Add(Pair.Value);
	}
	return true;
}

} // namespace LoreSourceControlUtils
