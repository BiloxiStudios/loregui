// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "ILoreSourceControlWorker.h"
#include "LoreSourceControlState.h"

/**
 * The per-operation Workers for the MVP. Each maps a UE source-control operation
 * onto one or more lore op ids driven through FLorevmFfi:
 *
 *   Connect      -> repository.info / repository.status (probe the warm handle)
 *   UpdateStatus -> repository.status + lock.file_status  (overlay refresh)
 *   CheckOut     -> lock.file_acquire                     (acquire lock)
 *   MarkForAdd   -> file.stage
 *   Delete       -> file.stage (staged removal)
 *   CheckIn      -> revision.commit (+ branch.push)       (submit)
 *   Revert       -> lock.file_release (+ unstage)         (release lock)
 *   Sync         -> revision.sync                         (pull latest)
 *   UpdateStatus history -> revision.history / file.history
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
