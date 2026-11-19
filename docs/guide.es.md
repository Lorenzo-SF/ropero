# Guía de usuario de Ropero

[English](guide.en.md) · [Español](guide.es.md) · [Português](guide.pt.md) · [Français](guide.fr.md) · [Deutsch](guide.de.md) · [Italiano](guide.it.md) · [日本語](guide.ja.md) · [简体中文](guide.zh-CN.md) · [한국어](guide.ko.md) · [Русский](guide.ru.md)

Ropero es un CLI escrito en Rust para instalar, configurar y ejecutar modelos de IA local con distintos engines. Prepara el engine elegido, inicia y supervisa su servicio y ofrece una API compatible con OpenAI cuando el engine la soporta. Ropero v2 está en desarrollo.

## Empezar

Desde la raíz del repositorio, compila el CLI y consulta los comandos disponibles:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

El comando de engines muestra los motores disponibles; también puedes elegir uno directamente, por ejemplo `ropero install engines basalt`. La instalación del engine prepara el runtime y sus dependencias. Instala el modelo por separado con `ropero install models <modelo>`.

El CLI empieza en inglés. Guarda el español con `ropero config --lang ES`; usa `ropero config --lang EN` para volver al inglés.

## Catálogo y TOML

Los manifiestos de `engines/` describen requisitos, detección, instalación y capacidades. Los archivos de `models/<engine>/` definen el modelo, formato, descarga, artefactos y opciones de ejecución. En uso normal, las definiciones activas se guardan en `~/.config/ropero/models/<engine>/` y los manifiestos personales en `~/.config/ropero/engines/`. Los TOML del repositorio son ejemplos: Ropero no los carga automáticamente.

```bash
ropero models add
ropero models add --path modelo.toml
ropero install models <modelo>
ropero <modelo>
ropero status --live
```

## Engines y límites

- **llama-server** y **turbo-llama** sirven modelos GGUF.
- **FastFlowLM** delega operaciones a su CLI `flm`; los formatos y el uso de NPU dependen de FastFlowLM y XRT.
- **Ignis** sirve artefactos descargados del engine.
- **Basalt** sirve packs `.basalt`. Ropero sólo lo admite en Linux x86-64. Si la instalación del engine termina bien, registra el TOML incluido de Qwen3.8-Flash-Next si aún no existe; el pack se instala aparte. Basalt requiere CPU AVX2, GPU NVIDIA Blackwell y un driver compatible.
- **Strata** sirve perfiles preparados; Ropero no crea ni descarga perfiles.
- **vLLM**, **MLX-Serve** y **TensorRT-LLM** tienen adaptadores, pero el soporte depende del sistema y del modelo. MLX-Serve requiere macOS y Apple Silicon.
- **AirLLM** tiene soporte parcial; Ropero no ofrece un adaptador de servidor.

Consulta la [referencia detallada de compatibilidad](engine-support.es.md) para conocer requisitos y límites.

## Agentes, IDE y herramientas

`ropero connect --list` muestra las conexiones automáticas, entre ellas OpenCode, Codex, Claude Code, Pi, Hermes, OpenClaw, PicoClaw y VS Code. Primero inicia un modelo con Ropero. La conexión configura el cliente, pero no instala el agente ni inicia el engine.

Zed admite endpoints compatibles con OpenAI y guarda las claves en el llavero del sistema; configúralo desde Agent Settings. La documentación oficial de Cursor no confirma conexiones directas con endpoints locales de Ropero. MicroClaw permite configurar `llm_base_url`. Graphify es una herramienta opcional para analizar repositorios con ayuda de un asistente compatible; no es un engine ni un modelo de Ropero.

Consulta la [referencia de integraciones](agent-integrations.es.md) y la [referencia del CLI](especificacion/ropero-cli.md).

## Activar y desactivar modelos

`ropero models` muestra por defecto los engines instalados y los modelos listos para usar. Si un engine instalado no tiene modelos disponibles, aparece con un aviso. Usa `--all` para consultar todo el catálogo o `--fired` para ver los modelos desactivados. Despedir o reactivar mueve el TOML a la carpeta `fired/` del engine o lo saca de ella; no borra la definición ni los archivos del modelo. Si el nombre ya está activo, Ropero pide otro nombre único antes de reactivarlo.

```bash
ropero models
ropero models --all
ropero models --fired
ropero model <modelo> --fired
ropero model <modelo> --hired
```
