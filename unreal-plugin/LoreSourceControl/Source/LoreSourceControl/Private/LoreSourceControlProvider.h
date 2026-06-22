// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "ISourceControlProvider.h"
#include "ISourceControlState.h"
#include "ISourceControlChangelist.h"
#include "ISourceControlChangelistState.h"
#include "LoreSourceControlState.h"
#include "ILoreSourceControlWorker.h"

class FLoreSourceControlCommand;
class FLorevmFfi;

DECLARE_DELEGATE_RetVal(FLoreSourceControlWorkerRef, FGetLoreSourceControlWorker);

/**
 * FLoreSourceControlProvider — the ISourceControlProvider that backs the editor's
 * Revision Control system with Lore, driving everything through the lorevm-ffi
 * C ABI (no CLI subprocess).
 *
 * Baseline: UE 5.3+ (single clean interface surface; see docs/BUILD.md for the
 * version-gating notes if you need to target older 5.x).
 *
 * Responsibilities:
 *  - Own the FLorevmFfi bridge: load the cdylib + open a warm handle on Init().
 *  - Map UE source-control operations to per-op Workers (RegisterWorker).
 *  - Execute() queues a Command carrying the bridge onto a worker thread (async)
 *    or runs it inline (sync), then Tick() flushes finished commands' results.
 *  - GetState() serves cached FLoreSourceControlState (overlay icons) and can
 *    force a refresh via an UpdateStatus command.
 */
class FLoreSourceControlProvider : public ISourceControlProvider
{
public:
	FLoreSourceControlProvider();
	virtual ~FLoreSourceControlProvider();

	//~ Begin ISourceControlProvider interface
	virtual void Init(bool bForceConnection = true) override;
	virtual void Close() override;
	virtual const FName& GetName() const override;
	virtual FText GetStatusText() const override;
	virtual TMap<EStatus, FString> GetStatus() const override;
	virtual bool IsEnabled() const override;
	virtual bool IsAvailable() const override;
	virtual bool QueryStateBranchConfig(const FString& ConfigSrc, const FString& ConfigDest) override;
	virtual void RegisterStateBranches(const TArray<FString>& BranchNames, const FString& ContentRoot) override;
	virtual int32 GetStateBranchIndex(const FString& BranchName) const override;
	virtual ECommandResult::Type GetState(const TArray<FString>& InFiles, TArray<FSourceControlStateRef>& OutState, EStateCacheUsage::Type InStateCacheUsage) override;
	virtual ECommandResult::Type GetState(const TArray<FSourceControlChangelistRef>& InChangelists, TArray<FSourceControlChangelistStateRef>& OutState, EStateCacheUsage::Type InStateCacheUsage) override;
	virtual TArray<FSourceControlStateRef> GetCachedStateByPredicate(TFunctionRef<bool(const FSourceControlStateRef&)> Predicate) const override;
	virtual FDelegateHandle RegisterSourceControlStateChanged_Handle(const FSourceControlStateChanged::FDelegate& SourceControlStateChanged) override;
	virtual void UnregisterSourceControlStateChanged_Handle(FDelegateHandle Handle) override;
	virtual ECommandResult::Type Execute(const FSourceControlOperationRef& InOperation, FSourceControlChangelistPtr InChangelist, const TArray<FString>& InFiles, EConcurrency::Type InConcurrency = EConcurrency::Synchronous, const FSourceControlOperationComplete& InOperationCompleteDelegate = FSourceControlOperationComplete()) override;
	virtual bool CanExecuteOperation(const FSourceControlOperationRef& InOperation) const override;
	virtual bool CanCancelOperation(const FSourceControlOperationRef& InOperation) const override;
	virtual void CancelOperation(const FSourceControlOperationRef& InOperation) override;
	virtual TArray<TSharedRef<class ISourceControlLabel>> GetLabels(const FString& InMatchingSpec) const override;
	virtual TArray<FSourceControlChangelistRef> GetChangelists(EStateCacheUsage::Type InStateCacheUsage) override;
	virtual bool UsesLocalReadOnlyState() const override;
	virtual bool UsesChangelists() const override;
	virtual bool UsesUncontrolledChangelists() const override;
	virtual bool UsesCheckout() const override;
	virtual bool UsesFileRevisions() const override;
	virtual bool UsesSnapshots() const override;
	virtual bool AllowsDiffAgainstDepot() const override;
	virtual TOptional<bool> IsAtLatestRevision() const override;
	virtual TOptional<int> GetNumLocalChanges() const override;
	virtual void Tick() override;
#if SOURCE_CONTROL_WITH_SLATE
	virtual TSharedRef<class SWidget> MakeSettingsWidget() const override;
#endif
	//~ End ISourceControlProvider interface

	/** Register a worker so the provider can map an operation name to the work. */
	void RegisterWorker(const FName& InName, const FGetLoreSourceControlWorker& InDelegate);

	/** The shared FFI bridge (owned by the provider). */
	FLorevmFfi& GetFfi() const { return *Ffi; }

	const FString& GetPathToRepositoryRoot() const { return PathToRepositoryRoot; }
	const FString& GetIdentity() const { return Identity; }
	const FString& GetBranchName() const { return BranchName; }
	bool IsLoreAvailable() const { return bLoreAvailable; }

	/** Store discovered repo info in the provider's connection state. Game thread only. */
	void SetRepositoryInfo(bool bInAvailable, const FString& InRepositoryRoot, const FString& InBranchName, const FString& InRepositoryId);

	/** Get or create the cached state for a file. */
	TSharedRef<FLoreSourceControlState, ESPMode::ThreadSafe> GetStateInternal(const FString& InFilename);

private:
	/** Build a worker for an operation name (or null if unsupported). */
	TSharedPtr<ILoreSourceControlWorker, ESPMode::ThreadSafe> CreateWorker(const FName& InOperationName) const;

	/** Run a command inline (synchronous). */
	ECommandResult::Type ExecuteSynchronousCommand(FLoreSourceControlCommand& InCommand, const FText& Task);

	/** Queue a command on the source-control thread pool, or run inline if unavailable. */
	ECommandResult::Type IssueCommand(FLoreSourceControlCommand& InCommand);

	/** Populate the command's connection context (root/branch/identity/bridge). */
	void PrimeCommand(FLoreSourceControlCommand& InCommand) const;

	FName ProviderName = FName("Lore");

	/** The lorevm-ffi bridge. Created in the ctor; loaded + opened on Init. */
	TUniquePtr<FLorevmFfi> Ffi;

	bool bLoreAvailable = false;
	FString PathToRepositoryRoot;
	FString Identity;
	FString BranchName;
	FString RepositoryId;
	FString LastError;

	/** State cache, keyed by absolute filename. */
	TMap<FString, TSharedRef<FLoreSourceControlState, ESPMode::ThreadSafe>> StateCache;

	/** Registered operation-name -> worker factory. */
	TMap<FName, FGetLoreSourceControlWorker> WorkersMap;

	/** Commands awaiting completion on the game thread. */
	TArray<FLoreSourceControlCommand*> CommandQueue;

	FSourceControlStateChanged OnSourceControlStateChanged;
};
