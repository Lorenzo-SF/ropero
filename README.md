# ropero

[English](README.en.md) · **Español**

**ropero** es una CLI que sirve modelos de lenguaje locales con el motor que
mejor rinda en cada máquina. Lee modelos declarados en TOML, arranca el motor
adecuado, comprueba que está listo y expone una puerta compatible con la API de
OpenAI.

> **Especificación vigente:** el resumen posterior del CLI y de configuración
> recibido el 2026-11-16 prevalece sobre los contratos históricos donde haya
> diferencias. Está consolidado en
> [`R-CLI-18…27`, `R-CFG-1…2` y `R-MOD-32…34`](docs-especificacion/ropero-especificacion.md#414-especificación-posterior-del-cli-y-configuración-2026-11-16).
> La tabla de estado en esa sección es una evaluación inicial; los elementos
> parciales o pendientes no deben leerse como funcionalidades terminadas.

## Estado del proyecto

El repositorio contiene el workspace Rust de Ropero. La implementación v1
anterior no forma parte de este checkout:

| Capa | Dónde vive | Estado |
|---|---|---|
| **v1** | No incluida en este checkout | La implementación anterior no forma parte de este árbol de trabajo. Sus contratos históricos se conservan como referencia documental. |
| **v2** | [`crates/`](crates/) | En construcción. Workspace Rust de seis crates; F0 y F1 terminadas. |

El workspace Rust de la v2 conserva los contratos y el estado en disco
documentados para la versión anterior.

Los TOML de [`models/`](models/) se conservan en el repositorio como copia de
referencia y respaldo. **La aplicación no los usa como catálogo**: en uso
normal, `install models`, `models` y `ropero <alias>` leen el catálogo del
usuario en `$XDG_CONFIG_HOME/ropero/models/` (o `~/.config/ropero/models/`).
El usuario debe colocar allí sus TOML. Los overrides `ROPERO_MODELS_DIR` y
`ROPERO_ROOT` se reservan para desarrollo y pruebas.

### Estado de la v2

La v2 es un workspace Rust activo y todavía está en desarrollo; no se presenta
como reemplazo publicado de la v1. Incluye CLI, catálogo TOML y manifiestos de
motores, configuración externa, arranque y parada, instalación, generación de
GGUF, conexiones con agentes y una batería interna de evaluación. La cobertura y
el grado de soporte dependen del comando, el motor y el agente; consulta
`ropero --help`, el changelog y la especificación antes de asumir que un flujo
está completo.

## Compilar y probar

Desde la raíz del repositorio:

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release -p ropero-cli
```

El ejecutable queda en `target/release/ropero`; consulta `--help` para ver los
comandos disponibles.

La CLI funciona desde cualquier directorio porque, en uso normal, resuelve el
catálogo desde la configuración del usuario (no buscando el catálogo del
workspace por directorios ancestrales):

```bash
cargo run --locked -p ropero-cli -- config
cargo run --locked -p ropero-cli -- models --all  # catálogo del usuario
```

### Registrar y probar modelos

`ropero models add` abre un asistente interactivo. También recibe un TOML con
`--path`, TOML por stdin con `--stdin`, o valores y asignaciones repetibles con
`--set`. `ropero template` muestra una guía breve del esquema; Ropero valida el
modelo antes de registrarlo.

`ropero test` permite elegir modelo, grupo y test si se ejecuta sin opciones.
En modo automatizado, requiere `--model` y exactamente uno entre `--test` y
`--test-group`; `--path` indica dónde guardar resultados. Ejemplos:

```bash
ropero test --model qwen --test-group A --path ./resultados
ropero test --model qwen --test A1 --path ./resultados
ropero command qwen
ropero info qwen
ropero status --live
```

### Catálogo, configuración y overrides (v2)

En uso normal, el catálogo es externo en
`$XDG_CONFIG_HOME/ropero/models/<motor>/` (o `~/.config/ropero/models/<motor>/`)
con manifiestos de motor en `<directorio_config>/engines/`. En el primer uso se
crea el directorio de configuración y `config.toml` por defecto de forma
idempotente y **sin sobrescribir**. `logs_dir` por defecto es
`<directorio_config>/logs`.

Precedencia de rutas: `variable de entorno > config.toml > valor por defecto`
(`ROPERO_MODELS_DIR`/`models_dir`, `GGUF_DIR`/`gguf_dir`,
`SAFETENSORS_DIR`/`safetensors_dir`, `ROPERO_BIN_DIR`/`bin_dir`,
`LOG_DIR`/`logs_dir`).

`ROPERO_MODELS_DIR` y `ROPERO_ROOT` se conservan como overrides para desarrollo
y pruebas (incluida la resolución de `<raíz>/engines/` cuando apuntan a
`<raíz>/models/`). Los TOML del repositorio **no se auto-descubren** en uso
normal.

**Migración desde TOML/manifiestos heredados:**

```bash
mkdir -p ~/.config/ropero/models ~/.config/ropero/engines
# copiar o mover modelos por motor
cp /ruta/antigua/models/<motor>/*.toml ~/.config/ropero/models/<motor>/
# copiar manifiestos de motor de usuario
cp /ruta/antigua/engines/*.toml ~/.config/ropero/engines/
```

Comandos de diagnóstico/configuración: `ropero config` (rutas y origen),
`ropero doctor` (estado de configuración y dependencias) y `ropero template`
(plantilla sugerida, sin escribirla). Estos comandos no modifican artefactos de
modelos; el bootstrap idempotente de configuración puede ejecutarse si falta.

`ropero connect --list` muestra las integraciones disponibles. Por ejemplo,
`ropero connect --model qwen --agent opencode` registra el modelo; separa varios
modelos con comas o punto y coma y usa `--remove` para retirarlos. Los
providers OpenCode, Pi, OpenClaw (`claw` es alias), Codex, Hermes y Kilo se
agrupan por engine; VS Code y Copilot usan Custom Endpoint; CodeBuddy y Droid
guardan el endpoint en cada entrada custom. Aider registra endpoint y clave
por modelo en sus archivos oficiales de model settings/metadata. MCode usa su
CLI oficial; como ésta elimina providers completos, `--remove` requiere
seleccionar todos los modelos Ropero de ese engine. Claude Code requiere la API
Anthropic Messages; Ropero muestra las variables necesarias para configurar una
URL compatible y el token correspondiente.

Para Codex, Ropero crea un perfil por engine sin cambiar el provider global.
Conecta los modelos y usa el perfil que imprime el comando, por ejemplo
`codex --profile ropero-turbo-llama -m ornith`. Si usas `CODEX_HOME`, Ropero
escribe ahí la configuración y los perfiles. Codex requiere Responses API;
Ropero rechaza engines que no la declaran soportada.

TRAE y TRAE CN requieren añadir o quitar el modelo desde su panel de modelos;
`ropero connect --agent trae|trae-cn --model <modelo>` imprime los valores
exactos para ese paso, sin editar archivos privados de la aplicación ni mostrar
claves. `claw` es sólo un alias histórico de OpenClaw; no es un agente distinto.
Gemini CLI, Cursor, Kiro, Antigravity y Devin no aparecen en `--list`: no se
encontró una vía documentada compatible con los endpoints servidos por Ropero
y/o una interfaz persistente segura para registrar el modelo.

`ropero config --lang EN` o `--lang ES` guarda el idioma de la interfaz; EN es
el valor inicial. `ropero models add` crea un TOML nuevo mediante asistente,
flags, fichero o stdin; usa `--set niveles.campo=VALOR_TOML` repetible para
añadir campos del esquema, por ejemplo `--set levels.list='["low","high"]'`.
`ropero info <modelo>` muestra la configuración y
`ropero command <modelo>` imprime los argumentos de serving en formato copiable.
`ropero status --live [SEGUNDOS]` refresca la vista en la terminal.

## La documentación

Los dos documentos normativos se leen **juntos**:

| Documento | Responde a |
|---|---|
| [Especificación y requerimientos](docs-especificacion/ropero-especificacion.md) | ¿Qué hace ropero y con qué semántica exacta? |
| [Especificación de construcción](docs-especificacion/ropero-construccion.md) | ¿Cómo se construye, en qué fases y con qué puertas? |

La especificación de requisitos también está disponible en
[inglés](docs-especificacion/ropero-especificacion.en.md).
La [especificación de construcción en inglés](docs-especificacion/ropero-construccion.en.md)
complementa el documento en español.

Las referencias del CLI en [español](docs-especificacion/ropero-cli.md) y
[inglés](docs-especificacion/ropero-cli.en.md), junto con las guías de
compatibilidad de [engines en español](docs/engine-support.es.md) y
[en inglés](docs/engine-support.en.md), documentan los comandos y sus límites.

**Regla de conflicto**: si discrepan sobre *qué debe hacer el sistema*, manda
la especificación; si discrepan sobre *cómo construirlo*, manda el documento de
construcción.

Las decisiones de arquitectura están en:

- [`ADR-001: Por qué Rust`](docs/adr/ADR-001-por-que-rust.md).
- [`ADR-002: Extracción vs reescritura`](docs/adr/ADR-002-extraccion-vs-reescritura.md).
- [`ADR-003: `ropero-code` es referencia, no dependencia`](docs/adr/ADR-003-ropero-code-es-referencia.md).

El [CHANGELOG en español](CHANGELOG.es.md) y su
[versión inglesa](CHANGELOG.md) registran los cambios de este workspace. El
historial de cambios de la v1 no está incluido en este checkout.
