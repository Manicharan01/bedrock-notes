# Bedrock 0.4.1 verification

Checked on Windows on 2026-10-16 using scratch notebooks only.

| Check | Result |
|---|---|
| Rust regressions | 47 passed: session expiry, HTTP, Markdown, storage, self-hosting, and new/legacy client headers |
| Client regressions | 15 passed, including portable Markdown export filenames |
| Documentation | Local links and anchors checked with `python scripts/check-docs.py` |
| Rust quality | Clippy with `-D warnings`; root and native formatting checked |
| Browser UI | Bedrock branding; read/write views; mobile note creation/save; note index; Links drawer; square-node Atlas |
| Responsive layout | 320 × 740 and 390 × 844 viewports had no horizontal document overflow |
| Existing notebook | Previous version's notebook UUID and all 8 notes (IDs, titles, bodies, timestamps) preserved exactly |
| Previous server | Updated client logged in, read a notebook, and logged out against the original 0.3.0 server |
| Deployment files | Base and HTTPS Compose configurations validated with both BEDROCK and legacy LATTICE variables |
| Windows | Rust server and Tauri desktop release executables rebuilt and packaged |
| Android | Bedrock 0.4.0/code 4000, ARM64; APK signature, ZIP integrity, 16 KB ZIP/ELF alignment verified |
| Native update identity | Android application ID and signing certificate match the previous APK; desktop identifier unchanged |
| Linux packages | Skipped as previously requested; source and CI retained |

The Android APK is a debug-signed sideload build, with min API 24 and target/compile API 36. No Android device/emulator or native Windows UI session was automated; the shared interface was exercised in the browser. The desktop package is an unsigned portable executable requiring WebView2. An NSIS installer was not rebuilt: its compiler previously failed with Windows DLL relocation error 623.

Compose configuration was validated using official Docker Compose v5.6.0. No Docker Engine, production deployment, TLS certificate issuance, or Linux runtime build was run. No existing user notes or live services were modified.

The original app identifiers, schema, database filename, backup format, cookies and recovery keys are retained for compatibility. Existing environment-variable names and client headers remain supported. Keep your original Compose project name when upgrading so its volume is reused.

`verification/` contains current test results, APK signature output, compatibility results and build evidence. Previews are supplied beside the packages. SDKs, dependency caches, private scratch keys, databases and compiled intermediates are excluded from the source archive.

Reproduce with `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, and `node --test tests/client.test.mjs tests/connection.test.mjs`. Native build instructions are in README. Package hashes are in `Bedrock-SHA256SUMS.txt`.
