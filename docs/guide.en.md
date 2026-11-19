# Ropero user guide

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · [Français](guide.fr.md) · [Deutsch](guide.de.md) · [Italiano](guide.it.md) · [日本語](guide.ja.md) · [简体中文](guide.zh-CN.md) · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero is a Rust CLI for installing, configuring, and running local AI models with different engines. It prepares the selected engine, starts and monitors its service, and exposes an OpenAI-compatible API when that engine supports one. Ropero v2 is under development.

## Get started

From the repository root, build the CLI and inspect the available commands:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

The engine command lists available engines; you can also select one directly, for example `ropero install engines basalt`. Engine installation prepares the runtime and its dependencies. Install the model separately with `ropero install models <model>`.

The CLI starts in English. Save Spanish with `ropero config --lang ES`; use `ropero config --lang EN` to switch back.

## Catalog and TOML

Manifests in `engines/` describe engine requirements, detection, installation, and capabilities. Files in `models/<engine>/` describe a model, its format, download, artifacts, and runtime options. In normal use, active model definitions are stored in `~/.config/ropero/models/<engine>/`, and personal engine manifests in `~/.config/ropero/engines/`. Repository TOML files are examples and are not loaded automatically.

```bash
ropero models add
ropero models add --path model.toml
ropero install models <model>
ropero <model>
ropero status --live
```

## Engines and limits

- **llama-server** and **turbo-llama** serve GGUF models.
- **FastFlowLM** delegates work to its `flm` CLI; supported formats and NPU use depend on FastFlowLM and XRT.
- **Ignis** serves downloaded engine artifacts.
- **Basalt** serves `.basalt` packs. Ropero supports it only on Linux x86-64. After a successful engine install, Ropero registers the bundled Qwen3.8-Flash-Next TOML if it is absent; the model pack is still installed separately. Basalt requires an AVX2 CPU, an NVIDIA Blackwell GPU, and a compatible driver.
- **Strata** serves prepared profiles; Ropero does not create or download profiles.
- **vLLM**, **MLX-Serve**, and **TensorRT-LLM** have adapters, but support depends on platform and model. MLX-Serve requires macOS and Apple Silicon.
- **AirLLM** support is partial; Ropero does not provide a server adapter.

See the [detailed engine compatibility reference](engine-support.en.md) for requirements and known limitations.

## Agents, IDEs, and tools

`ropero connect --list` shows automatic connectors, including OpenCode, Codex, Claude Code, Pi, Hermes, OpenClaw, PicoClaw, and VS Code. Start a model with Ropero before connecting. A connector configures the client; it does not install the agent or start the engine.

Zed can use OpenAI-compatible endpoints and stores keys in the system keychain; configure it in Agent Settings. Cursor's official documentation does not confirm direct connections to local Ropero endpoints. MicroClaw can use `llm_base_url` in its configuration. Graphify is an optional repository-analysis tool used with a compatible assistant, not a Ropero engine or model.

See the [integration reference](agent-integrations.en.md) and [CLI reference](especificacion/ropero-cli.en.md).

## Activate and deactivate models

By default, `ropero models` lists installed engines and models that are ready to use. An installed engine with no ready models is shown with a note. Use `--all` to inspect the full catalog, or `--fired` to see deactivated models. Firing or hiring moves the TOML into or out of that engine’s `fired/` directory; it does not delete the definition or model files. If an active model already uses the name, Ropero asks for a new unique active name before hiring.

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <model> --fired
ropero model <model> --hired
```
