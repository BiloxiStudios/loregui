// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#include "LoreSourceControlSettings.h"
#include "SourceControlHelpers.h"
#include "Misc/ConfigCacheIni.h"

namespace
{
	const TCHAR* SettingsSection = TEXT("LoreSourceControl.LoreSourceControlSettings");
}

void FLoreSourceControlSettings::LoadSettings()
{
	FScopeLock Lock(&CriticalSection);
	const FString& IniFile = SourceControlHelpers::GetSettingsIni();
	GConfig->GetString(SettingsSection, TEXT("LorevmFfiLibPath"), LorevmFfiLibPath, IniFile);
	GConfig->GetBool(SettingsSection, TEXT("UseInMemory"), bUseInMemory, IniFile);
	GConfig->GetBool(SettingsSection, TEXT("Offline"), bOffline, IniFile);
	GConfig->GetString(SettingsSection, TEXT("Identity"), Identity, IniFile);
}

void FLoreSourceControlSettings::SaveSettings() const
{
	FScopeLock Lock(&CriticalSection);
	const FString& IniFile = SourceControlHelpers::GetSettingsIni();
	GConfig->SetString(SettingsSection, TEXT("LorevmFfiLibPath"), *LorevmFfiLibPath, IniFile);
	GConfig->SetBool(SettingsSection, TEXT("UseInMemory"), bUseInMemory, IniFile);
	GConfig->SetBool(SettingsSection, TEXT("Offline"), bOffline, IniFile);
	GConfig->SetString(SettingsSection, TEXT("Identity"), *Identity, IniFile);
}
