![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**在本地安装并运行 AI 模型。**

[English](README.md) · [Español](README.es.md) · [Português](README.pt.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [日本語](README.ja.md) · **简体中文** · [한국어](README.ko.md) · [Русский](README.ru.md)

Ropero 管理本地模型及其运行引擎。选择模型后，Ropero 会准备对应引擎、启动服务并检查状态；引擎支持时，还会提供兼容 OpenAI 的 API。

## 功能

- 分别安装引擎（`ropero install engines`）和模型（`ropero install models`）。
- 使用 TOML 清单声明依赖、下载和运行参数。
- 启动、查看状态并停止模型服务。
- 提供诊断、配置查看、启动命令和模型评测工具。
- 连接兼容的编程助手，并说明各集成的限制。

## 快速开始

本仓库包含仍在开发中的 Ropero v2。在仓库根目录编译：

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

运行引擎安装命令可查看可用选项，也可以直接选择，例如 `ropero install engines basalt`。模型需单独安装：`ropero install models <模型名>`。

## 模型与使用

`engines/` 中的清单描述引擎及安装方式；[`models/<引擎>/`](models/) 存放模型 TOML 定义。常规使用时，活动目录位于 `~/.config/ropero/models/`。仓库中的 TOML 文件是示例，不会自动加载。

```bash
ropero models add
ropero models add --path models/llama-server/groot-o.toml
ropero <模型名>
ropero status --live
```

详细说明请查看[西班牙语](docs/engine-support.es.md)和[英语](docs/engine-support.en.md)指南、[CLI 参考](docs/especificacion/ropero-cli.en.md)及[集成指南](docs/agent-integrations.en.md)。另请参阅[简体中文用户指南](docs/guide.zh-CN.md)。用户指南提供十种语言；详细的兼容性和集成参考目前提供英语和西班牙语版本。
