# Ropero

**Install and run AI models locally.**

**English** · [Español](README.md)

Ropero organizes local models and the engines that run them. Choose a model;
Ropero prepares its declared engine, starts it, checks that it is ready, and
provides an OpenAI-compatible API when the engine supports one.

## What it offers

- **Separate engine and model installation.** `ropero install engines`
  prepares the software needed to run models; `ropero install models`
  obtains their artifacts by downloading or building them, depending on the model.
- **TOML-based configuration.** Each model declares its engine, format,
  location or download, options, and usage profiles.
- **Managed serving.** Ropero starts the right engine, assigns a port, waits
  for it to respond, and helps inspect or stop services.
- **Catalog and diagnostic tools.** Register models, inspect configuration and
  dependencies, print serving commands, and run an evaluation suite.
- **Coding assistant connections.** Integrations register models or endpoints
  with supported tools; limitations are documented for each agent.

Support depends on the engine, operating system, and command. Check the [engine
compatibility guide](docs/engine-support.en.md) before choosing an engine.

## Get started

This checkout contains Ropero v2, which is still under development. Build it
from the repository root:

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

The engine command lists available engines. You can also choose one directly,
for example:

```bash
ropero install engines basalt
```

Basalt requires Linux x86-64, an AVX2 CPU, an NVIDIA Blackwell GPU, and a
compatible driver. Ropero installs its runtime and dependencies; the model is
downloaded separately. See [install and use Basalt](docs/engine-support.en.md#install-and-use-basalt)
for the requirements and full workflow.

## Models and TOML

Ropero uses two kinds of TOML files:

- Manifests in `engines/` describe each engine, its requirements, and how to
  detect or install it.
- Files in `models/<engine>/` describe models, downloads, artifacts, and
  serving options.

During normal use, the active catalog is in
`~/.config/ropero/models/<engine>/`; user engine manifests go in
`~/.config/ropero/engines/`. The TOML files in this repository are examples and
references: Ropero does not load them automatically. Register a model with the
`ropero models add` wizard or pass an existing TOML file:

```bash
ropero models add
ropero models add --path model.toml
```

Then install a model by name or alias:

```bash
ropero install models <model>
```

The repository includes a Qwen3.8-Flash-Next definition for Basalt. The engine
guide explains how to copy it to your catalog, install it, and launch it.
Hugging Face downloads may require the `hf` CLI and an authenticated session.

Ropero initializes user configuration without overwriting existing files.
`ropero config` displays active paths; environment variables can override
paths for the catalog, artifacts, binaries, and logs.

## Use Ropero

Once the engine and model are installed, start a model by name:

```bash
ropero <model>
ropero models
ropero status --live
ropero info <model>
ropero command <model>
ropero <model> remove
```

`ropero test` evaluates a model or test group. To connect coding assistants,
run `ropero connect --list` and see the [CLI reference](docs-especificacion/ropero-cli.en.md).

## Status and documentation

This repository contains the Ropero v2 Rust workspace. It is under development
and is not yet presented as a published replacement for the v1 implementation.
Completion varies by command and integration.

| Document | What it covers |
|---|---|
| [Engine compatibility guide](docs/engine-support.en.md) | Requirements, installation, and per-engine limitations |
| [CLI reference](docs-especificacion/ropero-cli.en.md) | Commands, options, and detailed behavior |
| [Specification](docs-especificacion/ropero-especificacion.en.md) | Product requirements and contracts |
| [Build specification](docs-especificacion/ropero-construccion.en.md) | Architecture, phases, and quality gates |
| [Changelog](CHANGELOG.md) | Changes in this workspace |

The [Spanish engine guide](docs/engine-support.es.md) and [Spanish
specification](docs-especificacion/ropero-especificacion.md) are also available.

## Build and validate

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```
