# Ropero 用户指南

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · [Français](guide.fr.md) · [Deutsch](guide.de.md) · [Italiano](guide.it.md) · [日本語](guide.ja.md) · **简体中文** · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero 是一个用 Rust 编写的 CLI，可通过不同引擎安装、配置和运行本地 AI 模型。它会准备所选引擎、启动并监控服务；引擎支持时，可提供兼容 OpenAI 的 API。Ropero v2 仍在开发中。

## 开始使用

在仓库根目录构建 CLI 并查看可用命令：

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

engine 命令会列出可用引擎，也可直接指定，例如 `ropero install engines basalt`。安装引擎会准备运行时及其依赖。模型需单独通过 `ropero install models <model>` 安装。

CLI 默认使用英语。运行 `ropero config --lang ES` 可保存西班牙语；运行 `ropero config --lang EN` 可切回英语。

## 模型目录与 TOML

`engines/` 中的清单描述引擎要求、检测、安装和能力。`models/<engine>/` 中的文件定义模型、格式、下载、产物和运行参数。正常使用时，生效的模型定义位于 `~/.config/ropero/models/<engine>/`，用户引擎清单位于 `~/.config/ropero/engines/`。仓库中的 TOML 是示例，不会自动加载。

```bash
ropero models add
ropero models add --path model.toml
ropero install models <model>
ropero <model>
ropero status --live
```

## 引擎与限制

- **llama-server** 和 **turbo-llama** 可运行 GGUF 模型。
- **FastFlowLM** 将操作交给其 `flm` CLI；格式和 NPU 支持取决于 FastFlowLM 与 XRT。
- **Ignis** 运行下载的引擎产物。
- **Basalt** 运行 `.basalt` 模型包。Ropero 仅在 Linux x86-64 上支持 Basalt。引擎成功安装后，如果用户目录中还没有对应文件，Ropero 会自动注册随附的 Qwen3.8-Flash-Next TOML；模型包仍需单独安装。Basalt 需要 AVX2 CPU、NVIDIA Blackwell GPU 和兼容驱动。
- **Strata** 运行已准备好的配置；Ropero 不创建或下载配置。
- **vLLM**、**MLX-Serve** 和 **TensorRT-LLM** 提供适配器，但支持情况取决于平台和模型。MLX-Serve 需要 macOS 和 Apple Silicon。
- **AirLLM** 目前仅部分支持，Ropero 没有服务器适配器。

要求和已知限制请参阅[引擎兼容性详细文档（英文）](engine-support.en.md)。

## Agent、IDE 与工具

运行 `ropero connect --list` 可查看 OpenCode、Codex、Claude Code、Pi、Hermes、OpenClaw、PicoClaw 和 VS Code 等自动连接器。连接前请先用 Ropero 启动模型。连接器只配置客户端，不会安装 Agent 或启动引擎。

Zed 支持兼容 OpenAI 的端点，并将密钥保存在系统密钥环中，可在 Agent Settings 配置。Cursor 官方文档尚未确认可直接连接 Ropero 本地端点。MicroClaw 可在配置中使用 `llm_base_url`。Graphify 是可选的仓库分析工具，可配合兼容助手使用；它不是 Ropero 引擎或模型。

另请参阅[集成参考（英文）](agent-integrations.en.md)和[CLI 参考（英文）](especificacion/ropero-cli.en.md)。

## 启用和停用模型

`ropero models` 默认显示已安装的引擎和可立即使用的模型。已安装但没有可用模型的引擎也会附带提示显示。使用 `--all` 查看完整目录，使用 `--fired` 查看已停用的模型。停用或重新启用会将 TOML 移入或移出引擎的 `fired/` 目录，不会删除模型定义或模型文件。如果名称已被启用的模型占用，Ropero 会在重新启用前要求输入一个唯一的新名称。

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
