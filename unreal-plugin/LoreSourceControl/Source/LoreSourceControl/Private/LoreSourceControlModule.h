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
	/** Register one worker factory under an operation name. */
	void RegisterWorkers();

	FLoreSourceControlSettings Settings;
	FLoreSourceControlProvider Provider;
};
