# Getting started

This guide expands the [English README](../README.en.md). Ropero v2 is still
under development.

## Build and prepare

From the repository root:

```bash
cargo build --locked --release -p ropero-cli
```

The executable is `target/release/ropero`. In the examples below, `ropero`
means that executable available on your `PATH`.

```bash
ropero --help
ropero config
ropero doctor
ropero install engines
```

The engine command lists available engines. You can choose one directly, for
example `ropero install engines basalt`. Ropero installs the engine runtime and
dependencies; model files are installed separately with
`ropero install models <model>`. Basalt requires Linux x86-64, an AVX2 CPU, an
NVIDIA Blackwell GPU, and a compatible driver. See the [Basalt guide](engine-support.en.md#install-and-use-basalt).

## TOML and catalogs

Ropero uses two kinds of TOML files:

- `engines/`: engine requirements, detection, installation, and capabilities.
- `models/<engine>/`: model definition, format, download, artifacts, and
  serving parameters.

During normal use, the active catalog is in `~/.config/ropero/models/<engine>/`
and user engine manifests go in `~/.config/ropero/engines/`. Repository TOML
files are examples and references; Ropero does not load them automatically.
Register a model with the wizard or an existing file:

```bash
ropero models add
ropero models add --path model.toml
```

Then install it by name or alias:

```bash
ropero install models <model>
```

After a successful Basalt engine installation, Ropero registers the bundled
Qwen3.8-Flash-Next TOML in the active user catalog if it is not already there.
Install the pack separately with `ropero install models
qwen3.8-flash-next`. Hugging Face downloads may require the `hf` CLI and an
authenticated session.

Ropero initializes user configuration without overwriting existing files.
`ropero config` displays active paths; environment variables can override the
catalog, artifacts, binaries, and log paths.

## Run and evaluate

```bash
ropero <model>
ropero models
ropero status --live
ropero info <model>
ropero command <model>
ropero <model> remove
ropero test
```

To connect an assistant, run `ropero connect --list` and read the [integration
guide](agent-integrations.en.md). Connecting configures the client; it does
not install the agent or start the engine.

## Development checks

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```
