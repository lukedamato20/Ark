# Building and installing a local desktop package

OPS-002's scope, as recorded in [implementation-plan.md](../implementation-plan.md): Ark is
personal-use software distributed directly to a small number of named people the developer
knows, not published to strangers. That means the installers this produces are **unsigned** —
there is no code-signing certificate, no notarization, and no public update feed. This document
covers how to build one locally and what each OS shows the person installing it.

## Building

```powershell
pnpm install
pnpm tauri:build
```

This runs the frontend production build, compiles the Rust backend in release mode, and produces
platform-specific installers under `src-tauri/target/release/bundle/`.

### Windows prerequisite: a real Perl toolchain

`rusqlite` is built with the `bundled-sqlcipher-vendored-openssl` feature (see
`src-tauri/Cargo.toml`), which compiles OpenSSL from source as part of the build. On Windows,
that source build shells out to `perl`. The Perl that ships with Git for Windows is a minimal
distribution — it's missing modules the OpenSSL build script needs (`Locale::Maketext::Simple`
among them), and its bundled `cpan` client can't install them either, since a dependency of CPAN
itself is also missing. A debug build can still succeed if a working `openssl-sys` build is
already cached from an earlier successful compile, which is why this can go unnoticed until the
first release/bundle build on a machine.

The fix is a real, complete Perl distribution.
[Strawberry Perl](https://strawberryperl.com/) is the standard choice for this exact class of
problem in the Rust-on-Windows ecosystem:

```powershell
winget install --id StrawberryPerl.StrawberryPerl
```

Make sure Strawberry Perl's `perl.exe` resolves ahead of any other `perl` already on `PATH`
(Git for Windows' bundled one, in particular) before running `pnpm tauri:build`.

### What gets produced

A successful Windows build produces two installers:

- `src-tauri/target/release/bundle/msi/Ark_<version>_x64_en-US.msi`
- `src-tauri/target/release/bundle/nsis/Ark_<version>_x64-setup.exe`

Both install per-user (no administrator prompt) to `%LOCALAPPDATA%\Ark`, register a Start Menu
shortcut, and include the bundled llama.cpp runtime binaries alongside `ark.exe`.

## The one-time trust step

Because these builds are unsigned, each OS will flag the installer as coming from an unverified
publisher the first time it runs. This is expected — it does not mean the build is broken or
unsafe, only that the machine has no reason yet to recognize it. Whoever the developer is handing
a build to should be told this in advance, and to expect this exact prompt:

- **Windows (SmartScreen):** the installer opens a blue "Windows protected your PC" screen. Click
  **More info**, then **Run anyway**. This appears once per unrecognized executable, not once
  per install session.
- **macOS (Gatekeeper):** a double-click reports the app "cannot be opened because the developer
  cannot be verified." Right-click (or Control-click) the app instead and choose **Open** — this
  presents a second dialog with an **Open** button that a plain double-click never offers.

Neither step requires disabling SmartScreen or Gatekeeper system-wide. Both are a one-time
decision for this specific executable.

## Smoke test performed for this task

Verified on this Windows workstation (`docs/quality-baseline.md`'s reference machine):

1. `pnpm tauri:build` produced both installers listed above.
2. The NSIS installer (`Ark_0.1.0_x64-setup.exe /S`) installed silently to
   `%LOCALAPPDATA%\Ark`, including a Start Menu shortcut and the bundled llama.cpp runtime.
3. The installed `ark.exe` launched and stayed running across repeated process checks (no
   immediate crash).
4. The bundled `uninstall.exe /S` removed the install directory and Start Menu shortcut
   completely — verified empty afterward.

**Not verified in this pass:** the MSI installer's own install/uninstall flow (only the NSIS
build was smoke-tested), and macOS/Linux builds entirely — this development environment is
Windows-only, so `.dmg`/`.deb`/`.AppImage` output was neither built nor tested. If macOS or Linux
machines are ever added to the small distribution list this task's scope assumes, their installer
paths need the same real smoke test before being handed to anyone, not assumed to work from the
Windows result.
