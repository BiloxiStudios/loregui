// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "ISourceControlState.h"
#include "ISourceControlRevision.h"

/**
 * Working-copy state of a file, derived from `repository.status` (the lore op).
 * Maps the status flags (action + dirty/conflict) onto UE's notion of state.
 */
namespace ELoreWorkingCopyState
{
	enum Type
	{
		Unknown,
		Unchanged,
		Added,
		Modified,
		Deleted,
		Conflicted,
		NotControlled,
	};
}

/**
 * Advisory lock state, orthogonal to the working-copy state. Derived from
 * `lock.file_status` / `lock.file_query` + the current identity. This is what
 * drives the "checked out by me" vs "checked out by other" overlay badges.
 */
namespace ELoreLockState
{
	enum Type
	{
		Unknown,
		NotLocked,
		LockedByMe,
		LockedByOther,
	};
}

/**
 * FLoreSourceControlState — per-asset source-control state the editor caches and
 * renders as Content Browser overlay icons. The provider holds one of these per
 * absolute filename in its StateCache; the Workers populate them from lore op
 * results (status + lock).
 *
 * Overlay mapping (see GetIcon in the .cpp):
 *   - LockedByMe        -> "checked out by me"
 *   - LockedByOther     -> "checked out by other" + tooltip with LockUser
 *   - Modified/Added    -> "modified"
 *   - bNewerVersionOnServer -> "not at head / out of date"
 */
class FLoreSourceControlState : public ISourceControlState
{
public:
	explicit FLoreSourceControlState(const FString& InLocalFilename)
		: LocalFilename(InLocalFilename)
	{
	}

	//~ Begin ISourceControlState interface
	virtual int32 GetHistorySize() const override;
	virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> GetHistoryItem(int32 HistoryIndex) const override;
	virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FindHistoryRevision(int32 RevisionNumber) const override;
	virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FindHistoryRevision(const FString& InRevision) const override;
	virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> GetCurrentRevision() const override;
	virtual FResolveInfo GetResolveInfo() const override;
#if SOURCE_CONTROL_WITH_SLATE
	virtual FSlateIcon GetIcon() const override;
#endif
	virtual FText GetDisplayName() const override;
	virtual FText GetDisplayTooltip() const override;
	virtual const FString& GetFilename() const override;
	virtual const FDateTime& GetTimeStamp() const override;
	virtual bool CanCheckIn() const override;
	virtual bool CanCheckout() const override;
	virtual bool IsCheckedOut() const override;
	virtual bool IsCheckedOutOther(FString* Who = nullptr) const override;
	virtual bool IsCheckedOutInOtherBranch(const FString& CurrentBranch = FString()) const override;
	virtual bool IsModifiedInOtherBranch(const FString& CurrentBranch = FString()) const override;
	virtual bool IsCheckedOutOrModifiedInOtherBranch(const FString& CurrentBranch = FString()) const override;
	virtual TArray<FString> GetCheckedOutBranches() const override;
	virtual FString GetOtherUserBranchCheckedOuts() const override;
	virtual bool GetOtherBranchHeadModification(FString& HeadBranchOut, FString& ActionOut, int32& HeadChangeListOut) const override;
	virtual bool IsCurrent() const override;
	virtual bool IsSourceControlled() const override;
	virtual bool IsAdded() const override;
	virtual bool IsDeleted() const override;
	virtual bool IsIgnored() const override;
	virtual bool CanEdit() const override;
	virtual bool CanDelete() const override;
	virtual bool IsUnknown() const override;
	virtual bool IsModified() const override;
	virtual bool CanAdd() const override;
	virtual bool IsConflicted() const override;
	virtual bool CanRevert() const override;
	//~ End ISourceControlState interface

public:
	/** Absolute filename this state represents (the StateCache key). */
	FString LocalFilename;

	/** Working-copy state from `repository.status`. */
	ELoreWorkingCopyState::Type WorkingCopyState = ELoreWorkingCopyState::Unknown;

	/** Advisory lock state from `lock.file_status`/`lock.file_query`. */
	ELoreLockState::Type LockState = ELoreLockState::Unknown;

	/** Owner (identity) of a held lock, when LockState == LockedByOther/LockedByMe. */
	FString LockUser;

	/** True when the server holds a newer revision than the working tree (out of date). */
	bool bNewerVersionOnServer = false;

	/** Whether lock-based check-out is the active workflow for this state. */
	bool bUsingLocking = true;

	/** Timestamp of the last update to this state. */
	FDateTime TimeStamp = FDateTime::MinValue();
};
