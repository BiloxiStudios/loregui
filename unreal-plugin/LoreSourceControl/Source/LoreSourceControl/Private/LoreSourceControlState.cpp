// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlState.h"

#define LOCTEXT_NAMESPACE "LoreSourceControl.State"

// ---------------------------------------------------------------------------
// History accessors — populated by FLoreHistoryWorker (file.history lore op).
// ---------------------------------------------------------------------------

int32 FLoreSourceControlState::GetHistorySize() const
{
	return History.Num();
}

TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FLoreSourceControlState::GetHistoryItem(int32 HistoryIndex) const
{
	if (History.IsValidIndex(HistoryIndex))
	{
		return History[HistoryIndex];
	}
	return nullptr;
}

TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FLoreSourceControlState::FindHistoryRevision(int32 RevisionNumber) const
{
	for (const TSharedRef<FLoreSourceControlRevision, ESPMode::ThreadSafe>& Rev : History)
	{
		if (Rev->GetRevisionNumber() == RevisionNumber)
		{
			return Rev;
		}
	}
	return nullptr;
}

TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FLoreSourceControlState::FindHistoryRevision(const FString& InRevision) const
{
	for (const TSharedRef<FLoreSourceControlRevision, ESPMode::ThreadSafe>& Rev : History)
	{
		if (Rev->GetRevision() == InRevision)
		{
			return Rev;
		}
	}
	return nullptr;
}

TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FLoreSourceControlState::GetCurrentRevision() const
{
	// Current revision = the newest history entry (index 0 — file.history returns
	// entries newest-first). Returns nullptr when history hasn't been fetched yet.
	if (History.Num() > 0)
	{
		return History[0];
	}
	return nullptr;
}

FResolveInfo FLoreSourceControlState::GetResolveInfo() const
{
	return FResolveInfo();
}

// ---------------------------------------------------------------------------
// Content Browser icon — lock state dominates, then modification, then out-of-date.
// ---------------------------------------------------------------------------

#if SOURCE_CONTROL_WITH_SLATE
FSlateIcon FLoreSourceControlState::GetIcon() const
{
	// Map our state onto the editor's revision-control overlay icon set. We reuse
	// the engine's standard RevisionControl brushes so the badges look native and
	// re-theme with the editor.
	//
	// Priority: lock state dominates (it's the interactive workflow), then local
	// modification, then out-of-date.
	switch (LockState)
	{
	case ELoreLockState::LockedByMe:
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.CheckedOut");
	case ELoreLockState::LockedByOther:
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.CheckedOutByOtherUser");
	default:
		break;
	}

	if (bNewerVersionOnServer)
	{
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.NotAtHeadRevision");
	}

	switch (WorkingCopyState)
	{
	case ELoreWorkingCopyState::Added:
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.OpenForAdd");
	case ELoreWorkingCopyState::Modified:
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.CheckedOut");
	case ELoreWorkingCopyState::Deleted:
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.MarkedForDelete");
	case ELoreWorkingCopyState::Conflicted:
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.Conflicted");
	case ELoreWorkingCopyState::NotControlled:
		return FSlateIcon(FAppStyle::GetAppStyleSetName(), "RevisionControl.NotInDepot");
	default:
		break;
	}

	return FSlateIcon();
}
#endif

// ---------------------------------------------------------------------------
// Display name / tooltip
// ---------------------------------------------------------------------------

FText FLoreSourceControlState::GetDisplayName() const
{
	switch (LockState)
	{
	case ELoreLockState::LockedByMe:
		return LOCTEXT("CheckedOut", "Checked out");
	case ELoreLockState::LockedByOther:
		return FText::Format(LOCTEXT("CheckedOutOther", "Checked out by: {0}"), FText::FromString(LockUser));
	default:
		break;
	}

	if (bNewerVersionOnServer)
	{
		return LOCTEXT("NotCurrent", "Not at latest revision");
	}

	switch (WorkingCopyState)
	{
	case ELoreWorkingCopyState::Unchanged:    return LOCTEXT("Unchanged", "Unchanged");
	case ELoreWorkingCopyState::Added:        return LOCTEXT("Added", "Added");
	case ELoreWorkingCopyState::Modified:     return LOCTEXT("Modified", "Modified");
	case ELoreWorkingCopyState::Deleted:      return LOCTEXT("Deleted", "Deleted");
	case ELoreWorkingCopyState::Conflicted:   return LOCTEXT("Conflicted", "Conflicted");
	case ELoreWorkingCopyState::NotControlled:return LOCTEXT("NotControlled", "Not under source control");
	default:                                  return LOCTEXT("Unknown", "Unknown");
	}
}

FText FLoreSourceControlState::GetDisplayTooltip() const
{
	if (LockState == ELoreLockState::LockedByOther)
	{
		return FText::Format(LOCTEXT("LockedByTooltip", "Locked by {0} in Lore"), FText::FromString(LockUser));
	}
	if (LockState == ELoreLockState::LockedByMe)
	{
		return LOCTEXT("LockedByMeTooltip", "You hold the Lore lock on this file");
	}
	if (bNewerVersionOnServer)
	{
		return LOCTEXT("OutOfDateTooltip", "A newer revision exists on the Lore server; sync to update");
	}
	return GetDisplayName();
}

// ---------------------------------------------------------------------------
// File / timestamp
// ---------------------------------------------------------------------------

const FString& FLoreSourceControlState::GetFilename() const
{
	return LocalFilename;
}

const FDateTime& FLoreSourceControlState::GetTimeStamp() const
{
	return TimeStamp;
}

// ---------------------------------------------------------------------------
// Action predicates
// ---------------------------------------------------------------------------

bool FLoreSourceControlState::CanCheckIn() const
{
	// Can submit if we hold the lock (or aren't using locking) and there's a local change.
	const bool bHaveLock = (LockState == ELoreLockState::LockedByMe) || !bUsingLocking;
	return bHaveLock && IsModified() && !IsConflicted();
}

bool FLoreSourceControlState::CanCheckout() const
{
	if (!bUsingLocking)
	{
		return false; // No lock workflow -> nothing to check out.
	}
	// Can check out (acquire lock) when nobody else holds it and we don't already.
	return LockState == ELoreLockState::NotLocked;
}

bool FLoreSourceControlState::IsCheckedOut() const
{
	return LockState == ELoreLockState::LockedByMe;
}

bool FLoreSourceControlState::IsCheckedOutOther(FString* Who) const
{
	if (LockState == ELoreLockState::LockedByOther)
	{
		if (Who)
		{
			*Who = LockUser;
		}
		return true;
	}
	return false;
}

bool FLoreSourceControlState::IsCheckedOutInOtherBranch(const FString& /*CurrentBranch*/) const
{
	return false; // Cross-branch lock visibility is a future layer (not in MVP).
}

bool FLoreSourceControlState::IsModifiedInOtherBranch(const FString& /*CurrentBranch*/) const
{
	return false;
}

bool FLoreSourceControlState::IsCheckedOutOrModifiedInOtherBranch(const FString& /*CurrentBranch*/) const
{
	return false;
}

TArray<FString> FLoreSourceControlState::GetCheckedOutBranches() const
{
	return TArray<FString>();
}

FString FLoreSourceControlState::GetOtherUserBranchCheckedOuts() const
{
	return FString();
}

bool FLoreSourceControlState::GetOtherBranchHeadModification(FString& /*HeadBranchOut*/, FString& /*ActionOut*/, int32& /*HeadChangeListOut*/) const
{
	return false;
}

bool FLoreSourceControlState::IsCurrent() const
{
	return !bNewerVersionOnServer;
}

bool FLoreSourceControlState::IsSourceControlled() const
{
	return WorkingCopyState != ELoreWorkingCopyState::NotControlled
		&& WorkingCopyState != ELoreWorkingCopyState::Unknown;
}

bool FLoreSourceControlState::IsAdded() const
{
	return WorkingCopyState == ELoreWorkingCopyState::Added;
}

bool FLoreSourceControlState::IsDeleted() const
{
	return WorkingCopyState == ELoreWorkingCopyState::Deleted;
}

bool FLoreSourceControlState::IsIgnored() const
{
	return false;
}

bool FLoreSourceControlState::CanEdit() const
{
	// In a lock workflow the asset is editable once we hold the lock.
	return !bUsingLocking || LockState == ELoreLockState::LockedByMe;
}

bool FLoreSourceControlState::CanDelete() const
{
	return IsSourceControlled() && !IsCheckedOutOther();
}

bool FLoreSourceControlState::IsUnknown() const
{
	return WorkingCopyState == ELoreWorkingCopyState::Unknown;
}

bool FLoreSourceControlState::IsModified() const
{
	switch (WorkingCopyState)
	{
	case ELoreWorkingCopyState::Added:
	case ELoreWorkingCopyState::Modified:
	case ELoreWorkingCopyState::Deleted:
	case ELoreWorkingCopyState::Conflicted:
		return true;
	default:
		return false;
	}
}

bool FLoreSourceControlState::CanAdd() const
{
	return WorkingCopyState == ELoreWorkingCopyState::NotControlled;
}

bool FLoreSourceControlState::IsConflicted() const
{
	return WorkingCopyState == ELoreWorkingCopyState::Conflicted;
}

bool FLoreSourceControlState::CanRevert() const
{
	// Revert is meaningful if we have a local change or hold a lock.
	return IsModified() || LockState == ELoreLockState::LockedByMe;
}

#undef LOCTEXT_NAMESPACE
