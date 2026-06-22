// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "LoreSourceControlState.h"

class FLorevmFfi;
class FJsonObject;

/**
 * Stateless helpers shared by the Workers: path mapping between UE's absolute
 * filenames and lore's repo-relative paths, and translation of lore op result
 * JSON into FLoreSourceControlState.
 *
 * All op-id strings + JSON shapes are confined to this file and Operations.cpp —
 * the thin adapter seam. Nothing else in the plugin knows the lore contract.
 */
namespace LoreSourceControlUtils
{
	/** UE absolute filename -> repo-relative path (lore uses '/'-separated, repo-root-relative). */
	FString ToRepoRelative(const FString& RepositoryRoot, const FString& AbsoluteFilename);

	/** repo-relative path -> UE absolute filename. */
	FString ToAbsolute(const FString& RepositoryRoot, const FString& RelativePath);

	/**
	 * Run `repository.status` + `lock.file_status` for the given files and fold the
	 * results into one FLoreSourceControlState per file. `Identity` classifies a
	 * lock as mine vs other. Appends to OutStates. Returns false on a bridge error.
	 */
	bool QueryFileStates(FLorevmFfi& Ffi,
	                     const FString& RepositoryRoot,
	                     const FString& BranchName,
	                     const FString& Identity,
	                     const TArray<FString>& AbsoluteFiles,
	                     TArray<FLoreSourceControlState>& OutStates,
	                     TArray<FString>& OutErrorMessages);

	/** Map a `repository.status` file "action" + flags onto a working-copy state. */
	ELoreWorkingCopyState::Type MapWorkingCopyState(const FString& Action, bool bConflict);
}
