# Development

[Contributing](../CONTRIBUTING.md) · [Architecture](architecture.md) · [API](../API.md)

## Server and shared interface

Install Rust 1.85+ with rustfmt and Clippy, a C compiler for bundled SQLite, and Node.js 24+ for tests. The local build was verified with Rust 1.98.1. Lockfiles are committed.

```sh
git clone https://github.com/Manicharan01/bedrock-notes.git
cd bedrock-notes
cargo run --locked -- --port 4173 --db data/development.sqlite3
```

Open `http://127.0.0.1:4173`. Web assets are embedded, so restart `cargo run` after editing `web/`. The interface uses plain HTML, CSS and ES modules, with no bundler or npm install step.

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
node --test tests/client.test.mjs tests/connection.test.mjs
python scripts/check-docs.py
```

Use scratch databases and synthetic notes. Add regressions for draft preservation, revision races, imports, sessions, or parsing. Check visual changes at desktop, 390 px, and 320 px widths, including keyboard focus and phone navigation.

## Native clients

Install Rust 1.90+, Node.js 24+, and [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). The wrapper connects to a running server and uses the same `web/` directory.

```sh
cd native
npm ci
# Portable Windows executable, on Windows:
npm run tauri -- build --no-bundle --ci
# Linux, with WebKitGTK 4.1 development dependencies:
npm run tauri -- build --ci --bundles deb,appimage
```

Windows output: `native/src-tauri/target/release/app.exe`. Rename it `Bedrock.exe` when packaging. It needs an existing WebView2 Runtime. Builds are unsigned.

### Android

Use Java 21, SDK API 36, Build Tools 36.0.0, NDK 29.0.14206865, and Gradle 8.13. Install Rust's `aarch64-linux-android` target and set `JAVA_HOME`, `ANDROID_HOME`, and `NDK_HOME`.

```sh
cd native
npm ci
rustup target add aarch64-linux-android
npm run tauri -- android build --debug --target aarch64 --apk --ci
```

The generated project pins AGP 8.13.2/Gradle 8.13. Re-running `android init` can overwrite compatibility adjustments. On Windows without symlink support:

```powershell
.\Build-Android.ps1 -Gradle .\src-tauri\gen\android\gradlew.bat
```

The helper compiles Rust, copies the ARM64 library, and assembles a debug APK. Debug ID: `org.lattice.notes.debug`. Each machine's debug certificate can differ. Production signing needs a protected release key; see [Android signing](https://v2.tauri.app/distribute/sign/android/).

## CI and publishing

- `ci.yaml` runs on pushes and pull requests: Rust formatting/tests/Clippy on Windows and Linux, JavaScript tests, and documentation checks.
- `build.yaml` is a manual package build. Linux desktop packages are opt-in on manual runs; Android produces a test APK. Artifacts do not deploy a server or automatically publish a release.
- The **Linux desktop packages** input defaults to false. An ordinary push does not build DEB/AppImage packages.

For releases, update both application Cargo packages/lockfile entries, native npm files, Tauri config, Compose image tag, and changelog. Keep client versions consistent. Preserve dependency versions, app identifiers, schema, cookies, and notebook UUIDs unless deliberately migrating. The API and `--help` read Cargo's version.

Build on target platforms, run checks, verify APK signatures/alignment, and package binaries with notices and setup instructions. Publish SHA-256 checksums and signing/platform limitations. Never publish access keys, keystores, scratch databases, or SDK caches. [Verification](../VERIFICATION.md) distinguishes builds from device testing.
