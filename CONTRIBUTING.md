# Contributing to Bedrock

Documentation corrections, accessible UI fixes, reproducible bugs, and focused patches are welcome.

## Before a patch

Search [issues](https://github.com/Manicharan01/bedrock-notes/issues). Open a proposal for large features, schema changes, new dependencies, or changes to the data/security model. Small bug and documentation fixes can go straight to a pull request.

Use synthetic notes and redact private content. Report vulnerabilities through [Security](SECURITY.md), not a public issue.

## Work locally

Follow [Development](docs/development.md). Create a branch from `main`, keep the patch focused, and explain the problem and resulting behavior.

- Preserve drafts, notebook identity, revision checks, and compatible imports.
- Keep parsing, validation, authentication, and writes deterministic.
- Avoid dependencies for tasks the existing stack handles clearly.
- Add focused regressions for behavior changes. Check UI changes at desktop, 390 px and 320 px, with keyboard navigation and reduced motion.
- Update docs when setup, behavior, limits, or configuration changes.

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
node --test tests/client.test.mjs tests/connection.test.mjs
python scripts/check-docs.py
```

Native edits also need `cargo fmt --manifest-path native/src-tauri/Cargo.toml --check` and a target-platform build when possible. Tell reviewers which platforms were actually exercised.

## Community

Be respectful and keep feedback specific to the work. Harassment and publishing another person's private information are unacceptable. The maintainer may remove abusive content or restrict participation. Contributions use the repository's [MIT license](LICENSE).
