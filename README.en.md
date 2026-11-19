![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

> The canonical English README is [README.md](README.md). This path remains
> available for links that used the previous English filename.

**Install and run local AI models from one CLI.**

**English** · [Español](README.es.md) · [Português](README.pt.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [한국어](README.ko.md) · [Русский](README.ru.md)

[![CI](https://github.com/Lorenzo-SF/ropero/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Lorenzo-SF/ropero/actions/workflows/ci.yml)

Ropero connects a declarative model catalog to the engines that run those
models. It prepares an engine, starts and monitors its service, and exposes an
OpenAI-compatible API when the engine supports one.

## What it offers

- Separate engine installation (`ropero install engines`) and model
  installation (`ropero install models`).
- TOML manifests for requirements, downloads, formats, and runtime options.
- Model lifecycle management, status, diagnostics, and evaluation tools.
- Integrations with compatible agents and coding assistants, with documented
  limitations.

## Get started

Ropero v2 is under development. Build it from the repository root:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

The engine command lists available options; you can also select one directly,
for example `ropero install engines basalt`. After a successful Basalt
installation, Ropero registers the bundled Qwen3.8-Flash-Next model definition
in the active catalog if it is missing. Only the model pack remains to be
downloaded with `ropero install models qwen3.8-flash-next`. See the [getting
started guide](docs/quickstart.en.md) for the full workflow and requirements.

## Choosing a model definition

Browse the [model catalog](models/) by engine. Each TOML file describes the
model files, download source and launch options. For example, the
[groot-o definition](models/llama-server/groot-o.toml) lists the GGUF file
and its download size. From the repository root, add it to your active
catalog (by default `~/.config/ropero/models/`) and download the weights:

```bash
ropero models add --path models/llama-server/groot-o.toml
ropero install models groot-o
```

The `fired/` folders hold retired definitions. To write a definition for a
different model, start with the [model template](models/_template.toml).

## Documentation

| Guide | Contents |
|---|---|
| [Getting started](docs/quickstart.en.md) | Build, configure TOML, install, and run |
| [Engine compatibility](docs/engine-support.en.md) | Requirements, installation, and engine limits |
| [Integrations](docs/agent-integrations.en.md) | Agents, IDEs, and automatic or manual setup |
| [CLI reference](docs/especificacion/ropero-cli.en.md) | Commands and options |
| [License audit (Spanish)](docs/licensing-audit.es.md) | Upstream sources and unresolved items |
| [Contributing](CONTRIBUTING.md) | Change guidelines and local checks |
| [Security](.github/SECURITY.md) | Private vulnerability reporting |

Start with the [English user guide](docs/guide.en.md). User guides are available in ten languages; detailed compatibility and integration references are currently in English and Spanish.
