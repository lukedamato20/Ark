# Rust dependency advisory review

Owner: **Platform Engineering**  
Last reviewed: **2026-08-14**  
Next mandatory review: **2026-09-14**, or immediately on a Tauri/tauri-utils upgrade

`cargo audit` has no vulnerability exceptions and reports no known vulnerabilities. The lockfile
still produces 17 allowed maintenance/soundness warnings. These are not silently ignored: the
table below records their target reachability and the only compatible upstream removal path.

| Advisories | Locked path and target reachability | Review decision |
|---|---|---|
| RUSTSEC-2024-0411 through -0420 (10 unmaintained GTK3 crates: `atk*`, `gdk*`, `gtk*`, `gtk3-macros`) | Linux only: Ark → Tauri/wry/tao/muda → WebKitGTK/GTK3. `cargo tree --target x86_64-pc-windows-msvc` and `--target x86_64-apple-darwin` contain none of these crates; the Linux production UI necessarily reaches this backend. | No compatible update exists in the current Tauri 2 graph (`cargo update` changes zero packages). These are maintenance warnings, not disclosed vulnerabilities. Retain only while Tauri 2 requires GTK3; Platform Engineering must reevaluate monthly and migrate when Tauri exposes a maintained Linux backend. |
| RUSTSEC-2024-0429 (`glib` iterator unsoundness) | Linux only through the same Tauri/WebKitGTK graph. Ark has no direct `glib` dependency or `VariantStrIter` use, but the framework is production-reachable, so this is not labelled unreachable. | The advisory is limited to `VariantStrIter`; Ark does not call it directly. No compatible `glib` update exists under the locked GTK3 graph. Recheck upstream monthly and treat any broadened advisory or demonstrated framework call path as release-blocking. |
| RUSTSEC-2024-0370 (`proc-macro-error` unmaintained) | Linux-only build/proc-macro path through `glib-macros` and `gtk3-macros`; absent from Windows/macOS target trees and not linked as Ark runtime application code. | No compatible update exists. Accept the build-time maintenance risk pending the same Tauri Linux-backend upgrade; never convert it to a vulnerability exception without a separate approved record. |
| RUSTSEC-2025-0075, -0080, -0081, -0098, -0100 (`unic-*` unmaintained) | All supported targets: Ark → `tauri-utils 2.9.3` → `urlpattern 0.3.0` → `unic-ucd-ident`. This is both Tauri build/runtime infrastructure; Ark has no direct call site. | No compatible `urlpattern`/Tauri update is currently available (`cargo update -p urlpattern --dry-run` changes zero packages). Maintenance-only risk is retained with monthly review; a Tauri release removing this chain should be adopted promptly. |

The SEC-003 vulnerability upgrade is intentionally narrow: `plist 1.9.0 → 1.10.0` and
`quick-xml 0.39.4 → 0.41.0`. `crossbeam-epoch` remains `0.9.20`. No other lockfile package moved.
