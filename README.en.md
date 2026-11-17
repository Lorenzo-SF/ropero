# ropero

**English** · [Español](README.md)

**ropero** is a command-line tool for serving local language models with the
engine that best fits each machine. It reads TOML model definitions, starts the
selected engine, checks that it is ready, and exposes an OpenAI-compatible API.

> **Current specification:** the CLI and configuration summary received on
> 2026-11-16 supersedes older contracts where they differ. It is consolidated
> in [`R-CLI-18…27`, `R-CFG-1…2`, and `R-MOD-32…34`](docs-especificacion/ropero-especificacion.md#414-especificación-posterior-del-cli-y-configuración-2026-11-16).
> Its status table is an initial assessment; partial or pending items are not
> completed features.

## Two generations in transition

The repository contains the working v1 and the Rust v2 workspace under active
development:

| Generation | Location | Status |
|---|---|---|
| **v1** | [`ropero-code/`](ropero-code/) | Working Bash and Python implementation. It remains the released version. It is a reference for v2, not a dependency; see [ADR-003](docs/adr/ADR-003-ropero-code-es-referencia.md). |
| **v2** | [`crates/`](crates/) | Rust workspace under development; it is not yet presented as a published replacement for v1. |

The two generations coexist until F9, as described in
[`ADR-002`](docs/adr/ADR-002-extraccion-vs-reescritura.md). The staged plan
preserves v1's contracts and on-disk state while v2 is built.

The TOML files in [`models/`](models/) are reference copies and backups. The
application does **not** discover them as its normal catalog. By default,
`install models`, `list`, and `ropero <alias>` read the user's catalog from
`$XDG_CONFIG_HOME/ropero/models/` or `~/.config/ropero/models/`. Place model
TOML files there. `ROPERO_MODELS_DIR` and `ROPERO_ROOT` are development and test
overrides.

### v2 scope

The Rust workspace includes a CLI, TOML catalogs and engine manifests, external
configuration, engine start and stop workflows, installation, GGUF generation,
agent connections, and an internal evaluation suite. Support varies by command,
engine, and agent. Check `ropero --help`, the changelog, and the specification
before assuming a workflow is complete.

## Install and use v1

```bash
git clone https://github.com/Lorenzo-SF/ropero.git
cd ropero/ropero-code
bash install.sh --local
```

This creates `~/.local/bin/ropero → <repo>/ropero-code/bin/ropero` (the only
entry in `PATH`) and `~/.local/share/ropero → <repo>/ropero-code/bin`.

```bash
ropero install setup      # base dependencies (python3, git, uv, hf, jq, cmake…)
ropero install engines    # choose engines and install their dependencies
ropero install models     # choose models (download or build a GGUF)
```

Running `ropero install` without arguments performs all three steps in order.
See the [v1 usage guide](ropero-code/README.md) for the rest of the workflow.

## Build and check v2

From the repository root:

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --release
```

In normal use, the CLI resolves the catalog from user configuration, regardless
of the current working directory:

```bash
cargo run --locked -p ropero-cli -- config
cargo run --locked -p ropero-cli -- list --all
```

### Catalog, configuration, and overrides

The normal catalog lives in
`$XDG_CONFIG_HOME/ropero/models/<engine>/` (or
`~/.config/ropero/models/<engine>/`), with engine manifests in
`<config_directory>/engines/`. On first use, ropero creates the configuration
directory and a default `config.toml` idempotently, without overwriting
existing files. The default `logs_dir` is `<config_directory>/logs`.

Path precedence is `environment variable > config.toml > default`:
`ROPERO_MODELS_DIR`/`models_dir`, `GGUF_DIR`/`gguf_dir`,
`SAFETENSORS_DIR`/`safetensors_dir`, `ROPERO_BIN_DIR`/`bin_dir`, and
`LOG_DIR`/`logs_dir`.

`ROPERO_MODELS_DIR` and `ROPERO_ROOT` remain development and test overrides,
including resolution of `<root>/engines/` when pointed at `<root>/models/`.
Repository TOML files are not auto-discovered during normal use.

**Migrate legacy model TOML and engine manifests:**

```bash
mkdir -p ~/.config/ropero/models ~/.config/ropero/engines
# Copy or move models, grouped by engine
cp /old/path/models/<engine>/*.toml ~/.config/ropero/models/<engine>/
# Copy user engine manifests
cp /old/path/engines/*.toml ~/.config/ropero/engines/
```

`ropero config` displays paths and their source; `ropero doctor` checks
configuration and dependencies; `ropero template` suggests a model template
without writing it. These commands do not modify model artifacts. The
idempotent configuration bootstrap may run if configuration is missing.

### Connect coding agents

`ropero connect --list` shows supported integrations. For example:

```bash
ropero connect --model qwen --agent opencode
```

Separate multiple models with commas or semicolons. Add `--uninstall` to remove
the selected model connections. OpenCode, Pi, OpenClaw (`claw` is an alias),
Codex, Hermes, and Kilo group providers by engine. VS Code and Copilot use
Custom Endpoint; CodeBuddy and Droid store endpoints on each custom entry.
Aider registers an endpoint and key per model in its official model
settings/metadata files. MCode uses its official CLI; because it removes entire
providers, uninstall requires selecting all Ropero models for that engine.
Claude Code requires the Anthropic Messages API, which Ropero's
OpenAI-compatible engines do not expose directly.

## Documentation

Read the two normative documents together:

| Document | Answers |
|---|---|
| [Specification and requirements](docs-especificacion/ropero-especificacion.md) | What should ropero do, and with what exact semantics? |
| [Build specification](docs-especificacion/ropero-construccion.md) | How is it built, in what phases, and with which quality gates? |

If they conflict, the requirements specification governs system behavior; the
build specification governs implementation. Architecture decisions are in
[`docs/adr/`](docs/adr/). The [v2 changelog](CHANGELOG.md) records this
workspace's changes; the [v1 changelog](ropero-code/CHANGELOG.md) remains in
effect for the working release.
