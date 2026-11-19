# Guía de inicio

Esta guía amplía el resumen del [README en español](../README.md). Ropero v2
sigue en desarrollo.

## Compilar y preparar

Desde la raíz del repositorio:

```bash
cargo build --locked --release -p ropero-cli
```

El ejecutable queda en `target/release/ropero`. En los ejemplos, `ropero`
representa ese ejecutable en tu `PATH`.

```bash
ropero --help
ropero config
ropero doctor
ropero install engines
```

El comando de engines muestra los motores instalables. También puedes elegir
uno directamente, por ejemplo `ropero install engines basalt`. Ropero instala
el runtime y las dependencias del motor; los archivos del modelo se instalan
aparte con `ropero install models <modelo>`. Basalt requiere Linux x86-64,
CPU AVX2, GPU NVIDIA Blackwell y un driver compatible. Consulta la [guía de
Basalt](engine-support.es.md#instalar-y-usar-basalt).

## TOML y catálogo

Ropero usa dos tipos de TOML:

- `engines/`: requisitos, detección, instalación y capacidades de cada engine.
- `models/<engine>/`: definición del modelo, formato, descarga, artefactos y
  parámetros de ejecución.

En uso normal, el catálogo activo está en `~/.config/ropero/models/<engine>/`
y los manifiestos de usuario en `~/.config/ropero/engines/`. Los TOML del
repositorio son ejemplos y referencias: Ropero no los carga automáticamente.
Registra un modelo con el asistente o un archivo existente:

```bash
ropero models add
ropero models add --path modelo.toml
```

Después instala el modelo por nombre o alias:

```bash
ropero install models <modelo>
```

Cuando la instalación de Basalt termina correctamente, Ropero registra el TOML
Qwen3.8-Flash-Next incluido en el catálogo activo del usuario si aún no existe.
Instala el pack por separado con `ropero install models
qwen3.8-flash-next`. Hugging Face puede requerir el CLI `hf` y una sesión
autenticada.

La inicialización de la configuración del usuario no sobrescribe archivos
existentes. `ropero config` muestra las rutas activas; variables de entorno
pueden cambiar rutas del catálogo, artefactos, binarios y logs.

## Ejecutar y evaluar

```bash
ropero <modelo>
ropero models
ropero status --live
ropero info <modelo>
ropero command <modelo>
ropero <modelo> remove
ropero test
```

Para conectar un asistente, consulta `ropero connect --list` y la [guía de
integraciones](agent-integrations.es.md). La conexión configura el cliente,
pero no instala el agente ni inicia el engine.

## Comprobaciones de desarrollo

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
```
