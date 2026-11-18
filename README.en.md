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

## Project status

The repository contains Ropero's Rust workspace. The earlier v1 implementation
is not part of this checkout:

| Generation | Location | Status |
|---|---|---|
| **v1** | Not included in this checkout | The earlier implementation is not part of this working tree. Its historical contracts remain as documentation. |
| **v2** | [`crates/`](crates/) | Rust workspace under development; it is not yet presented as a published replacement for v1. |

The Rust workspace preserves the earlier version's documented contracts and
on-disk state.

The TOML files in [`models/`](models/) are reference copies and backups. The
application does **not** discover them as its normal catalog. By default,
`install models`, `models`, and `ropero <alias>` read the user's catalog from
`$XDG_CONFIG_HOME/ropero/models/` or `~/.config/ropero/models/`. Place model
TOML files there. `ROPERO_MODELS_DIR` and `ROPERO_ROOT` are development and test
overrides.

### v2 scope

The Rust workspace includes a CLI, TOML catalogs and engine manifests, external
configuration, engine start and stop workflows, installation, GGUF generation,
agent connections, and an internal evaluation suite. Support varies by command,
engine, and agent. Check `ropero --help`, the changelog, and the specification
before assuming a workflow is complete.

Basalt is integrated for Linux x86-64 with an NVIDIA Blackwell GPU.
`ropero install engines basalt` installs the runtime and prepares its Python
dependencies; installing the `.basalt` pack is a separate step. Ropero starts
Basalt's OpenAI server and assigns its port; the manifest and
Qwen3.8-Flash-Next TOML are in `engines/basalt.toml` and `models/basalt/`.
The [engine compatibility guide](docs/engine-support.en.md#install-and-use-basalt)
covers CUDA/Python requirements, setup, and authentication limits.

## Build and check

From the repository root:

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release -p ropero-cli
```

The executable is `target/release/ropero`; run `--help` to see the available
commands.

In normal use, the CLI resolves the catalog from user configuration, regardless
of the current working directory:

```bash
cargo run --locked -p ropero-cli -- config
cargo run --locked -p ropero-cli -- models --all
```

### Model setup and evaluation

`ropero models add` opens an interactive model wizard. It also accepts a TOML
file with `--path`, TOML from standard input with `--stdin`, or direct values
and repeatable `--set` assignments. `ropero template` prints the compact schema
guide; model creation is validated by Ropero itself.

`ropero test` opens model, group, and test selectors when run without options.
For automation, pass `--model` and exactly one of `--test` or `--test-group`;
`--path` selects the results directory.

Models and tests run sequentially. Ropero stops the tested model's engine when
that model's run finishes, including when `--all-models` is selected.

For example:

```bash
ropero test --model qwen --test-group A --path ./results
ropero test --model qwen --test A1 --path ./results
ropero command qwen
ropero info qwen
ropero status --live
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

Separate multiple models with commas or semicolons. Add `--remove` to remove
the selected model connections. OpenCode, Pi, OpenClaw (`claw` is an alias),
Codex, Hermes, and Kilo group providers by engine. VS Code and Copilot use
Custom Endpoint; CodeBuddy and Droid store endpoints on each custom entry.
Aider registers an endpoint and key per model in its official model
settings/metadata files. MCode uses its official CLI; because it removes entire
providers, uninstall requires selecting all Ropero models for that engine.
Claude Code requires the Anthropic Messages API. Ropero prints the environment
variables needed to configure a compatible URL and token.

For Codex, Ropero creates one profile per engine without changing the global
provider. Connect the models and use the profile printed by the command, for
example `codex --profile ropero-turbo-llama -m ornith`. If `CODEX_HOME` is set,
Ropero writes the configuration and profiles there. Codex requires the
Responses API; Ropero rejects engines that do not declare support for it.

TRAE and TRAE CN require adding or removing the model in their model settings;
`ropero connect --agent trae|trae-cn --model <model>` prints the exact values
for that step without editing private application files or displaying keys.
`claw` is a historical OpenClaw alias, not a separate agent. Gemini CLI,
Cursor, Kiro, Antigravity, and Devin are omitted from `--list`: no documented
integration compatible with Ropero engine endpoints and/or safe persistent
model-registration interface was found.

`ropero config --lang EN` or `--lang ES` stores the interface language; EN is
the default. `ropero models add` creates a TOML through a wizard, flags, file,
or stdin. Repeat `--set block.field=TOML_VALUE` to define schema fields, e.g.
`--set levels.list='["low","high"]'`. `ropero info <model>` displays its configuration, and
`ropero command <model>` prints serving arguments in copyable form.
`ropero status --live [SECONDS]` refreshes the terminal view.

## Documentation

Read the two normative documents together:

| Document | Answers |
|---|---|
| [Specification and requirements](docs-especificacion/ropero-especificacion.en.md) | What should ropero do, and with what exact semantics? |
| [Build specification](docs-especificacion/ropero-construccion.en.md) | How is it built, in what phases, and with which quality gates? |

The current CLI references in [English](docs-especificacion/ropero-cli.en.md)
and [Spanish](docs-especificacion/ropero-cli.md), together with the engine
compatibility guides in [English](docs/engine-support.en.md) and
[Spanish](docs/engine-support.es.md), document commands, evidence, and support
limits.

If they conflict, the requirements specification governs system behavior; the
build specification governs implementation. Architecture decisions are in
[`docs/adr/`](docs/adr/). The [changelog](CHANGELOG.md) records this
workspace's changes; the historical v1 changelog is not included in this
checkout.
