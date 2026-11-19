# Contributing

Thanks for helping improve Ropero. Before opening a pull request, check the
existing issues and keep changes focused on one behavior or documentation
area.

## Local checks

For Rust changes, run the same core checks as CI from the repository root:

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```

Update or add tests when behavior changes. Keep manifests and examples
consistent with the documented CLI. Do not commit API keys, credentials,
downloaded model weights, or private configuration files.

## Pull requests

Explain the user-visible change, its limits, and the checks you ran. Changes to
the default branch require a pull request, passing Linux and macOS CI, and
review under the repository's CODEOWNERS and branch rules.

## Third-party material

Do not copy upstream code, binaries, model weights, logos, or images into the
repository without checking the applicable license and attribution terms.
When adding an engine, agent, or model, document the upstream source and its
license in the licensing audit.
