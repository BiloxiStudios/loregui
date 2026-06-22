# Building the Lore Source Control plugin

Two artifacts: (1) the `lorevm-ffi` shared library, and (2) the UE plugin itself.

## 1. Build the lorevm-ffi shared library (the cdylib)

The plugin links **our** lore binding, not Epic's CLI. The binding is shipped as
a C-ABI cdylib in the LoreGUI repo at `crates/lorevm-ffi`.

```bash
# From the LoreGUI repo root:
cargo build -p lorevm-ffi --release
```

Output (per host platform):

| Platform | File | Path |
|----------|------|------|
| Linux    | `liblorevm_ffi.so`    | `target/release/liblorevm_ffi.so` |
| macOS    | `liblorevm_ffi.dylib` | `target/release/liblorevm_ffi.dylib` |
| Windows  | `lorevm_ffi.dll`      | `target/release/lorevm_ffi.dll` |

Verify the exported C symbols match what the plugin expects:

```bash
# Linux:
nm -D target/release/liblorevm_ffi.so | grep lorevm_ffi
# expect: lorevm_ffi_open, lorevm_ffi_call, lorevm_ffi_close,
#         lorevm_ffi_string_free, lorevm_ffi_abi_version
```

The ABI version string the library reports (`lorevm-ffi/1`) must match
`LOREVM_FFI_ABI_MAJOR_EXPECTED` in `Source/LoreSourceControl/Private/Ffi/lorevm_ffi.h`.
The plugin asserts this at load and refuses a mismatched major.

### Cross-compiling

The cdylib must match the UE target platform/architecture. Cross-compile with a
Rust target (e.g. `cargo build -p lorevm-ffi --release --target x86_64-pc-windows-msvc`)
or build natively on each platform you ship. On macOS, build a universal binary
(`lipo`) or one per arch. Code-sign/notarize as your distribution requires.

## 2. Install + build the plugin

1. Copy `unreal-plugin/LoreSourceControl/` into your UE project's `Plugins/`
   directory (so you have `<Project>/Plugins/LoreSourceControl/LoreSourceControl.uplugin`).

2. Make the shared library discoverable. The loader (`FLorevmFfi::Load`) tries, in
   order:
   1. the explicit path in `SourceControlSettings.ini`
      (`[LoreSourceControl.LoreSourceControlSettings] LorevmFfiLibPath=...`),
   2. the `LOREVM_FFI_LIB` environment variable,
   3. `Plugins/LoreSourceControl/Binaries/ThirdParty/LorevmFfi/<Platform>/<libname>`,
   4. the bare library name (OS loader search path).

   The conventional spot is (3). Create it and drop your built library in:

   ```
   Plugins/LoreSourceControl/Binaries/ThirdParty/LorevmFfi/Linux/liblorevm_ffi.so
   Plugins/LoreSourceControl/Binaries/ThirdParty/LorevmFfi/Mac/liblorevm_ffi.dylib
   Plugins/LoreSourceControl/Binaries/ThirdParty/LorevmFfi/Win64/lorevm_ffi.dll
   ```

   `LoreSourceControl.Build.cs` registers that file as a `RuntimeDependency` so
   packaged builds carry it.

3. Generate project files and build (e.g. right-click the `.uproject` →
   *Generate Visual Studio project files*, then build; or `RunUAT BuildPlugin`).

4. Launch the editor. **Settings → Revision Control → Connect to Revision
   Control…**, choose **Lore (StudioBrain)**, and connect. The status panel shows
   the loaded ABI version and the repository root.

## Engine versions

Baseline is **UE 5.3+** — the `ISourceControlProvider` surface implemented here
matches 5.3. To target older 5.x or 4.27, re-introduce the engine-version macro
gates (the reference plugin's `LoreSourceControlVersion.h` pattern:
`ENGINE_MAJOR_VERSION`/`ENGINE_MINOR_VERSION` guards around the few methods that
changed signature across versions — e.g. `GetStatus`, `GetIcon`/`GetIconName`,
`GetResolveInfo`/`GetBaseRevForMerge`, the changelist `Execute` overload). The MVP
keeps a single clean 5.3+ surface for reviewability.

## Dev / no-server mode

For a local repo with no lore server, set in `SourceControlSettings.ini`:

```ini
[LoreSourceControl.LoreSourceControlSettings]
UseInMemory=True
Offline=True
Identity=you@example.com
```

These map straight to `lorevm_ffi_open`'s request JSON
(`{"dir","in_memory","offline","identity"}`).
