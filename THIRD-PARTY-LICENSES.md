# Third-Party Licenses — LoreGUI

LoreGUI is licensed under the MIT License (see [`LICENSE`](LICENSE)). The
distributed desktop binary bundles third-party software whose attribution and
license texts are reproduced in the generated bundles below. See
[`NOTICE`](NOTICE) for the high-level summary.

These files are **generated** from the real dependency graph and re-verified in
CI (`.github/workflows/licenses.yml`), so they can't drift from what actually
ships.

| Bundle | Covers | Generator |
| --- | --- | --- |
| [THIRD-PARTY-LICENSES-RUST.md](THIRD-PARTY-LICENSES-RUST.md) | Every Rust crate linked into the LoreGUI binary + the bundled `loreserver` sidecar — including Epic's upstream `lore` crates and the vendored `quinn-proto` fork (~520 crates). | [`cargo-about`](https://github.com/EmbarkStudios/cargo-about) (`about.toml` + `about.hbs`) |
| [THIRD-PARTY-LICENSES-FRONTEND.md](THIRD-PARTY-LICENSES-FRONTEND.md) | The production npm packages embedded in the Vite-built frontend. | [`license-checker-rseidelsohn`](https://github.com/RSeidelsohn/license-checker-rseidelsohn) via `frontend/scripts/gen-third-party-licenses.mjs` |

## Regenerating

```bash
# Rust (needs cargo-about: cargo install cargo-about)
cargo about generate --all-features about.hbs -o THIRD-PARTY-LICENSES-RUST.md

# Frontend (needs frontend deps installed: npm --prefix frontend ci)
node frontend/scripts/gen-third-party-licenses.mjs
```

## License posture

All bundled dependencies are under permissive licenses (MIT, Apache-2.0, BSD,
ISC, Zlib, BSL-1.0, Unicode, ...) or the file-level weak-copyleft MPL-2.0.
**There are no GPL / LGPL / AGPL / SSPL dependencies**, so there is no copyleft
obligation on the LoreGUI source. Both generators fail if a dependency outside
the accepted license set is introduced, keeping this guarantee enforced.
