// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "ILoreSourceControlWorker.h"
#include "LoreSourceControlState.h"
#include "LoreSourceControlRevision.h"

/**
 * The per-operation Workers for the MVP. Each maps a UE source-control operation
 * onto one or more lore op ids driven through FLorevmFfi:
 *
 *   Connect      -> repository.info / repository.status  (probe the warm handle)
 *   UpdateStatus -> repository.status + lock.file_status  (overlay refresh)
 *   CheckOut     -> lock.file_acquire                     (acquire lock)
 *   MarkForAdd   -> file.stage
 *   Delete       -> file.stage (staged removal)
 *   CheckIn      -> revision.commit (+ branch.push)       (submit)
 *   Revert       -> lock.file_release (+ file.unstage)    (release lock)
 *   Sync         -> revision.sync                         (pull latest)
 *   GetHistory   -> file.history                          (per-file history)
 *
 * Each worker collects FLoreSourceControlState rows in `States` (populated on the
 * worker thread by Execute) and flushes them to the provider in UpdateStates (game
 * thread). The States vector and the per-worker fields are the marshalling channel
 * between the two threads.
 */

/** Probe the bridge + repo, refresh the provider's connection info. */
class FLoreConnectWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	bool bAvailable = false;
	FString RepositoryRoot;
	FString BranchName;
	FString RepositoryId;
};

/** Refresh per-file status + lock state for the requested files (overlay refresh). */
class FLoreUpdateStatusWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	TArray<FLoreSourceControlState> States;
};

/** Acquire a lock on the files (UE "Check Out"). */
class FLoreCheckOutWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	TArray<FLoreSourceControlState> States;
};

/** Stage a new file for the next commit (UE "Mark For Add"). */
class FLoreMarkForAddWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	TArray<FLoreSourceControlState> States;
};

/** Stage a removal (UE "Delete"). */
class FLoreDeleteWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	TArray<FLoreSourceControlState> States;
};

/** Commit + push the submitted files (UE "Check In"). */
class FLoreCheckInWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	TArray<FLoreSourceControlState> States;
};

/** Discard local changes + release the lock (UE "Revert"). */
class FLoreRevertWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	TArray<FLoreSourceControlState> States;
};

/** Pull the latest revision (UE "Sync"). */
class FLoreSyncWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	TArray<FLoreSourceControlState> States;
};

/**
 * FLoreHistoryWorker — fetch per-file revision history via `file.history`.
 *
 * Lore op: `file.history`
 *   args: { "path": "<repo-relative>", "branch": "<name>", "length": <n> }
 *   result: { "entries": [{ "path", "repository", "revision", "revision_number",
 *                           "parents", "address", "size", "action" }, ...] }
 *
 * One call per file in Command.Files (parallel history per-file is the normal
 * UE source-control history-panel use case). The collected FLoreSourceControlRevision
 * records are flushed into the provider's state cache via UpdateStates, where the
 * state's History array is populated.
 *
 * Notes / follow-up (UE-BUILD-PENDING):
 *  - `file.history` does not return timestamps or commit messages. Calling
 *    `revision.info` per hash to fill those fields is a follow-up task.
 *  - The `length` limit defaults to 50; expose it as an operation parameter
 *    when the editor's history panel supports it.
 */
class FLoreHistoryWorker : public ILoreSourceControlWorker
{
public:
	virtual FName GetName() const override;
	virtual bool Execute(FLoreSourceControlCommand& InCommand) override;
	virtual bool UpdateStates() const override;

	/** Per-file history results: filename -> ordered list of revisions (newest-first). */
	TMap<FString, TArray<TSharedRef<FLoreSourceControlRevision, ESPMode::ThreadSafe>>> HistoryMap;
};
