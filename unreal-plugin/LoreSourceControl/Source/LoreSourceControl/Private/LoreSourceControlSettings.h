// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "Misc/ScopeLock.h"

/**
 * FLoreSourceControlSettings — user-facing knobs persisted to the editor's
 * SourceControlSettings.ini. The MVP keeps this deliberately small:
 *
 *  - LorevmFfiLibPath: optional explicit path to the lorevm-ffi shared library.
 *    Empty -> the loader auto-discovers it (env var, then plugin Binaries dir).
 *  - bUseInMemory / bOffline: forwarded to lorevm_ffi_open's request JSON. Useful
 *    for local/dev repos without a server.
 *  - Identity: commit identity forwarded to the open request (may be empty).
 *
 * Thread-safe: settings are read on worker threads and written on the game thread.
 */
class FLoreSourceControlSettings
{
public:
	FString GetLorevmFfiLibPath() const { FScopeLock Lock(&CriticalSection); return LorevmFfiLibPath; }
	void SetLorevmFfiLibPath(const FString& In) { FScopeLock Lock(&CriticalSection); LorevmFfiLibPath = In; }

	bool GetUseInMemory() const { FScopeLock Lock(&CriticalSection); return bUseInMemory; }
	void SetUseInMemory(bool In) { FScopeLock Lock(&CriticalSection); bUseInMemory = In; }

	bool GetOffline() const { FScopeLock Lock(&CriticalSection); return bOffline; }
	void SetOffline(bool In) { FScopeLock Lock(&CriticalSection); bOffline = In; }

	FString GetIdentity() const { FScopeLock Lock(&CriticalSection); return Identity; }
	void SetIdentity(const FString& In) { FScopeLock Lock(&CriticalSection); Identity = In; }

	/** Load/save from the editor's source control ini. Implemented in the .cpp. */
	void LoadSettings();
	void SaveSettings() const;

private:
	mutable FCriticalSection CriticalSection;
	FString LorevmFfiLibPath;
	bool bUseInMemory = false;
	bool bOffline = false;
	FString Identity;
};
