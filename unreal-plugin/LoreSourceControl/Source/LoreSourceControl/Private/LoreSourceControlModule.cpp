// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlModule.h"
#include "LoreSourceControlOperations.h"
#include "LoreSourceControlLog.h"
#include "LoreSourceControlDeveloperSettings.h"

#include "ISourceControlModule.h"
#include "Modules/ModuleManager.h"

#if WITH_EDITOR
#include "ISettingsModule.h"
#include "ISettingsSection.h"
#endif

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
	// GetHistory is handled as an explicit provider call (no ISourceControlOperation
	// name for it in UE 5.3's standard surface); FLoreHistoryWorker is constructed
	// directly by the provider when history is requested. UE-BUILD-PENDING: confirm
	// the editor's history panel invocation path and wire accordingly.
}

void FLoreSourceControlModule::RegisterSettings()
{
#if WITH_EDITOR
	if (ISettingsModule* SettingsModule = FModuleManager::GetModulePtr<ISettingsModule>("Settings"))
	{
		// Register ULoreSourceControlDeveloperSettings in the editor's Project
		// Settings panel under Plugins > Lore Source Control. The section name
		// matches the CDO's GetSectionName() so the ini round-trip is stable.
		SettingsModule->RegisterSettings(
			TEXT("Project"),    // container
			TEXT("Plugins"),    // category
			TEXT("LoreSourceControl"), // section
			LOCTEXT("LoreSettingsName", "Lore Source Control"),
			LOCTEXT("LoreSettingsDesc",
				"Configure the Lore source-control provider: lorevm-ffi library path, "
				"server URL, commit identity, and dev/offline mode."),
			GetMutableDefault<ULoreSourceControlDeveloperSettings>()
		);
	}
#endif
}

void FLoreSourceControlModule::UnregisterSettings()
{
#if WITH_EDITOR
	if (ISettingsModule* SettingsModule = FModuleManager::GetModulePtr<ISettingsModule>("Settings"))
	{
		SettingsModule->UnregisterSettings(TEXT("Project"), TEXT("Plugins"), TEXT("LoreSourceControl"));
	}
#endif
}

void FLoreSourceControlModule::StartupModule()
{
	Settings.LoadSettings();
	RegisterWorkers();
	RegisterSettings();

	// Register our provider so it shows up in the editor's Revision Control menu.
	IModularFeatures::Get().RegisterModularFeature("SourceControl", &Provider);

	UE_LOG(LogSourceControl, Log, TEXT("[Lore] LoreSourceControl module started"));
}

void FLoreSourceControlModule::ShutdownModule()
{
	UnregisterSettings();
	Provider.Close();
	IModularFeatures::Get().UnregisterModularFeature("SourceControl", &Provider);

	UE_LOG(LogSourceControl, Log, TEXT("[Lore] LoreSourceControl module shut down"));
}

#undef LOCTEXT_NAMESPACE

IMPLEMENT_MODULE(FLoreSourceControlModule, LoreSourceControl);
