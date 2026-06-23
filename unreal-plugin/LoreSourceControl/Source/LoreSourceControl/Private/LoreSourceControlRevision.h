// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "ISourceControlRevision.h"

/**
 * FLoreSourceControlRevision — one entry in the per-file revision history
 * returned by the `file.history` lore op. Implements ISourceControlRevision so
 * the editor's source-control history panel and diff viewer can show it.
 *
 * Populated by FLoreHistoryWorker::Execute on the worker thread, then stored in
 * FLoreSourceControlState::History (game thread after UpdateStates).
 *
 * The `file.history` op JSON shape:
 *   { "entries": [{ "path", "repository", "revision", "revision_number",
 *                   "parents", "address", "size", "action" }, ...] }
 * where "action" is one of "keep" | "add" | "delete" | "move" | "copy".
 */
class FLoreSourceControlRevision : public ISourceControlRevision
{
public:
    explicit FLoreSourceControlRevision(const FString& InFilename)
        : LocalFilename(InFilename)
    {
    }

    //~ Begin ISourceControlRevision interface
    virtual bool Get(FString& InOutFilename) const override;
    virtual bool GetAnnotated(TArray<FAnnotationLine>& OutLines) const override;
    virtual bool GetAnnotated(FString& InOutFilename) const override;
    virtual const FString& GetFilename() const override;
    virtual int32 GetRevisionNumber() const override;
    virtual const FString& GetRevision() const override;
    virtual const FString& GetDescription() const override;
    virtual const FString& GetUserName() const override;
    virtual const FString& GetClientSpec() const override;
    virtual const FString& GetAction() const override;
    virtual TSharedPtr<ISourceControlRevision, ESPMode::ThreadSafe> GetBranchSource() const override;
    virtual const FDateTime& GetDate() const override;
    virtual int32 GetCheckInIdentifier() const override;
    virtual int32 GetFileSize() const override;
    //~ End ISourceControlRevision interface

public:
    /** Absolute local filename (matches the state this revision belongs to). */
    FString LocalFilename;

    /** Repo-relative path at this revision (may differ from LocalFilename on moves). */
    FString Path;

    /** Repository identifier string. */
    FString Repository;

    /** Lore revision hash (hex). This is the ISourceControlRevision "revision string". */
    FString RevisionHash;

    /** Sequential revision number (monotonically increasing in lore). */
    int32 RevisionNumber = 0;

    /** File size at this revision in bytes. */
    int64 FileSize = 0;

    /**
     * Action applied to the file at this revision.
     * Serialised as lowercase strings by lore: "keep"|"add"|"delete"|"move"|"copy".
     * Stored in the ISourceControlRevision "action" string.
     */
    FString Action;

    /**
     * Parent revision hashes (zero hashes are omitted by lore-vm).
     * Stored for display; ISourceControlRevision doesn't surface them directly.
     */
    TArray<FString> Parents;

    /**
     * Content address (lore CAS address) at this revision.
     * Not surfaced by ISourceControlRevision but useful for diff / get ops.
     */
    FString ContentAddress;

    /**
     * Timestamp of this revision.
     * lore's revision history event does not currently emit a timestamp;
     * this is set to FDateTime::MinValue() until `revision.info` is called
     * for the specific revision hash. See // UE-BUILD-PENDING note in .cpp.
     */
    FDateTime Timestamp = FDateTime::MinValue();
};
