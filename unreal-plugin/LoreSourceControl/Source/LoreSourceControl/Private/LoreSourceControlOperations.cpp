// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlOperations.h"
#include "LoreSourceControlCommand.h"
#include "LoreSourceControlModule.h"
#include "LoreSourceControlProvider.h"
#include "LoreSourceControlUtils.h"
#include "LoreSourceControlLog.h"
#include "Ffi/LorevmFfi.h"

#include "SourceControlOperations.h"
#include "Dom/JsonObject.h"
#include "Dom/JsonValue.h"

#define LOCTEXT_NAMESPACE "LoreSourceControl.Ops"

// Helper: build a {"paths":[...], "branch":...} args object for the lock ops from
// a command's repo-relative file list.
static TSharedRef<FJsonObject> MakeLockArgs(const FLoreSourceControlCommand& Command)
{
	TSharedRef<FJsonObject> Args = MakeShared<FJsonObject>();
	TArray<TSharedPtr<FJsonValue>> Paths;
	for (const FString& Abs : Command.Files)
	{
		Paths.Add(MakeShared<FJsonValueString>(
			LoreSourceControlUtils::ToRepoRelative(Command.PathToRepositoryRoot, Abs)));
	}
	Args->SetArrayField(TEXT("paths"), Paths);
	Args->SetStringField(TEXT("branch"), Command.BranchName);
	return Args;
}

// Helper: push a worker's collected States into the provider's state cache. Game
// thread only (called from UpdateStates).
static bool FlushStatesToProvider(const TArray<FLoreSourceControlState>& States)
{
	if (FLoreSourceControlModule* Module = FLoreSourceControlModule::GetPtr())
	{
		FLoreSourceControlProvider& Provider = Module->GetProvider();
		for (const FLoreSourceControlState& In : States)
		{
			TSharedRef<FLoreSourceControlState, ESPMode::ThreadSafe> Cached =
				Provider.GetStateInternal(In.LocalFilename);
			Cached->WorkingCopyState = In.WorkingCopyState;
			Cached->LockState = In.LockState;
			Cached->LockUser = In.LockUser;
			Cached->bNewerVersionOnServer = In.bNewerVersionOnServer;
			Cached->bUsingLocking = In.bUsingLocking;
			Cached->TimeStamp = In.TimeStamp;
		}
		return States.Num() > 0;
	}
	return false;
}

// ===========================================================================
// Connect
// ===========================================================================
FName FLoreConnectWorker::GetName() const { return "Connect"; }

bool FLoreConnectWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}

	// The warm handle is opened by the provider before queuing Connect; probe it
	// with repository.info to confirm the repo is reachable and read branch/id.
	const TSharedRef<FJsonObject> Args = MakeShared<FJsonObject>();
	const FLorevmResult R = Command.Ffi->Call(TEXT("repository.info"), Args);
	if (!R.bSuccess)
	{
		// repository.info may legitimately fail offline; fall back to status which
		// works against an in-memory/offline repo.
		const FLorevmResult S = Command.Ffi->Call(TEXT("repository.status"), MakeShared<FJsonObject>());
		if (!S.bSuccess)
		{
			Command.ErrorMessages.Add(FString::Printf(TEXT("connect probe failed: [%s] %s"), *S.ErrorKind, *S.ErrorMessage));
			return false;
		}
		bAvailable = true;
		if (S.Result.IsValid())
		{
			const TSharedPtr<FJsonObject>* Rev = nullptr;
			if (S.Result->TryGetObjectField(TEXT("revision"), Rev) && Rev)
			{
				(*Rev)->TryGetStringField(TEXT("branch_name"), BranchName);
				(*Rev)->TryGetStringField(TEXT("repository"), RepositoryId);
			}
		}
		RepositoryRoot = Command.PathToRepositoryRoot;
		return true;
	}

	bAvailable = true;
	RepositoryRoot = Command.PathToRepositoryRoot;
	if (R.Result.IsValid())
	{
		// repository.info result: { remote_url, id, name, default_branch, default_branch_name, ... }
		R.Result->TryGetStringField(TEXT("default_branch_name"), BranchName);
		R.Result->TryGetStringField(TEXT("id"), RepositoryId);
	}
	return true;
}

bool FLoreConnectWorker::UpdateStates() const
{
	if (FLoreSourceControlModule* Module = FLoreSourceControlModule::GetPtr())
	{
		Module->GetProvider().SetRepositoryInfo(bAvailable, RepositoryRoot, BranchName, RepositoryId);
	}
	return true;
}

// ===========================================================================
// UpdateStatus — overlay refresh (status + locks)
// ===========================================================================
FName FLoreUpdateStatusWorker::GetName() const { return "UpdateStatus"; }

bool FLoreUpdateStatusWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}
	if (Command.Files.Num() == 0)
	{
		return true; // Nothing to refresh.
	}

	return LoreSourceControlUtils::QueryFileStates(
		*Command.Ffi, Command.PathToRepositoryRoot, Command.BranchName, Command.Identity,
		Command.Files, States, Command.ErrorMessages);
}

bool FLoreUpdateStatusWorker::UpdateStates() const
{
	return FlushStatesToProvider(States);
}

// ===========================================================================
// CheckOut — acquire lock
// ===========================================================================
FName FLoreCheckOutWorker::GetName() const { return "CheckOut"; }

bool FLoreCheckOutWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}

	const FLorevmResult R = Command.Ffi->Call(TEXT("lock.file_acquire"), MakeLockArgs(Command));
	if (!R.bSuccess)
	{
		Command.ErrorMessages.Add(FString::Printf(TEXT("lock.file_acquire: [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
		return false;
	}

	// On success, mark the requested files as LockedByMe for the cache.
	for (const FString& Abs : Command.Files)
	{
		FLoreSourceControlState State(Abs);
		State.LockState = ELoreLockState::LockedByMe;
		State.LockUser = Command.Identity;
		State.bUsingLocking = true;
		State.TimeStamp = FDateTime::Now();
		States.Add(State);
	}
	return true;
}

bool FLoreCheckOutWorker::UpdateStates() const { return FlushStatesToProvider(States); }

// ===========================================================================
// MarkForAdd — stage a new file
// ===========================================================================
FName FLoreMarkForAddWorker::GetName() const { return "MarkForAdd"; }

bool FLoreMarkForAddWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}

	// file.stage takes a paths array; stage all requested files in one call.
	const TSharedRef<FJsonObject> Args = MakeShared<FJsonObject>();
	TArray<TSharedPtr<FJsonValue>> Paths;
	for (const FString& Abs : Command.Files)
	{
		Paths.Add(MakeShared<FJsonValueString>(
			LoreSourceControlUtils::ToRepoRelative(Command.PathToRepositoryRoot, Abs)));
	}
	Args->SetArrayField(TEXT("paths"), Paths);

	const FLorevmResult R = Command.Ffi->Call(TEXT("file.stage"), Args);
	if (!R.bSuccess)
	{
		Command.ErrorMessages.Add(FString::Printf(TEXT("file.stage: [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
		return false;
	}
	for (const FString& Abs : Command.Files)
	{
		FLoreSourceControlState State(Abs);
		State.WorkingCopyState = ELoreWorkingCopyState::Added;
		State.TimeStamp = FDateTime::Now();
		States.Add(State);
	}
	return true;
}

bool FLoreMarkForAddWorker::UpdateStates() const { return FlushStatesToProvider(States); }

// ===========================================================================
// Delete — stage a removal
// ===========================================================================
FName FLoreDeleteWorker::GetName() const { return "Delete"; }

bool FLoreDeleteWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}

	// file.stage reconciles individual file paths against the filesystem, so a
	// deleted-on-disk file is staged as a removal. Stage all paths in one call.
	const TSharedRef<FJsonObject> Args = MakeShared<FJsonObject>();
	TArray<TSharedPtr<FJsonValue>> Paths;
	for (const FString& Abs : Command.Files)
	{
		Paths.Add(MakeShared<FJsonValueString>(
			LoreSourceControlUtils::ToRepoRelative(Command.PathToRepositoryRoot, Abs)));
	}
	Args->SetArrayField(TEXT("paths"), Paths);

	const FLorevmResult R = Command.Ffi->Call(TEXT("file.stage"), Args);
	if (!R.bSuccess)
	{
		Command.ErrorMessages.Add(FString::Printf(TEXT("file.stage(delete): [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
		return false;
	}
	for (const FString& Abs : Command.Files)
	{
		FLoreSourceControlState State(Abs);
		State.WorkingCopyState = ELoreWorkingCopyState::Deleted;
		State.TimeStamp = FDateTime::Now();
		States.Add(State);
	}
	return true;
}

bool FLoreDeleteWorker::UpdateStates() const { return FlushStatesToProvider(States); }

// ===========================================================================
// CheckIn — commit (+ push)
// ===========================================================================
FName FLoreCheckInWorker::GetName() const { return "CheckIn"; }

bool FLoreCheckInWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}

	// Pull the description off the FCheckIn operation.
	FString Description;
	if (Command.Operation->GetName() == "CheckIn")
	{
		const TSharedRef<FCheckIn> CheckIn = StaticCastSharedRef<FCheckIn>(Command.Operation);
		Description = CheckIn->GetDescription().ToString();
	}

	const TSharedRef<FJsonObject> Args = MakeShared<FJsonObject>();
	Args->SetStringField(TEXT("message"), Description);
	const FLorevmResult R = Command.Ffi->Call(TEXT("revision.commit"), Args);
	if (!R.bSuccess)
	{
		Command.ErrorMessages.Add(FString::Printf(TEXT("revision.commit: [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
		return false;
	}

	// Optionally push the branch so the submit is visible to others.
	if (Command.bPushAfterCommit)
	{
		const FLorevmResult P = Command.Ffi->Call(TEXT("branch.push"), MakeShared<FJsonObject>());
		if (!P.bSuccess)
		{
			// Commit succeeded but push failed — surface as a warning, not a hard fail.
			Command.InfoMessages.Add(FString::Printf(TEXT("committed; branch.push failed: [%s] %s"), *P.ErrorKind, *P.ErrorMessage));
		}
	}

	// After a successful submit the files are clean and the lock is released.
	for (const FString& Abs : Command.Files)
	{
		FLoreSourceControlState State(Abs);
		State.WorkingCopyState = ELoreWorkingCopyState::Unchanged;
		State.LockState = ELoreLockState::NotLocked;
		State.TimeStamp = FDateTime::Now();
		States.Add(State);
	}
	return true;
}

bool FLoreCheckInWorker::UpdateStates() const { return FlushStatesToProvider(States); }

// ===========================================================================
// Revert — release lock (+ discard local changes)
// ===========================================================================
FName FLoreRevertWorker::GetName() const { return "Revert"; }

bool FLoreRevertWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}

	// Release any lock we hold on these files. file_release wants owner + owner_id;
	// we pass our identity for both (the MVP identity model). A "not found" is fine.
	const TSharedRef<FJsonObject> Args = MakeLockArgs(Command);
	Args->SetStringField(TEXT("owner"), Command.Identity);
	Args->SetStringField(TEXT("owner_id"), Command.Identity);
	const FLorevmResult R = Command.Ffi->Call(TEXT("lock.file_release"), Args);
	if (!R.bSuccess)
	{
		Command.ErrorMessages.Add(FString::Printf(TEXT("lock.file_release: [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
		return false;
	}

	// Unstage any staged change so the working copy reflects a clean revert.
	// file.unstage takes a paths array; do it in one best-effort call.
	{
		const TSharedRef<FJsonObject> UnstageArgs = MakeShared<FJsonObject>();
		TArray<TSharedPtr<FJsonValue>> Paths;
		for (const FString& Abs : Command.Files)
		{
			Paths.Add(MakeShared<FJsonValueString>(
				LoreSourceControlUtils::ToRepoRelative(Command.PathToRepositoryRoot, Abs)));
		}
		UnstageArgs->SetArrayField(TEXT("paths"), Paths);
		Command.Ffi->Call(TEXT("file.unstage"), UnstageArgs); // best-effort
	}

	for (const FString& Abs : Command.Files)
	{
		FLoreSourceControlState State(Abs);
		State.WorkingCopyState = ELoreWorkingCopyState::Unchanged;
		State.LockState = ELoreLockState::NotLocked;
		State.TimeStamp = FDateTime::Now();
		States.Add(State);
	}
	return true;
}

bool FLoreRevertWorker::UpdateStates() const { return FlushStatesToProvider(States); }

// ===========================================================================
// Sync — pull latest revision
// ===========================================================================
FName FLoreSyncWorker::GetName() const { return "Sync"; }

bool FLoreSyncWorker::Execute(FLoreSourceControlCommand& Command)
{
	if (Command.Ffi == nullptr || !Command.Ffi->IsLoaded())
	{
		Command.ErrorMessages.Add(TEXT("lorevm-ffi bridge not loaded"));
		return false;
	}

	const FLorevmResult R = Command.Ffi->Call(TEXT("revision.sync"), MakeShared<FJsonObject>());
	if (!R.bSuccess)
	{
		Command.ErrorMessages.Add(FString::Printf(TEXT("revision.sync: [%s] %s"), *R.ErrorKind, *R.ErrorMessage));
		return false;
	}

	// After a sync the synced files are current.
	for (const FString& Abs : Command.Files)
	{
		FLoreSourceControlState State(Abs);
		State.bNewerVersionOnServer = false;
		State.TimeStamp = FDateTime::Now();
		States.Add(State);
	}
	return true;
}

bool FLoreSyncWorker::UpdateStates() const { return FlushStatesToProvider(States); }

#undef LOCTEXT_NAMESPACE
