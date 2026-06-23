// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "Modules/ModuleManager.h"
#include "LoreSourceControlSettings.h"
#include "LoreSourceControlProvider.h"

/**
 * FLoreSourceControlModule — editor module entry point. Owns the singleton
 * Settings + Provider and registers the provider with the editor's
 * ISourceControlModule so "Lore" appears in the Revision Control provider list.
 *
 * Settings surface (two complementary layers):
 *  - FLoreSourceControlSettings: per-developer ini (SourceControlSettings.ini),
 *    the lightweight thread-safe struct that predates UDeveloperSettings.
 *  - ULoreSourceControlDeveloperSettings: project-wide UObject settings shown in
 *    Edit → Project Settings → Plugins → Lore Source Control. Registered with
 *    ISettingsModule in StartupModule / UnregisterSettings in ShutdownModule.
 *
 * Provider Init() merges both: per-developer values take priority over the
 * project-wide CDO defaults.
 */
class FLoreSourceControlModule : public IModuleInterface
{
public:
	//~ Begin IModuleInterface
	virtual void StartupModule() override;
	virtual void ShutdownModule() override;
	//~ End IModuleInterface

	FLoreSourceControlSettings& AccessSettings() { return Settings; }
	const FLoreSourceControlSettings& GetSettings() const { return Settings; }

	FLoreSourceControlProvider& GetProvider() { return Provider; }
	const FLoreSourceControlProvider& GetProvider() const { return Provider; }

	static FLoreSourceControlModule& Get()
	{
		return FModuleManager::LoadModuleChecked<FLoreSourceControlModule>("LoreSourceControl");
	}

	static FLoreSourceControlModule* GetPtr()
	{
		return FModuleManager::GetModulePtr<FLoreSourceControlModule>("LoreSourceControl");
	}

private:
	/** Register per-op worker factories. */
	void RegisterWorkers();

	/** Register ULoreSourceControlDeveloperSettings with ISettingsModule (editor only). */
	void RegisterSettings();

	/** Unregister from ISettingsModule on shutdown. */
	void UnregisterSettings();

	FLoreSourceControlSettings Settings;
	FLoreSourceControlProvider Provider;
};
