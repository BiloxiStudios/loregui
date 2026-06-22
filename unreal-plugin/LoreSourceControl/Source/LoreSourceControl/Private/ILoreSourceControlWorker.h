// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"

class FLoreSourceControlCommand;

/**
 * ILoreSourceControlWorker — a unit of source-control work, one per
 * ISourceControlOperation the editor can request. The Provider maps an
 * operation name (e.g. "CheckOut") to the worker that performs it.
 *
 * Each worker's Execute() runs on a worker thread and drives the lore op(s) via
 * the command's FLorevmFfi; UpdateStates() runs on the game thread to push the
 * results into the provider's state cache.
 */
class ILoreSourceControlWorker
{
public:
	virtual ~ILoreSourceControlWorker() = default;

	/** Name of the operation this worker handles, matching ISourceControlOperation::GetName. */
	virtual FName GetName() const = 0;

	/** Runs on a source-control worker thread. Returns success. */
	virtual bool Execute(FLoreSourceControlCommand& InCommand) = 0;

	/** Runs on the game thread to push results into the provider's state cache. */
	virtual bool UpdateStates() const = 0;
};

typedef TSharedRef<ILoreSourceControlWorker, ESPMode::ThreadSafe> FLoreSourceControlWorkerRef;
