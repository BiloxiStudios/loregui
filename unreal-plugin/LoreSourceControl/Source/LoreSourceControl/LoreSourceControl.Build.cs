// Copyright Biloxi Studios Inc. MIT License. SBAI-4086.

using System.IO;
using UnrealBuildTool;

public class LoreSourceControl : ModuleRules
{
	public LoreSourceControl(ReadOnlyTargetRules Target) : base(Target)
	{
		PCHUsage = ModuleRules.PCHUsageMode.UseExplicitOrSharedPCHs;

		// Expose the Public/ directory so external code (and tests) can include
		// LoreSourceControlDeveloperSettings.h without a full Private/ include.
		PublicIncludePaths.AddRange(
			new string[]
			{
				Path.Combine(ModuleDirectory, "Public"),
			}
		);

		PrivateDependencyModuleNames.AddRange(
			new string[]
			{
				"Core",
				"CoreUObject",
				"Engine",              // UDeveloperSettings base class lives in Engine
				"DeveloperSettings",   // ISettingsModule + UDeveloperSettings registration
				"Json",                // FJsonSerializer for the FFI request/response JSON
				"Slate",
				"SlateCore",
				"InputCore",
				"SourceControl",       // ISourceControlProvider / State / Operations
				"Projects",            // IPluginManager (locate the staged cdylib)
			}
		);

		if (Target.bBuildEditor)
		{
			PrivateDependencyModuleNames.AddRange(
				new string[]
				{
					"UnrealEd",
					"EditorFramework",
					"Settings",            // ISettingsModule for Project Settings panel entry
				}
			);
		}

		// ------------------------------------------------------------------
		// lorevm-ffi shared library (built from crates/lorevm-ffi).
		//
		// We DO NOT link an import library at build time — LorevmFfi.cpp loads the
		// shared lib at runtime via FPlatformProcess::GetDllHandle so the editor
		// still starts if the lib is absent (the provider reports "unavailable").
		//
		// We DO stage the shared library next to the plugin as a runtime
		// dependency, so packaged/installed builds carry it. Drop the platform
		// library you built into:
		//     Binaries/ThirdParty/LorevmFfi/<Platform>/<libname>
		// (Win64/lorevm_ffi.dll, Mac/liblorevm_ffi.dylib, Linux/liblorevm_ffi.so)
		// See docs/BUILD.md for how to produce it with `cargo build -p lorevm-ffi`.
		// ------------------------------------------------------------------
		string ThirdPartyBase = Path.Combine(PluginDirectory, "Binaries", "ThirdParty", "LorevmFfi");

		string PlatformDir;
		string LibFileName;
		if (Target.Platform == UnrealTargetPlatform.Win64)
		{
			PlatformDir = "Win64";
			LibFileName = "lorevm_ffi.dll";
		}
		else if (Target.Platform == UnrealTargetPlatform.Mac)
		{
			PlatformDir = "Mac";
			LibFileName = "liblorevm_ffi.dylib";
		}
		else
		{
			PlatformDir = "Linux";
			LibFileName = "liblorevm_ffi.so";
		}

		string LibPath = Path.Combine(ThirdPartyBase, PlatformDir, LibFileName);

		// Stage the library into the packaged Binaries dir if present. We don't fail
		// the build if it's missing — a developer may point at it via LOREVM_FFI_LIB.
		if (File.Exists(LibPath))
		{
			RuntimeDependencies.Add("$(BinaryOutputDir)/" + LibFileName, LibPath);
		}
	}
}
