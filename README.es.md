![Ropero: One CLI. Any Model. Any Engine.](docs/ropero_banner.png)

# Ropero

**Instala y ejecuta modelos de IA en local desde un solo CLI.**

[English](README.md) · **Español** · [Português](README.pt.md) · [Français](README.fr.md) · [Deutsch](README.de.md) · [Italiano](README.it.md) · [日本語](README.ja.md) · [简体中文](README.zh-CN.md) · [한국어](README.ko.md) · [Русский](README.ru.md)

[![CI](https://github.com/Lorenzo-SF/ropero/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Lorenzo-SF/ropero/actions/workflows/ci.yml)

Ropero conecta un catálogo declarativo de modelos con los motores que los
ejecutan. Prepara el motor, inicia y supervisa el servicio y ofrece una API
compatible con OpenAI cuando el engine la soporta.

## Qué ofrece

- Instalación separada de motores (`ropero install engines`) y modelos
  (`ropero install models`).
- Manifiestos TOML para requisitos, descargas, formatos y parámetros.
- Gestión de ejecución, estado, diagnóstico y evaluación de modelos.
- Integraciones con agentes y asistentes compatibles, con límites documentados.

## Empezar

Ropero v2 está en desarrollo. Para compilar desde la raíz del repositorio:

```bash
cargo build --locked --release -p ropero-cli
ropero --help
ropero doctor
ropero install engines
```

El comando de engines muestra las opciones disponibles; también puedes
seleccionar una, por ejemplo `ropero install engines basalt`. La instalación
del modelo va por separado. Consulta la [guía de inicio](docs/quickstart.es.md)
para el flujo completo y los requisitos de cada paso.

Si Basalt se instala correctamente, Ropero registra la definición incluida de
Qwen3.8-Flash-Next en el catálogo activo si todavía no existe. Solo queda
descargar el pack con `ropero install models qwen3.8-flash-next`.

La interfaz del CLI empieza en inglés. Para guardar español, ejecuta
`ropero config --lang ES`; `ropero config --lang EN` vuelve al inglés.

## Elegir una definición de modelo

En el [catálogo de modelos](models/) puedes explorar los ejemplos por motor.
Cada TOML indica qué archivos necesita el modelo, dónde descargarlos y con
qué parámetros arrancarlo. Por ejemplo, la [ficha de groot-o](models/llama-server/groot-o.toml)
incluye el nombre del GGUF y el tamaño de la descarga. Desde la raíz del
repositorio, añade la ficha a tu catálogo activo (por defecto en
`~/.config/ropero/models/`) y descarga los pesos:

```bash
ropero models add --path models/llama-server/groot-o.toml
ropero install models groot-o
```

Las carpetas `fired/` contienen definiciones retiradas. Para crear una ficha
propia puedes partir de la [plantilla de modelo](models/_template.toml).

## Documentación

| Guía | Contenido |
|---|---|
| [Inicio rápido](docs/quickstart.es.md) | Compilar, configurar TOML, instalar y ejecutar |
| [Compatibilidad de engines](docs/engine-support.es.md) | Requisitos, instalación y límites por motor |
| [Integraciones](docs/agent-integrations.es.md) | Agentes, IDE y conexiones automáticas o manuales |
| [Referencia del CLI](docs/especificacion/ropero-cli.md) | Comandos y opciones |
| [Auditoría de licencias](docs/licensing-audit.es.md) | Fuentes upstream y cuestiones pendientes |
| [Contribuir](CONTRIBUTING.md) | Cambios y comprobaciones locales |
| [Seguridad](.github/SECURITY.md) | Reporte privado de vulnerabilidades |

Empieza por la [guía de usuario en español](docs/guide.es.md). Las guías de usuario están disponibles en diez idiomas; las referencias detalladas de compatibilidad e integraciones están actualmente en español e inglés.
