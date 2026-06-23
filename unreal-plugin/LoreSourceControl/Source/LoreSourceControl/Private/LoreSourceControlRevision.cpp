// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlRevision.h"

// ---------------------------------------------------------------------------
// ISourceControlRevision interface
// ---------------------------------------------------------------------------

bool FLoreSourceControlRevision::Get(FString& InOutFilename) const
{
    // UE-BUILD-PENDING: a real implementation would call file.dump / storage.get_file
    // via FLorevmFfi to retrieve the file at this revision's content address into
    // a temp path, then set InOutFilename to that path. The shape is here so the
    // editor's diff viewer can call it; returning false tells it the op is unsupported.
    return false;
}

bool FLoreSourceControlRevision::GetAnnotated(TArray<FAnnotationLine>& /*OutLines*/) const
{
    // Lore does not currently provide line-level annotation / blame.
    return false;
}

bool FLoreSourceControlRevision::GetAnnotated(FString& /*InOutFilename*/) const
{
    return false;
}

const FString& FLoreSourceControlRevision::GetFilename() const
{
    return LocalFilename;
}

int32 FLoreSourceControlRevision::GetRevisionNumber() const
{
    return RevisionNumber;
}

const FString& FLoreSourceControlRevision::GetRevision() const
{
    return RevisionHash;
}

const FString& FLoreSourceControlRevision::GetDescription() const
{
    // UE-BUILD-PENDING: lore's file.history event carries revision_number + hash
    // but not the commit message. A follow-up could call `revision.info` for each
    // hash to populate a Description field. Returning an empty string here is valid
    // per ISourceControlRevision (the engine falls back to the revision string).
    static const FString Empty;
    return Empty;
}

const FString& FLoreSourceControlRevision::GetUserName() const
{
    // UE-BUILD-PENDING: same as GetDescription — needs `revision.info` per hash.
    static const FString Empty;
    return Empty;
}

const FString& FLoreSourceControlRevision::GetClientSpec() const
{
    static const FString Empty;
    return Empty;
}

const FString& FLoreSourceControlRevision::GetAction() const
{
    return Action;
}

TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> FLoreSourceControlRevision::GetBranchSource() const
{
    // MVP does not cross-branch revision linking.
    return nullptr;
}

const FDateTime& FLoreSourceControlRevision::GetDate() const
{
    // UE-BUILD-PENDING: lore's FileHistory event does not carry a timestamp;
    // populate via `revision.info` for each hash in a follow-up.
    return Timestamp;
}

int32 FLoreSourceControlRevision::GetCheckInIdentifier() const
{
    return RevisionNumber;
}

int32 FLoreSourceControlRevision::GetFileSize() const
{
    // FileSize is int64 in our struct but ISourceControlRevision returns int32.
    // Clamp to avoid signed overflow on large files.
    return static_cast<int32>(FMath::Min(FileSize, static_cast<int64>(TNumericLimits<int32>::Max())));
}
