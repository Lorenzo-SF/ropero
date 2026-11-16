# ropero

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

## Dos capas, una transición

El repositorio contiene la v1 funcional y el workspace Rust de la v2 en
construcción:

| Capa | Dónde vive | Estado |
|---|---|---|
| **v1** | [`ropero-code/`](ropero-code/) | Funcional. Bash + Python. Es la versión que se instala y se usa hoy. **Es sólo referencia** para la v2 (ver [ADR-003](docs/adr/ADR-003-ropero-code-es-referencia.md)): la v2 se desarrolla como si esa carpeta no existiera. |
| **v2** | [`crates/`](crates/) | En construcción. Workspace Rust de seis crates; F0 y F1 terminadas. |

La v1 y la v2 conviven hasta F9, como decide
[`ADR-002`](docs/adr/ADR-002-extraccion-vs-reescritura.md). El plan construye la
v2 por capas para conservar los contratos y el estado en disco de la v1.

Los TOML de [`models/`](models/) se conservan en el repositorio como copia de
referencia y respaldo. **La aplicación no los usa como catálogo**: en uso
normal, `install models`, `list` y `ropero <alias>` leen el catálogo del
usuario en `$XDG_CONFIG_HOME/ropero/models/` (o `~/.config/ropero/models/`).
El usuario debe colocar allí sus TOML. Los overrides `ROPERO_MODELS_DIR` y
`ROPERO_ROOT` se reservan para desarrollo y pruebas.

### Qué tiene ya la v2

- Workspace de seis crates: `ropero-core`, `ropero-runtime`, `ropero-download`,
  `ropero-forge`, `ropero-server` y `ropero-cli`.
- Toolchain fijado a Rust 1.100.0 y edition 2024.
- CI, los tres ADR y el test de arquitectura.
- Modelo de datos, validador del TOML y catálogo, con errores `ruta:línea`.
- Montaje del argv, incluidos `replace`, `requires`, placeholders y alias.
- Comandos `validate`, `list`, `template`, `args`, `levels` y `path`.
- Golden master del argv contra la v1 y dominio puro verificado por test.
- Batería actual: 156 tests pasados y 2 ignorados; `ropero-cli` tiene 394
  líneas y `ropero-core` no usa E/S.

La siguiente fase es F2 (motores): los identificadores de motor ya existen,
pero todavía no hay manifiestos de motor. Después siguen runtime, estado y
puertos, descargas, forja GGUF, endpoint, instalación v2 y convivencia final.

## Usar la v1

```bash
git clone https://github.com/Lorenzo-SF/ropero.git
cd ropero/ropero-code
bash install.sh --local
```

Eso deja `~/.local/bin/ropero → <repo>/ropero-code/bin/ropero` (lo único en el
PATH) y `~/.local/share/ropero → <repo>/ropero-code/bin`.

```bash
ropero install setup      # dependencias base (python3, git, uv, hf, jq, cmake…)
ropero install engines    # elige motores e instala SUS dependencias
ropero install models     # elige modelos (descarga o fabrica el GGUF)
```

`ropero install` sin argumentos hace las tres cosas en orden. El resto de la
guía de uso está en [`ropero-code/README.md`](ropero-code/README.md).

## Compilar y probar la v2

Desde la raíz del repositorio:

```bash
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --workspace --release
```

La CLI funciona desde cualquier directorio porque, en uso normal, resuelve el
catálogo desde la configuración del usuario (no buscando el catálogo del
workspace por directorios ancestrales):

```bash
cargo run --locked -p ropero-cli -- config
cargo run --locked -p ropero-cli -- list --all  # catálogo del usuario
```

### Catálogo, configuración y overrides (v2, working tree)

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

## La documentación

Los dos documentos normativos se leen **juntos**:

| Documento | Responde a |
|---|---|
| [Especificación y requerimientos](docs-especificacion/ropero-especificacion.md) | ¿Qué hace ropero y con qué semántica exacta? |
| [Especificación de construcción](docs-especificacion/ropero-construccion.md) | ¿Cómo se construye, en qué fases y con qué puertas? |

**Regla de conflicto**: si discrepan sobre *qué debe hacer el sistema*, manda
la especificación; si discrepan sobre *cómo construirlo*, manda el documento de
construcción.

Las decisiones de arquitectura están en:

- [`ADR-001: Por qué Rust`](docs/adr/ADR-001-por-que-rust.md).
- [`ADR-002: Extracción vs reescritura`](docs/adr/ADR-002-extraccion-vs-reescritura.md).
- [`ADR-003: `ropero-code` es referencia, no dependencia`](docs/adr/ADR-003-ropero-code-es-referencia.md).

El [CHANGELOG de la v2](CHANGELOG.md) registra los cambios de esta
construcción. El [CHANGELOG de la v1](ropero-code/CHANGELOG.md) sigue vigente
para la versión funcional.
