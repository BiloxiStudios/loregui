// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlCommand.h"
#include "Modules/ModuleManager.h"

FLoreSourceControlCommand::FLoreSourceControlCommand(
	const FSourceControlOperationRef& InOperation,
	const FLoreSourceControlWorkerRef& InWorker,
	const FSourceControlOperationComplete& InOperationCompleteDelegate)
	: Operation(InOperation)
	, Worker(InWorker)
	, OperationCompleteDelegate(InOperationCompleteDelegate)
{
}

bool FLoreSourceControlCommand::DoWork()
{
	bCommandSuccessful = Worker->Execute(*this);
	bExecuteProcessed.Increment();
	return bCommandSuccessful;
}

void FLoreSourceControlCommand::DoThreadedWork()
{
	Concurrency = EConcurrency::Asynchronous;
	DoWork();
}

void FLoreSourceControlCommand::Abandon()
{
	bCommandSuccessful = false;
	bExecuteProcessed.Increment();
}

void FLoreSourceControlCommand::Cancel()
{
	bCancelledCounter.Increment();
}

bool FLoreSourceControlCommand::IsCanceled() const
{
	return bCancelledCounter.GetValue() > 0;
}

ECommandResult::Type FLoreSourceControlCommand::ReturnResults()
{
	// Push worker results into the provider's state cache (game thread).
	Worker->UpdateStates();

	// Fire the completion delegate so the editor's source-control UI updates.
	const ECommandResult::Type Result = bCommandSuccessful ? ECommandResult::Succeeded : ECommandResult::Failed;
	OperationCompleteDelegate.ExecuteIfBound(Operation, Result);

	return Result;
}
