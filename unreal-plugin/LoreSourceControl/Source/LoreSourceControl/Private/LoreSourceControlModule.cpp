// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlModule.h"
#include "LoreSourceControlOperations.h"
#include "LoreSourceControlLog.h"

#include "ISourceControlModule.h"
#include "Modules/ModuleManager.h"

#define LOCTEXT_NAMESPACE "LoreSourceControl"

template <typename WorkerType>
static FLoreSourceControlWorkerRef CreateLoreWorker()
{
	return MakeShared<WorkerType, ESPMode::ThreadSafe>();
}

void FLoreSourceControlModule::RegisterWorkers()
{
	// Map UE source-control operation names to our per-op Workers. The operation
	// name is the FName each ISourceControlOperation reports via GetName().
	Provider.RegisterWorker("Connect",      FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreConnectWorker>));
	Provider.RegisterWorker("UpdateStatus", FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreUpdateStatusWorker>));
	Provider.RegisterWorker("CheckOut",     FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreCheckOutWorker>));
	Provider.RegisterWorker("MarkForAdd",   FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreMarkForAddWorker>));
	Provider.RegisterWorker("Delete",       FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreDeleteWorker>));
	Provider.RegisterWorker("CheckIn",      FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreCheckInWorker>));
	Provider.RegisterWorker("Revert",       FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreRevertWorker>));
	Provider.RegisterWorker("Sync",         FGetLoreSourceControlWorker::CreateStatic(&CreateLoreWorker<FLoreSyncWorker>));
}

void FLoreSourceControlModule::StartupModule()
{
	Settings.LoadSettings();
	RegisterWorkers();

	// Register our provider so it shows up in the editor's Revision Control menu.
	IModularFeatures::Get().RegisterModularFeature("SourceControl", &Provider);

	UE_LOG(LogSourceControl, Log, TEXT("[Lore] LoreSourceControl module started"));
}

void FLoreSourceControlModule::ShutdownModule()
{
	Provider.Close();
	IModularFeatures::Get().UnregisterModularFeature("SourceControl", &Provider);

	UE_LOG(LogSourceControl, Log, TEXT("[Lore] LoreSourceControl module shut down"));
}

#undef LOCTEXT_NAMESPACE

IMPLEMENT_MODULE(FLoreSourceControlModule, LoreSourceControl);
