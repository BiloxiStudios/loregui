// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "Misc/IQueuedWork.h"
#include "ISourceControlProvider.h"
#include "ISourceControlOperation.h"
#include "ISourceControlChangelist.h"
#include "ILoreSourceControlWorker.h"
#include "LoreSourceControlState.h"

class FLorevmFfi;

/**
 * FLoreSourceControlCommand — one queued unit of source-control work, run on a
 * worker thread (or inline for synchronous ops). Mirrors the GitSourceControl /
 * reference-plugin Command shape, but instead of a CLI binary path it carries a
 * pointer to the shared FLorevmFfi bridge the Worker drives.
 *
 * Lifecycle:
 *   - Built on the game thread by the Provider's Execute().
 *   - DoThreadedWork()/DoWork() runs Worker->Execute(*this) on a worker thread,
 *     which calls Ffi->Call(...) for each lore op.
 *   - ReturnResults() runs back on the game thread: Worker->UpdateStates() pushes
 *     results into the Provider's state cache, then the completion delegate fires.
 */
class FLoreSourceControlCommand : public IQueuedWork
{
public:
	FLoreSourceControlCommand(const FSourceControlOperationRef& InOperation,
	                          const FLoreSourceControlWorkerRef& InWorker,
	                          const FSourceControlOperationComplete& InOperationCompleteDelegate = FSourceControlOperationComplete());

	/** Run the worker; returns whether it succeeded. */
	bool DoWork();

	//~ Begin IQueuedWork interface
	virtual void DoThreadedWork() override;
	virtual void Abandon() override;
	//~ End IQueuedWork interface

	void Cancel();
	bool IsCanceled() const;

	/** Push results into the state cache + fire the completion delegate. Game thread only. */
	ECommandResult::Type ReturnResults();

public:
	/** The shared FFI bridge (owned by the Provider). Workers call this. NOT owned here. */
	FLorevmFfi* Ffi = nullptr;

	/** Repository working dir (the lore "dir"); set at connect. */
	FString PathToRepositoryRoot;

	/** Current branch name — lock ops are branch-scoped. */
	FString BranchName;

	/** Commit identity (email/user id), used to classify a lock as mine vs other. */
	FString Identity;

	/** Check out acquires a lock; binary assets stay read-only until checked out. */
	bool bUsingLocking = true;

	/** Check in pushes the revision to the server after committing. */
	bool bPushAfterCommit = true;

	FSourceControlOperationRef Operation;
	FLoreSourceControlWorkerRef Worker;
	FSourceControlOperationComplete OperationCompleteDelegate;

	/** Set once Execute has run. */
	FThreadSafeCounter bExecuteProcessed;

	/** Raised to request cancel. */
	FThreadSafeCounter bCancelledCounter;

	bool bCommandSuccessful = false;

	EConcurrency::Type Concurrency = EConcurrency::Synchronous;

	/** Tick deletes this command when it finishes. */
	bool bAutoDelete = false;

	TArray<FString> Files;
	FSourceControlChangelistPtr Changelist;

	TArray<FString> InfoMessages;
	TArray<FString> ErrorMessages;
};
