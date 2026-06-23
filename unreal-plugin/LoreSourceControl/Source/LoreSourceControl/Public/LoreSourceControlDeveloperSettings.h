// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

#pragma once

#include "CoreMinimal.h"
#include "Engine/DeveloperSettings.h"

#include "LoreSourceControlDeveloperSettings.generated.h"

/**
 * ULoreSourceControlDeveloperSettings — project-level Lore source-control
 * settings exposed through the editor's Project Settings panel
 * (Edit → Project Settings → Plugins → Lore Source Control).
 *
 * Stored in DefaultSourceControlSettings.ini; complements the per-developer
 * FLoreSourceControlSettings (SourceControlSettings.ini). The provider merges
 * both at startup: the per-developer ini values take priority over the project
 * defaults, so a developer can override the server URL for local dev without
 * touching the shared project ini.
 *
 * Thread-safety: UObject subsystem — read on the game thread only. Workers
 * snapshot what they need at command-build time.
 */
UCLASS(Config = SourceControlSettings, DefaultConfig,
       meta = (DisplayName = "Lore Source Control"))
class LORESOURCECONTROL_API ULoreSourceControlDeveloperSettings : public UDeveloperSettings
{
    GENERATED_BODY()

public:
    ULoreSourceControlDeveloperSettings();

    //~ Begin UDeveloperSettings
    virtual FName GetContainerName() const override { return FName("Project"); }
    virtual FName GetCategoryName()  const override { return FName("Plugins"); }
    virtual FName GetSectionName()   const override { return FName("LoreSourceControl"); }
    //~ End UDeveloperSettings

    // -------------------------------------------------------------------------
    // Library
    // -------------------------------------------------------------------------

    /**
     * Explicit path to the lorevm-ffi shared library.
     * Empty = auto-discover (LOREVM_FFI_LIB env var → plugin Binaries dir → OS path).
     * Use this in DefaultSourceControlSettings.ini to pin a CI-provisioned build.
     */
    UPROPERTY(Config, EditAnywhere, Category = "Library",
              meta = (DisplayName = "lorevm-ffi library path (optional override)",
                      ToolTip = "Absolute path to liblorevm_ffi.so / lorevm_ffi.dll / "
                                "liblorevm_ffi.dylib. Leave empty for auto-discovery."))
    FString LoreVmBinaryPath;

    // -------------------------------------------------------------------------
    // Server
    // -------------------------------------------------------------------------

    /**
     * Lore server base URL for this project.
     * Passed to the warm-handle open request (lorevm_ffi_open's "dir" is still the
     * local working dir; this is the remote server the repo tracks).
     * Default: "http://localhost:17171" for local dev without a shared server.
     */
    UPROPERTY(Config, EditAnywhere, Category = "Server",
              meta = (DisplayName = "Lore server URL",
                      ToolTip = "Base URL of the Lore server this project tracks. "
                                "Used when the provider opens a warm handle. "
                                "e.g. \"http://lore-server.example.com:17171\"."))
    FString ServerUrl = TEXT("http://localhost:17171");

    // -------------------------------------------------------------------------
    // Identity
    // -------------------------------------------------------------------------

    /**
     * Default commit identity for this project (email or display name).
     * Per-developer override: set Identity in SourceControlSettings.ini
     * (the legacy FLoreSourceControlSettings). The provider prefers the
     * per-developer value when non-empty.
     */
    UPROPERTY(Config, EditAnywhere, Category = "Identity",
              meta = (DisplayName = "Identity (email / display name)",
                      ToolTip = "Commit identity used to classify locks (mine vs. other) "
                                "and to author commits. Per-developer override available "
                                "in SourceControlSettings.ini."))
    FString Identity;

    // -------------------------------------------------------------------------
    // Dev / Offline
    // -------------------------------------------------------------------------

    /**
     * Open the warm handle in in-memory mode (no on-disk persistence).
     * Maps to the "in_memory" field of lorevm_ffi_open's request JSON.
     * Useful for CI or headless tests with no on-disk lore repo.
     */
    UPROPERTY(Config, EditAnywhere, Category = "Dev / Offline",
              meta = (DisplayName = "In-memory mode",
                      ToolTip = "Open the lore handle with no on-disk storage. "
                                "Use for CI / test environments."))
    bool bUseInMemory = false;

    /**
     * Open the warm handle in offline mode (all network ops disabled).
     * Maps to the "offline" field of lorevm_ffi_open's request JSON.
     * Status / history still work from the local repo; lock/sync/push return errors.
     */
    UPROPERTY(Config, EditAnywhere, Category = "Dev / Offline",
              meta = (DisplayName = "Offline mode",
                      ToolTip = "Disable all network ops. Status and history work from "
                                "the local repo; lock/sync/push return errors."))
    bool bOffline = false;

    // -------------------------------------------------------------------------
    // Convenience
    // -------------------------------------------------------------------------

    /** CDO accessor. Never null after module load. Game thread only. */
    static const ULoreSourceControlDeveloperSettings* Get()
    {
        return GetDefault<ULoreSourceControlDeveloperSettings>();
    }
};
