# Ropero

**Instala y ejecuta modelos de IA en local.**

[English](README.en.md) · **Español**

Ropero organiza los modelos locales y los motores que los ejecutan. Eliges un
modelo; Ropero prepara el motor declarado para él, lo arranca, comprueba que
esté listo y ofrece una interfaz compatible con la API de OpenAI cuando el
motor la soporta.

## Qué ofrece

- **Instalación separada de motores y modelos.** `ropero install engines`
  prepara el software necesario para ejecutar modelos; `ropero install
  models` obtiene sus artefactos por descarga o generación, según el modelo.
- **Configuración declarativa con TOML.** Cada modelo indica su motor, formato,
  ubicación o descarga, opciones y perfiles de uso.
- **Ejecución gestionada.** Ropero inicia el motor adecuado, asigna un puerto,
  espera a que responda y ayuda a consultar o detener los servicios.
- **Herramientas de catálogo y diagnóstico.** Permite registrar modelos,
  revisar configuración y dependencias, imprimir comandos y ejecutar una
  batería de evaluación.
- **Conexiones con asistentes de código.** Incluye integraciones que registran
  modelos o endpoints en herramientas compatibles; sus límites se documentan
  por agente.

El soporte depende del motor, el sistema y el comando. Consulta la [guía de
compatibilidad](docs/engine-support.es.md) antes de elegir un motor.

## Empezar

Este checkout contiene la v2 de Ropero, todavía en desarrollo. Para compilarla
desde la raíz del repositorio:

```bash
cargo build --locked --release -p ropero-cli
```

El ejecutable queda en `target/release/ropero`. En los ejemplos siguientes,
`ropero` representa ese ejecutable en tu `PATH`.

```bash
ropero --help
ropero config
ropero doctor
ropero install engines
```

El comando de engines muestra los motores instalables. También puedes elegir
uno directamente, por ejemplo:

```bash
ropero install engines basalt
```

Basalt requiere Linux x86-64, CPU AVX2, GPU NVIDIA Blackwell y un driver
compatible. Ropero instala su runtime y dependencias; el modelo se descarga
aparte. Consulta [instalar y usar Basalt](docs/engine-support.es.md#instalar-y-usar-basalt)
para conocer los requisitos y el flujo completo.

## Modelos y TOML

Ropero usa dos tipos de TOML:

- Los manifiestos de `engines/` describen cada motor, sus requisitos y cómo
  detectarlo o instalarlo.
- Los TOML de `models/<motor>/` describen modelos, descargas, artefactos y
  parámetros de ejecución.

En uso normal, el catálogo activo está en
`~/.config/ropero/models/<motor>/`; los manifiestos de usuario van en
`~/.config/ropero/engines/`. Los TOML que ves en este repositorio son ejemplos
y referencias: Ropero no los carga automáticamente. Registra un modelo con el
asistente `ropero models add`, o pasa un fichero TOML existente:

```bash
ropero models add
ropero models add --path modelo.toml
```

Después instala el modelo indicando su nombre o alias:

```bash
ropero install models <modelo>
```

Para Basalt, el repositorio incluye una definición de Qwen3.8-Flash-Next. La
guía de compatibilidad explica cómo copiarla al catálogo, instalarla y lanzar
el modelo. Las descargas desde Hugging Face pueden requerir el CLI `hf` y una
sesión autenticada.

La configuración del usuario se inicializa sin sobrescribir archivos
existentes. `ropero config` muestra las rutas activas; las variables de entorno
pueden sobrescribir rutas como el catálogo, los artefactos, los binarios y los
logs.

## Usar Ropero

Con el motor y el modelo instalados, arranca el modelo por su nombre:

```bash
ropero <modelo>
ropero models
ropero status --live
ropero info <modelo>
ropero command <modelo>
ropero <modelo> remove
```

`ropero test` permite evaluar un modelo o un grupo de pruebas. Para conectar
asistentes de código, consulta `ropero connect --list` y la [referencia del
CLI](docs-especificacion/ropero-cli.md).

## Estado y documentación

Este repositorio contiene el workspace Rust de Ropero v2. Está en desarrollo y
no se presenta todavía como sustituto publicado de la implementación v1. El
grado de avance varía según cada comando e integración.

| Documento | Para qué sirve |
|---|---|
| [Guía de compatibilidad](docs/engine-support.es.md) | Requisitos, instalación y límites por motor |
| [Referencia del CLI](docs-especificacion/ropero-cli.md) | Comandos, opciones y comportamiento detallado |
| [Especificación](docs-especificacion/ropero-especificacion.md) | Contratos y requisitos del producto |
| [Especificación de construcción](docs-especificacion/ropero-construccion.md) | Arquitectura, fases y puertas de calidad |
| [Changelog](CHANGELOG.es.md) | Cambios de este workspace |

La especificación también está disponible en [inglés](docs-especificacion/ropero-especificacion.en.md),
y la [guía de compatibilidad en inglés](docs/engine-support.en.md) acompaña a
su versión en español.

## Compilar y validar

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```
