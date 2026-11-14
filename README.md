# ropero

**ropero** es una CLI que sirve modelos de lenguaje locales con el motor que
mejor rinda en cada máquina. Lee modelos declarados en TOML, arranca el motor
adecuado, comprueba que está listo y expone una puerta compatible con la API de
OpenAI.

## Dos capas, una transición

El repositorio contiene la v1 funcional y el workspace Rust de la v2 en
construcción:

| Capa | Dónde vive | Estado |
|---|---|---|
| **v1** | [`ropero-code/`](ropero-code/) | Funcional. Bash + Python. Es la versión que se instala y se usa hoy. |
| **v2** | [`crates/`](crates/) | En construcción. Workspace Rust de seis crates; F0 y F1 terminadas. |

La v1 y la v2 conviven hasta F9, como decide
[`ADR-002`](docs/adr/ADR-002-extraccion-vs-reescritura.md). El plan construye la
v2 por capas para conservar los contratos y el estado en disco de la v1.

### Qué tiene ya la v2

- Workspace de seis crates: `ropero-core`, `ropero-runtime`, `ropero-download`,
  `ropero-forge`, `ropero-server` y `ropero-cli`.
- Toolchain fijado a Rust 1.100.0 y edition 2024.
- CI, los dos ADR y el test de arquitectura.
- Modelo de datos, validador del TOML y catálogo, con errores `ruta:línea`.
- Montaje del argv, incluidos `replace`, `requires`, placeholders y alias.
- Comandos `validate`, `list`, `template`, `args`, `levels` y `path`.
- Golden master del argv contra la v1 y dominio puro verificado por test.
- Batería actual: 152 tests pasados y 4 ignorados; `ropero-cli` tiene 389
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

La CLI v2 se puede apuntar a otro catálogo con `ROPERO_MODELS_DIR` (debe
apuntar a la raíz que contiene `models/`):

```bash
ROPERO_MODELS_DIR=/ruta/al/models cargo run --locked -p ropero-cli -- list
```

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

El [CHANGELOG de la v2](CHANGELOG.md) registra los cambios de esta
construcción. El [CHANGELOG de la v1](ropero-code/CHANGELOG.md) sigue vigente
para la versión funcional.
