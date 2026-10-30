# Spec de tarea: ropero 1.0 — refactor del monolito a módulos

- **Estado**: DECIDIDO (contrato cerrado con el usuario; las decisiones marcadas *DECIDIDO (asumido)* son suposiciones del architect que invalidan parte del diseño si cambian)
- **Fecha**: 2026-10-30 · **Base**: `main` limpio @ `f0adfc1` · **Repo**: `/Users/lorenzosanchez/workspace/github/lasaca/ropero`
- **Entregable**: un solo cambio de comportamiento — ropero 1.0 modular, bash 3.2, sin código muerto

---

## 0. Stack y contexto

- **bash 3.2** (el `/bin/bash` de macOS) como objetivo de sintaxis; **python3 >= 3.8** para los módulos python.
- SO objetivo: **macOS (Apple Silicon)** y **CachyOS (Arch)**. Hardware: MSI RTX 5080 (Linux) y Mac Apple Silicon.
- Código actual: `bin/ropero` (6288 líneas, monolito) + `bin/*.sh` auxiliares. La auditoría ya está hecha; este spec es el **contrato**, no el inventario. Los implementadores portan leyendo el código legado (ver §12: se conserva en `bin/_legacy/` hasta F8).
- Fuentes legadas clave (citar al portar):
  - `bin/ropero:6047-6288` — dispatcher/argparse del arranque (`--port`, `--cpu`, `--background`, `--force`, niveles/flags/opciones del modelo).
  - `bin/ropero:1515-1595` — `cmd_use` (contrato del plugin de opencode).
  - `bin/ropero:1398-1455` — `cmd_stop` (stop por puerto, no por nombre).
  - `bin/ropero:4013-4100` — `cmd_install` / `install opencode-plugin`.
  - `bin/ropero:218,245-303,394,512` — `port_listening`, `get_model_on_port`, `_stop_port`, `ropero_managed_ports`.
  - `bin/_modelconf.py` (entero) — cargador TOML: **BUENO, se mantiene** (sólo mudanza + cambios de §7.4).
  - `bin/_ggufcheck.py`, `endpoints/server.py`, `quant/` — buenos, se integran.
  - `opencode-plugin/index.ts` — borrado por error en `84dd9dc`; restaurar con `git show 84dd9dc^:opencode-plugin/index.ts`.
  - `bin/01-env.sh:8-16` — variables de entorno actuales.

## 1. Alcance y no-objetivos

**Alcance**: nueva estructura `bin/lib/` + `bin/lib/engines/` + `bin/python/` + `bin/server/` + `bin/quant/`; dispatcher fino; esquema TOML 1.0; `install.sh` nuevo; comando `ropero test`; restauración del plugin de opencode; limpieza; README/CHANGELOG/\_template.

**No-objetivos**: no cambiar el formato dump de modelconf (DUMP_VERSION=1 se mantiene), no reescribir la lógica de gguffer/endpoint/server.py (se portan), no añadir engines nuevos, no soporte iGPU (se borran sus rastros), no tag de git, no CI nueva fuera de `tests/suite/`.

---

## 2. Árbol de ficheros nuevo

```
ropero/
├── install.sh                      # NUEVO: curl|bash; SÓLO obtiene ropero (§9)
├── README.md                       # reescrito: corto (F8)
├── CHANGELOG.md                    # NUEVO: 1.0.0 (F8)
├── AGENTS.md, .gitignore, .tool-versions
├── docs/specs/refactor-1.0.md      # este spec
├── opencode-plugin/
│   └── index.ts                    # RESTAURADO de 84dd9dc^ (F0)
├── bin/
│   ├── ropero                      # NUEVO: dispatcher fino (§5.1)
│   ├── lib/
│   │   ├── 00-utils.sh             # TODA la presentación (alaja+fallback) y helpers base (§5.2)
│   │   ├── 01-env.sh               # entorno: variables, directorios, OS (§4)
│   │   ├── 02-modelconf.sh         # wrapper bash de bin/python/modelconf.py (§5.3)
│   │   ├── 03-engines.sh           # cargador de engines + contrato (§5.4)
│   │   ├── 04-launch.sh            # arranque/poll/stop/estado/API key (§5.5)
│   │   ├── 05-download.sh          # descargas gguf/checkpoint/aux (§5.6)
│   │   ├── cmd-catalog.sh          # list, template, args, levels, path
│   │   ├── cmd-runtime.sh          # use, stop, status, arranque directo
│   │   ├── cmd-install.sh          # install setup/engines/link/opencode-plugin
│   │   ├── cmd-models.sh           # models / install models (--download|--build)
│   │   ├── cmd-doctor.sh           # doctor (alias check), config
│   │   ├── cmd-gguffer.sh          # gguffer (una sola implementación de [build])
│   │   ├── cmd-test.sh             # ropero test (§8.14)
│   │   └── engines/
│   │       ├── llama-server.sh     # un engine = un fichero (§5.4, §6)
│   │       ├── vllm.sh
│   │       ├── mlx-serve.sh
│   │       ├── airllm.sh
│   │       ├── tensorrt-llm.sh
│   │       └── fastflowlm.sh
│   ├── python/
│   │   ├── modelconf.py            # port de bin/_modelconf.py (mudanza + §7.4)
│   │   ├── ggufcheck.py            # port de bin/_ggufcheck.py (sin cambios de comportamiento)
│   │   └── model_card.py           # port de quant/model-card.py
│   ├── server/
│   │   └── endpoint.py             # port de endpoints/server.py (CLI idéntico)
│   ├── quant/                      # port de quant/ (pipeline + parches)
│   │   ├── README.md, ACRITO.md, modelos.tsv, lista-para-generar.md,
│   │   ├── que-compensa-cuantizar.md, comandos-cuantizar.md, tipos.txt
│   │   ├── cuantizar-todos.sh, verificar-gguf.sh, subir-hf.sh, todo.sh
│   │   ├── quant-tipos.sh, aplicar-parche-motor.sh
│   │   └── patches/*.patch
│   └── _legacy/                    # F1..F7: código viejo SOLO como referencia de portado (F8 lo borra)
├── models/
│   ├── _template.toml              # reescrito: esquema 1.0 completo (§7)
│   ├── llama/*.toml                # SIN CAMBIOS (migración §7.5)
│   ├── mlx/{metalnemotron,metalqwen}.toml   # NUEVOS (§7.6); rm *.toml.txt
│   ├── airllm/*.toml               # sin cambios obligatorios
│   ├── tensorrt/fired/             # sin cambios
│   └── vllm/                       # NUEVO (vacío, .gitkeep)
└── tests/
    ├── prueba1.md, prueba2.md, prueba3.md, prueba_larga.md, prueba_gorda.md
    ├── juez.md, gen_corpus_largo.py          # se conservan
    ├── suite/                        # NUEVO: suite bash sin red/GPU (§13)
    └── fakes/                        # NUEVO: binarios fake en PATH (§13)
```

**Regla de añadir un engine = añadir un fichero** `bin/lib/engines/<id>.sh` que cumpla la interfaz de §5.4 (y, si tiene modelos, `models/<dir>/`). Nada más.

### 2.1 Orden de carga del dispatcher

```
bin/ropero
  1. resuelve ROPERO_ROOT (sige symlinks de $0 SIN readlink -f: bucle readlink + cd/pwd)
  2. source lib/00-utils.sh        (siempre)
  3. source lib/01-env.sh          (siempre)
  4. LAZY por comando: 02-modelconf.sh → 03-engines.sh → 04-launch.sh → cmd-<grupo>.sh
```

El dispatcher tiene la tabla de ruteo COMPLETA desde F1 (comando → fichero `cmd-*.sh` y función `rp_cmd_<comando>`). Si el fichero aún no existe (fases intermedias), error claro `comando <X> aún no portado (fase <N>)` y exit 2. Así ningún implementador posterior toca `bin/ropero`.

Ruteo: `install|engines|link → cmd-install.sh`; `models → cmd-models.sh`; `use|stop|status → cmd-runtime.sh`; `list|template|args|levels|path → cmd-catalog.sh`; `doctor|check|config → cmd-doctor.sh`; `gguffer → cmd-gguffer.sh`; `test → cmd-test.sh`; primer argumento que no es comando conocido ni empieza por `-` → arranque directo de modelo (`cmd-runtime.sh`, §8.2).

---

## 3. Contrato bash 3.2 (constraints duras)

- **PROHIBIDO**: `local -n`/namerefs, `declare -A`/arrays asociativos, `mapfile`/`readarray`, `${var,,}`/`${var^^}`, `&>>`, `coproc`. El objetivo es `/bin/bash` 3.2 de macOS.
- **PERMITIDO**: arrays indexadas, `local`, `case`, `[[ ]]`, `printf -v`, `while IFS= read -r`.
- Leer la salida de modelconf/gguffer: bucles `while IFS=$'\x1f' read -r ...` (NUNCA mapfile). El argv de un modelo se consume línea a línea sin word-splitting.
- Shebang `#!/usr/bin/env bash`. Guarda inicial: `(( BASH_VERSINFO[0] >= 3 ))` o error.
- Si alguna funcionalidad futura exigiera bash>=4: ADR de re-exec (`exec bash4 "$0" "$@"`), NO namerefs a ciegas. Hoy no hay ninguna.
- Comentarios y mensajes de usuario en español (convención del repo).

---

## 4. Entorno — `bin/lib/01-env.sh`

Source seguro (sólo defaults, cero ejecución). `rp_env_setup` hace los `mkdir -p`.

| Variable | Default | Uso |
|---|---|---|
| `ROPERO_ROOT` | auto (raíz de la instalación, resuelta por el dispatcher) | modelos, plugin, tests, quant |
| `GGUF_DIR` | `$HOME/models/gguf` | GGUFs + auxiliares en `$GGUF_DIR/<modelo>/` |
| `SAFETENSORS_DIR` | `$HOME/models/safetensors` | checkpoints (mlx, airllm, vllm) |
| `LOG_DIR` | `$HOME/.logs/ropero` | logs de arranque `<modelo>.log` |
| `STATE_DIR` | `$HOME/.local/state/ropero` | `api_key`, `ports/<puerto>` (runtime) |
| `LLAMA_DIR` | `$HOME/llama.cpp` | clone del fork del motor llama |
| `BIN_DIR` | `$HOME/.local/bin` | symlinks de binarios (`ropero`, `llama-server`) |
| `HOST` | `127.0.0.1` | bind de todos los servers |
| `PORT` | `9999` | slot GPU/dGPU |
| `CPU_PORT` | `9998` | slot CPU (sólo Linux; en macOS NO se gestiona) |
| `LLAMA_API_KEY` | ver §5.5 `rp_api_key` | la MISMA key para todos los engines |
| `ROPERO_START_TIMEOUT` | `300` (seg) | timeout de arranque/espera de modelo |
| `HF_TOKEN` | — | se pasa a `hf download` |

Overrides por modelo (los resuelve modelconf, se mantienen): `ROPERO_<NOMBRE>_CTX`, `ROPERO_<NOMBRE>_GGUF`, `ROPERO_MODEL_OPT_<K>`.

**Puertos gestionados** (para `status`/`stop all`): `$PORT` + `$CPU_PORT` (sólo Linux) + los `[model].port` pineados de los TOML activos + los puertos ad-hoc con fichero en `$STATE_DIR/ports/`. **No hay iGPU**: ningún puerto ni código para ella.

`rp_detect_os` → `macos | cachyos | arch | linux | otro` (F5/doctor lo usan para brew/pacman).

---

## 5. API pública de los módulos

Convención general: prefijo `rp_` para bash; errores de uso → exit 2; fallos → exit 1; éxito → 0. Toda salida máquina (líneas `KEY=`, dumps, argv) va con `printf` crudo, NUNCA por las funciones de presentación.

### 5.1 `bin/ropero` (dispatcher)

- `ropero [help|-h|--help]` → ayuda general, exit 0.
- `ropero --version` → `ropero 1.0.0`, exit 0. *(DECIDIDO (asumido))*
- `ropero <cmd> --help|-h` → ayuda del comando, exit 0 (todos los comandos).
- Comando desconocido / opción que empieza por `-` sin comando → mensaje corto sin volcado, exit 2.
- Ruteo y carga lazy según §2.1.

### 5.2 `bin/lib/00-utils.sh` — TODA la presentación

Detección: `rp_ui_init` cachea si `alaja` está en PATH y si el destino soporta ANSI (`NO_COLOR`, no-TTY → plano). **`status` y `list` funcionan sin alaja** (fallback printf ANSI equivalente).

| Función | Args | Comportamiento |
|---|---|---|
| `rp_has_alaja` | — | exit 0 si alaja disponible |
| `rp_msg <tipo> <texto>` | tipo ∈ ok\|error\|warn\|info\|notice\|debug | `alaja success\|error\|warning\|info\|notice\|debug` o fallback ANSI `✓/✗/⚠/·` |
| `rp_header <texto>` | — | `alaja header` o regla `═══` |
| `rp_step <n> <total> <texto>` | — | progreso de fase (instaladores) |
| `rp_confirm <pregunta>` | — | pregunta sí/no con `read -r`; respeta `RP_YES=1` (--yes); default no |
| `rp_die <código> <msg>` | — | `rp_msg error` + exit código |
| `rp_usage_error <cmd> <msg>` | — | error de uso + hint `ropero <cmd> --help`, exit 2 |
| `rp_require_cmd <cmd> [hint]` | — | exit 1 con hint si falta un binario |
| `rp_now` | — | `date +%Y%m%d-%H%M%S` (nombres de informes) |

### 5.3 `bin/lib/02-modelconf.sh` + `bin/python/modelconf.py`

modelconf.py mantiene su CLI (`dump|args|validate|build|aux`, `--root`, formato dump v1 con separador 0x1F) y su parser TOML propio compatible con python 3.8. Cambios 1.0 en §7.4. El wrapper:

| Función | Salida | Notas |
|---|---|---|
| `rp_mc_load` | carga el catálogo en arrays paralelas `MODEL_*` (names/engine/format/context/port/path/alias/origin) | un dump, `while IFS=$'\x1f' read -r`; nombres con alias expandidos; detecta choque alias↔nombre de otro modelo → exit 1 |
| `rp_model_exists <nombre>` | exit 0/1 | incluye alias |
| `rp_model_field <nombre> <campo>` | valor por stdout | |
| `rp_model_argv <nombre> [nivel] [flags...] [opts k=v...]` | argv, UN ARGUMENTO POR LÍNEA | passthrough a `modelconf.py args` |
| `rp_model_build_argv <nombre> [--out ruta]` | argv del gguffer | passthrough a `modelconf.py build` |
| `rp_model_aux <nombre>` | líneas `tipo\|repo\|file\|name\|size\|use` | |

### 5.4 `bin/lib/03-engines.sh` + interfaz de engine

Un engine = `bin/lib/engines/<id>.sh` con OCHO funciones obligatorias `rp_eng_<id>_<hook>`. El cargador verifica las ocho; si falta una, error nombrándola.

| Hook | Firma | stdout | exit |
|---|---|---|---|
| `label` | — | etiqueta humana (`llama-server · fork turboquant`) | 0 |
| `compatible` | — | — (razón en stderr si no) | 0 compatible / 1 no |
| `deps` | — | comandos externos, separados por espacio | 0 |
| `install` | `[--force]` | progreso por `rp_msg`/`rp_step` | 0/1 |
| `resolve` | — | ruta absoluta del binario (PATH→BIN_DIR→LLAMA_DIR) | 0/1 (1 = no instalado) |
| `probe` | `<puerto>` | `ok` \| `down` \| `missing` | 0 sólo si ok |
| `native_auth` | — | `yes` \| `no` | 0 |
| `launch` | `<modelo> <puerto> <logfile> <argv...>` | PID del proceso arrancado | 0/1 (diagnóstico en stderr) |

Contrato de `launch`: arranca EN BACKGROUND con stdout/stderr al `<logfile>`; añade sólo binario+subcomando+host+puerto+auth (lo específico de §6); el modelo y sus flags ya vienen en `<argv...>` (salida de `rp_model_argv`, placeholders resueltos). Polling de readiness: lo hace `04-launch.sh`, no el engine.

API del cargador: `rp_engine_list` (ids), `rp_engine_load <id>`, `rp_engine_call <id> <hook> [args...]`, `rp_engine_for_model <modelo>` (lee el TOML), `rp_engine_canonical <id>` (`mlx_lm`→`mlx-serve`).

### 5.5 `bin/lib/04-launch.sh` — runtime

| Función | Comportamiento |
|---|---|
| `rp_start_model <modelo> <bg> <cpu> <force> <port> <nivel> <flags> <opts>` | porta `start_model` (bin/ropero:1600+): resuelve engine+argv, gestiona puerto (`--port` > pineado > slot), arranca con `rp_eng_<id>_launch`, espera con `rp_wait_ready`, escribe `$STATE_DIR/ports/<puerto>` (MODEL, ENGINE, PID, PORT, LEVEL, TS, LOG) |
| `rp_wait_ready <puerto> <engine> [timeout]` | poll del PROBE del engine (`/health` sin key; si no, `/v1/models` con `Authorization: Bearer $LLAMA_API_KEY`) hasta que responde Y el model-id del modelo está en `/v1/models`. Espera al MODELO, no sólo al puerto. Timeout → exit 1 con las últimas líneas del log |
| `rp_stop_port <puerto>` | mata POR PUERTO (árbol del PID del estado / del proceso que escucha). **Nunca `pkill -f` global** |
| `rp_managed_ports` | puertos de §4, uno por línea |
| `rp_model_on_port <puerto>` | model-id vía `/v1/models` (Bearer); fallback al fichero de estado |
| `rp_port_listening <puerto>` | `lsof -iTCP:<p> -sTCP:LISTEN` |
| `rp_api_key` | si `LLAMA_API_KEY` fijada, usa esa; si no, lee `$STATE_DIR/api_key`; si no existe, genera `sk-ropero-<hex16>` (`openssl rand -hex 16`, fallback `/dev/urandom`+`od`), chmod 600, y la exporta |
| `rp_state_clear <puerto>` | borra el fichero de estado al parar |

`--cpu` (sólo arranque directo y Linux): sirve en `$CPU_PORT`. En macOS `--cpu` es error de uso exit 2. *(DECIDIDO (asumido))*

### 5.6 `bin/lib/05-download.sh`

Porta `ropero_download_gguf` / `ropero_download_safetensors` / `_ropero_dir_has_weights` (bin/00-utils.sh:119-500). Checkpoints: `hf download <repo> --local-dir "$SAFETENSORS_DIR/<repo>"` (respeta `HF_TOKEN`, `excludes`). GGUF+aux: a `$GGUF_DIR/<modelo>/`. Nunca re-descargan lo que ya pesa lo que dice `size`.

### 5.7 `bin/server/endpoint.py` y `bin/python/*.py`

- `endpoint.py`: port de `endpoints/server.py` con **CLI idéntico** (`--backend airllm|tensorrt|flm --model-path --model-id --host --port --api-key --backend-port [--max-seq-len]`). Es el ÚNICO expuesto; el backend siempre en 127.0.0.1 interna. FastAPI+uvicorn obligatorios, httpx opcional (urllib fallback).
- `ggufcheck.py`: port sin cambios de comportamiento (`FICHERO` valida; `--info` metadata).
- `model_card.py`: port de `quant/model-card.py` (README.md + CUANTIZATION.md del GGUF fabricado).

---

## 6. Tabla de engines

| engine id | dir models/ | gate OS/hw | deps externas | instalación (`install engines`) | binario | server propio | auth nativa | vía endpoint.py |
|---|---|---|---|---|---|---|---|---|
| `llama-server` | `llama/` | macOS (Metal) · Linux (CUDA) | git, cmake, ninja, curl, toolchain C/C++ | clone **HTTPS** de `AtomicBot-ai/atomic-llama-cpp-turboquant` → `bin/quant/aplicar-parche-motor.sh` (parches de `bin/quant/patches/`) → cmake+ninja (CUDA en Linux/NVIDIA, Metal en macOS) → link `$BIN_DIR/llama-server` | `$LLAMA_DIR` build → `$BIN_DIR` | sí (OpenAI) | **sí**: `--api-key` | no |
| `vllm` | `vllm/` | Linux + `nvidia-smi` OK | python≥3.9, uv, driver CUDA | `uv tool install vllm` | `vllm` | sí (OpenAI) | **sí**: `--api-key` | no |
| `mlx-serve` | `mlx/` | macOS arm64 | uv (para mlx-lm), brew | 1) `uv tool install mlx-lm` (o `uv pip`) 2) `brew install ddalcu/mlx-serve` — EN ESTE ORDEN | `mlx-serve` | sí (OpenAI) | **sí**: `--api-key-env` + `--api-key-strict` (`/health` queda abierto) | no |
| `airllm` | `airllm/` | macOS · Linux (RAM ≥ pesos) | python≥3.8, uv | `uv tool install airllm` | — (librería pura) | no | no | **sí** (`--backend airllm`) |
| `tensorrt-llm` | `tensorrt/` | Linux + `nvidia-smi` OK | python, CUDA | wheel de TRT-LLM vía uv *(DECIDIDO (asumido): canal exacto a verificar al portar)* | `trtllm-serve` | sí | no (sin auth) | **sí** (`--backend tensorrt`) |
| `fastflowlm` | `flm/` | Linux | flm | instalación del canal oficial de FLM *(DECIDIDO (asumido): ídem)* | `flm` | sí (`flm serve`) | no (no chequea key) | **sí** (`--backend flm`) |

Detalle de `launch` por engine (argv del modelo ya resuelto):

- **llama-server**: `llama-server --host $HOST --port <p> --api-key "$LLAMA_API_KEY" <argv>`.
- **vllm**: `vllm serve "$SAFETENSORS_DIR/<[model].path>" <argv> --host $HOST --port <p> --api-key "$LLAMA_API_KEY"`.
- **mlx-serve**: `export ROPERO_MLX_API_KEY="$LLAMA_API_KEY"; mlx-serve serve <argv> --host $HOST --port <p> --model "$SAFETENSORS_DIR/<[model].path>" --api-key-env ROPERO_MLX_API_KEY --api-key-strict`. La key NUNCA va en argv (la tabla de procesos es world-readable). `/health` abierto = probe sin key.
- **airllm / tensorrt-llm / fastflowlm**: `python3 "$ROPERO_ROOT/bin/server/endpoint.py" --backend <id> --model-path <ruta> --model-id <modelo> --host $HOST --port <p> --api-key "$LLAMA_API_KEY" --backend-port <p+1000> <argv>`.

`compatible` se evalúa SIN instalar nada (gate de la tabla). `install` de cada engine instala SUS deps sólo al seleccionarse (nada de "instala todo").

---

## 7. Esquema TOML 1.0 (`models/*.toml`)

Un modelo es DATO. Nada de lo que hay en un TOML se ejecuta. El validador (`modelconf.py validate`) es la fuente de verdad; `_template.toml` documenta TODO lo siguiente.

### 7.1 Secciones y claves

| Sección | Clave | Tipo | Obligatoria | Notas |
|---|---|---|---|---|
| `[model]` | `name` | string | sí | único global (activos+fired), kebab-case |
| | `engine` | string | sí | `llama-server\|vllm\|mlx-serve\|airllm\|tensorrt-llm\|fastflowlm`; `mlx_lm` se ACEPTA y mapea a `mlx-serve` con aviso |
| | `context` | int | sí | tokens de contexto |
| | `alias` | string | no | alias público adicional |
| | `port` | int | no | puerto pineado |
| | `format` | `gguf\|safetensors` | no (gguf) | |
| | `path` | string | sí si safetensors, prohibido si gguf | ruta del checkpoint relativa a `$SAFETENSORS_DIR` |
| `[files]` | `gguf` | string | sí si gguf | nombre de fichero (nunca ruta absoluta); va en `$GGUF_DIR/<name>/` |
| | `mtp`, `mmproj`, `chat_template` | string | no | ídem; son los RUNTIME aux |
| `[download]` | `repo` | string | sí* | `owner/repo` |
| | `file` | string | sí si gguf; opcional/ignorado si safetensors | safetensors = checkpoint entero vía `hf download` |
| | `name`, `size`, `excludes[]` | — | no | name=destino, size=informativo |
| | `manual` | bool | no | si true, no se ofrece en `install models` |
| `[download.aux.<tipo>]` | `file` | string | sí | tipo ∈ `mtp\|mmproj\|chat_template\|imatrix\|tokenizer` |
| | `repo` | string | no | si el aux vive en OTRO repo; default `[download].repo` |
| | `name`, `size` | — | no | |
| | `use` | `run\|build` | no | default por tipo: mtp/mmproj=`run`; chat_template/imatrix/tokenizer=`build` |
| `[build]` | (ver §7.3) | — | — | la receta del gguffer |
| `[levels]` | `list[]`, `default` | — | no | niveles de razonamiento |
| `[flags]` | `list[]` | — | no | flags booleanos que acepta el modelo |
| `[options]` | `list[]` | — | no | opciones con valor |
| `[options.default]` | `<opción>` | string | no | debe estar en `[options].list` |
| `[aliases]` | `<alias>` = nivel | string | no | el nivel debe estar en `[levels].list` |
| `[limits]` | `min_ram_gb` | número >0 | no | check antes de arrancar (macOS pagina) |
| `[args]` | `base[]` | lista | sí para llama-server | argv del server que sirve el modelo; un elemento = un argumento |
| `[args.level.<n>]` | `base[]`, `reasoning_effort`, `chat_template_kwargs{}` | — | no | el atajo se traduce a `--chat-template-kwargs <json>` |
| `[args.flag.<f>]` | `base[]`, `requires[]` | — | no | `requires` ⊆ claves de `[files]` |
| `[args.option.<o>]` | `base[]` (admite `${value}`), `replace` | — | no | `replace="--flag"` sustituye la pareja del base |

\* `[download]` es obligatoria salvo que el modelo tenga `[build]` (se fabrica). Con `[build]`, `[download]` es opcional y sólo lleva los auxiliares.

### 7.2 Placeholders y overrides

En cualquier `base[]`: `${gguf}` `${mtp}` `${mmproj}` `${chat_template}` `${ctx}` `${value}` `${model_path}` (NUEVO 1.0: `$SAFETENSORS_DIR/<[model].path>`, sólo safetensors) `${env:VAR}` `${env:VAR:-defecto}`.

Orden de montaje del argv: `[args].base` → `[args.level.<nivel>]` → `[args.flag.<f>]` (en el orden pedido) → `[args.option.<o>]`. Overrides de entorno: `ROPERO_<NOMBRE>_CTX`, `ROPERO_<NOMBRE>_GGUF`, `ROPERO_MODEL_OPT_<K>`.

### 7.3 `[build]` — la receta (UNA implementación: `ropero gguffer`)

Claves (= flags del gguffer): `repo`, `gguf`, `gguf_file`, `quant` (string o lista), `imatrix`, `imatrix_repo`, `imatrix_file`, `out`, `out_principal`, `from_subdir`, `include`, `intermedio` (f16|bf16), `keep_source`, `keep_intermedio`, `no_mtp`, `force`, `size` (informativo). Clave desconocida → AVISO en dump/build/validate. `[build]` y `[download]` pueden coexistir (§8.11).

### 7.4 Cambios en modelconf.py para 1.0 (lo único que se toca)

1. `KNOWN_ENGINES` += `vllm`; `KNOWN_ENGINE_DIRS` += `vllm`; engine id `mlx_lm` → **`mlx-serve`** (dir `mlx` se mantiene). `mlx_lm` se acepta como legacy y se canóniza con aviso en stderr.
2. Placeholder nuevo `${model_path}` (resolución como §7.2).
3. `validate`: `[download].file` obligatoria SÓLO con `format=gguf`; con safetensors es opcional y se avisa si existe.
4. Nada más. `DUMP_VERSION=1` y el formato de 32 campos NO cambian.

### 7.5 Reglas de migración

1. `models/llama/*.toml` activos (`muse, nemotron, qwen36, qwencoder, gptoss, ornith, granite, qwen38`): **SIGUEN VALENDO SIN CAMBIOS**. Criterio: `python3 bin/python/modelconf.py validate --root models` → OK, 0 problemas.
2. `models/airllm/*.toml`: sin cambios obligatorios (su `[download].file` se tolera; opcionalmente se elimina).
3. `models/mlx/fired/mlx_lm.toml`: una línea, `engine = "mlx-serve"`.
4. `models/mlx/*.toml.txt`: se BORRAN (eran comentarios, no TOML); sustituidos por §7.6.
5. `models/tensorrt/fired/`, `models/llama/fired/`: sin cambios de esquema.
6. `models/_template.toml`: reescrito documentando §7 entero (todas las claves, todos los aux, ejemplo mínimo y ejemplo con build+download).

### 7.6 TOML de los dos modelos mlx-serve (del comando real en uso)

`models/mlx/metalnemotron.toml`:

```toml
# metalnemotron — NVIDIA-Nemotron-3.5-Lightning-30B-A3B-4bit (MLX, mlx-serve)
# Comando real servido (el engine añade serve/--host/--port/--model/--api-key-env):
#   mlx-serve serve --mtp --host 127.0.0.1 --port 9999 --model <SAFETENSORS_DIR>/... \
#     --os-reserve-gib 1 --ctx-size 133072 --max-resident-models 1 --no-vision \
#     --tokenize-cache-entries 16 --int8-qk --prefill-decode-share 0.50 --kv-quant 4 \
#     --skip-mem-preflight --prefix-cache-disk 20GB --max-tokens 16384

[model]
name    = "metalnemotron"
engine  = "mlx-serve"
context = 133072
format  = "safetensors"
path    = "mlx-community/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-4bit"

[files]

[download]
repo = "mlx-community/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-4bit"
size = "~18 GB"          # checkpoint entero: hf download, sin .file

[limits]
min_ram_gb = 48          # orientativo

[args]
base = [
    "--mtp",
    "--os-reserve-gib", "1",
    "--ctx-size", "${ctx}",
    "--max-resident-models", "1",
    "--no-vision",
    "--tokenize-cache-entries", "16",
    "--int8-qk",
    "--prefill-decode-share", "0.50",
    "--kv-quant", "4",
    "--skip-mem-preflight",
    "--prefix-cache-disk", "20GB",
    "--max-tokens", "16384",
]
```

`models/mlx/metalqwen.toml`: idéntico salvo:

```toml
[model]
name    = "metalqwen"
path    = "mlx-community/Qwen3.8-27B-4bit"

[download]
repo = "mlx-community/Qwen3.8-27B-4bit"
size = "~15 GB"

[limits]
min_ram_gb = 32          # orientativo
```

*(DECIDIDO (asumido): metalqwen usa el mismo perfil de serve que metalnemotron — así se usa en realidad; `min_ram_gb` orientativo, ajustable sin invalidar el diseño.)*

---

## 8. Contrato de comandos

Exit codes globales: **0** éxito · **1** fallo de ejecución · **2** error de uso (typo, flag desconocido, falta selección sin TTY) · **130** SIGINT. Todos aceptan `--help`.

### 8.1 `ropero help` / `ropero` (sin args)
Ayuda general con la lista de comandos. Exit 0.

### 8.2 `ropero <modelo> [opciones]` — arranque directo
`ropero <modelo> [--<nivel>] [--<flag>] [--<opt>=<v>] [--port N] [--cpu] [--background] [--force]`
Niveles/flags/opciones deben estar DECLARADOS en el TOML (`[levels]/[flags]/[options]`); si no, error de uso exit 2 (no se ignoran). `--cpu` sólo Linux. `--port` requiere número (repetido: gana el último con aviso). Foreground por defecto; `--background` + `--force` como hoy. Salida: progreso humano + al final `ROPERO_PORT=<n>` si background. Exit 0/1/2.

### 8.3 `ropero use <modelo> [--port N] [--nivel L] [--flag F]...`
**Contrato del plugin de opencode — NO romper**:
- Idempotente por (modelo, nivel): si el puerto ya sirve ese modelo con ese nivel → `✓ '<modelo>' ya está sirviéndose en :<p>` + **última línea stdout cruda `ROPERO_PORT=<n>`** + exit 0, sin reiniciar. Nivel distinto → reinicia.
- Si el puerto lo ocupa otro modelo: lo para POR PUERTO y arranca el pedido.
- Espera a que el MODELO esté listo (no sólo el puerto), respeta `ROPERO_START_TIMEOUT`.
- Exit 0 (y `ROPERO_PORT=`) / 1 arranque / 2 uso o modelo desconocido.

### 8.4 `ropero args <modelo> [--<nivel>] [--<flag>] [--<opt>=v]...`
Imprime el argv final, UN ARGUMENTO POR LÍNEA (misma gramática que §8.2). Exit 0/2.

### 8.5 `ropero levels <modelo>`
Un nivel por línea (los de `[levels].list`). Sin niveles → salida vacía, exit 0. Modelo desconocido → exit 2. (El plugin parsea `^[A-Za-z0-9._-]+$` por línea.)

### 8.6 `ropero path <modelo>`
Líneas `CLAVE=ruta` con las rutas resueltas (checkpoint/gguf, aux run, binario del engine, log). Exit 0/2.

### 8.7 `ropero list [--all]`
Una línea por modelo activo: **dos espacios** + `<name>` + columnas (engine, format, context, size/desc). `--all` incluye fired (marcados). **Formato contratado por el plugin** (regex `^\s{2}[A-Za-z0-9._-]+\s`). Exit 0.

### 8.8 `ropero template`
Vuelca `models/_template.toml` a stdout. Exit 0.

### 8.9 `ropero config` / `ropero config <engine>`
Sin args: tabla `ENGINE · COMPATIBLE · INSTALADO · MODELOS` (los 6 engines; instalado = `resolve`/`probe`, silenciosa ante engines rotos). Con engine: detalle del engine. Exit 0.

### 8.10 `ropero stop [modelo|all]`
`all` (default): para TODOS los puertos gestionados; si no hay nada, imprime "Nada que detener" y exit 0. `<modelo>`: busca su puerto y mata SÓLO ese. Máx. un argumento; no tiene flags (error uso exit 2). Modelo desconocido → exit 2. Exit 0/1.

### 8.11 `ropero status [modelo] [--watch]`
Sin args: tabla de puertos gestionados: `PUERTO · ENGINE · MODELO · ESTADO` (probe real). `status <modelo>`: exit 0 si está sirviéndose, 1 si no, 2 desconocido. `--watch`: refresco cada 2s hasta Ctrl-C (exit 0). **Funciona sin alaja** y sin ANSI si `NO_COLOR`/no-TTY.

### 8.12 `ropero doctor` (alias `ropero check`)
Chequea: bash (>=4 aviso), python3>=3.8, curl/wget, tar, git, uv, hf, jq, cmake, ninja, OS/paquetero, directorios (GGUF/SAFETENSORS/LOG/STATE escribibles), API key, engines (compatible/resolve/probe por slot), modelos activos presentes en disco. Salida ✓/⚠/✗. Exit 0 si no hay ✗ crítico; 1 si hay (crítico = python3, directorios, o NINGÚN engine compatible instalado; engines individuales no instalados son ⚠).

### 8.13 `ropero install ...` / `ropero engines` / `ropero models` / `ropero link`

- `ropero install` (sin subcomando) = `install setup` + `install engines` + `install models`. **Sin modo unattended**: sin TTY y sin selección explícita → error exit 2.
- `ropero install setup [--yes] [--dry-run]`: sólo lo que falte, interactivo por defecto (`--yes` no pregunta; `--dry-run` sólo lista). Instala: python3>=3.8, curl/wget, tar, git, uv, `hf` (`uv tool install 'huggingface_hub[cli]'`), jq, cmake, ninja; Linux→base-devel (pacman); macOS→brew (si falta brew, ofrece instalarlo). bash>=4: SÓLO check+aviso. **CUDA sólo al elegir un engine que lo pida** (eso pasa en `install engines`). NO instala opencode ni alaja.
- `ropero install engines [engine...] [--force]`: menú multi-selección (o lista explícita) de engines compatibles; cada engine instala SUS deps (§6). Sin TTY sin selección → exit 2. `ropero engines` = alias.
- `ropero install models [modelo...] [--download|--build] [--force]`: ver abajo. `ropero models` = alias.
- `ropero install link`: recrea symlinks en `$BIN_DIR` de los binarios de engines instalados (p.ej. `llama-server` del build). No acepta selección.
- `ropero install opencode-plugin`: copia `opencode-plugin/index.ts` → `~/.config/opencode/plugins/ropero-modelo/index.ts` (copia, NO symlink: el watcher de opencode no sigue symlinks); backup `.bak` si difiere; avisa si el config de opencode no declara el plugin.

**`install models`**: sin modelo → menú interactivo (tamaños incluidos); sin TTY sin selección → exit 2. Con `[download]` Y `[build]` en el mismo TOML: sin flag → **error "especifica --download o --build", exit 2**; con flag ejecuta ese camino; en menú interactivo se pregunta. Sólo `[download]`: baja gguf/checkpoint + aux (según `use`). Sólo `[build]`: fabrica vía gguffer con la receta (`rp_model_build_argv`).

### 8.14 `ropero test <modelo>` (NUEVO)
`ropero test <modelo> [--prueba P] [--todas] [--yes] [--timeout S] [--port N] [--nivel L]`

- Pruebas: `prueba1` (default), `prueba2`, `prueba3`, `prueba_larga`, `prueba_gorda`. `--todas` = las cinco en secuencia, UN .md por prueba.
- Requiere `opencode` y `jq` en PATH; si falta → mensaje claro nombrando el binario, exit 1.
- Confirmación previa (salvo `--yes`) avisando de que ejecuta `ropero stop` y `pkill -9 llama-server opencode mcode claude`.
- Flujo: `mktemp -d` + `cd`; guarda `$PWD` original; arranca el modelo en background con log a `engine.log` del tmp; poll de readiness (`/v1/models` con Bearer o `/health`) con `--timeout` (default 300 s); `opencode run "$(cat $ROPERO_ROOT/tests/<prueba>.md)" --format json > prueba.json`; extrae session id con jq (`select(.sessionID or .session_id)`); `opencode session export $session > session.json`; compone en el `$PWD` original `ropero-test-<modelo>-<prueba>-<ts>.md` (ts = `rp_now`), AUTOCONTENIDO con bloques delimitados: `## Juez` (tests/juez.md) + `## Prueba` (el prompt) + `## Log del engine` (engine.log) + `## Salida` (prueba.json) + `## Sesión` (session.json).
- `trap` de limpieza (EXIT/INT/TERM): para el modelo y borra el tmpdir.
- Exit 0 éxito; 1 fallo (timeout, opencode rc≠0, sin session id); 2 uso. `--todas` continúa tras fallo y resume (`N ok · M fallos`); exit 1 si alguno falló.
- **Ubicación de los prompts**: `tests/` del repo, resueltos por ruta absoluta `$ROPERO_ROOT/tests/` (el test corre en un tmpdir). Justificación en ADR-2.

### 8.15 `ropero gguffer`
`ropero gguffer [--repo R] [--gguf G] [--gguf-file F] [--quant Q]... [--imatrix I] [--imatrix-repo R] [--imatrix-file F] [--out O] [--out-principal P] [--from-subdir S] [--include PAT]... [--intermedio f16|bf16] [--keep-source] [--keep-intermedio] [--no-mtp] [--force]`

Una SOLA implementación de fabricar GGUFs (la usa el CLI a mano y `[build]` de los TOML). Puerto del cuerpo actual de `cmd_gguffer` + pipeline de `quant/`: clona/baja pesos → convierte (intermedio f16/bf16) → imatrix si procede → cuantiza → verifica con `bin/python/ggufcheck.py` → genera model card con `bin/python/model_card.py`.

**BUG a corregir (1.0)**: si la cuantización pedida (`--quant`) coincide con la del GGUF fuente (`--gguf`), NO es error y NO se limpia nada: se limita a descargar/colocar ese gguf (y sus aux) en su sitio, avisando "cuantización ya aplicada en el repo". Exit 0.

---

## 9. `install.sh` (raíz) — el instalador de una línea

```
curl -fsSL https://raw.githubusercontent.com/Lorenzo-SF/ropero/main/install.sh | bash
install.sh [--version <ref>] [--dir <ruta>] [--tarball <fichero>] [--help]
```

- **SÓLO obtiene ropero.** Nada más (no engines, no modelos, no opencode, no alaja).
- `--version` default `main` (cualquier ref de GitHub). `--dir` default `$HOME/.local/share/ropero` *(DECIDIDO (asumido))*. `--tarball` instala desde un fichero local (tests/offline).
- Obtención: tarball `https://codeload.github.com/Lorenzo-SF/ropero/tar.gz/<ref>` (o `--tarball` local); fallback `git clone --depth 1` HTTPS. `GITHUB_TOKEN`/`GH_TOKEN` viaja en cabecera (`curl -H "Authorization: Bearer ..."`; git via `http.extraheader`).
- **Validación de rutas del tarball**: rechaza entradas absolutas o con `..` antes de extraer.
- Idempotente: si `<dir>` ya es ropero (mismo origen), actualiza (re-descarga a tmp + sustitución atómica); nunca rompe una instalación previa de otra cosa (error claro).
- Crea `$BIN_DIR/ropero` → `<dir>/bin/ropero` (symlink; `mkdir -p $BIN_DIR`).
- Verifica que lo extraído contiene `bin/ropero`.
- Si `python3` < 3.8 (o no existe): aviso claro → `ropero install setup`. NO instala nada.
- Exit 0 / 1 (fetch, extracción, validación) / 2 (uso).

---

## 10. ADRs

**ADR-1 — bash 3.2, no re-exec.** DECIDIDO. El objetivo es el `/bin/bash` 3.2 de macOS: sin namerefs, sin `declare -A`, sin mapfile (§3). Si en el futuro algo lo exigiera, se escribe un ADR de re-exec (`exec bash4 "$0" "$@"` al inicio) — hoy ningún módulo lo necesita y el re-exec añade un modo de fallo (bash4 ausente) sin contrapartida.

**ADR-2 — prompts de `ropero test` en `tests/`.** DECIDIDO (asumido). Los cinco `prueba*.md` + `juez.md` + `gen_corpus_largo.py` son DATO que evoluciona junto (juez y corpus se editan con las pruebas), ya viven en `tests/`, y el tarball de install.sh los lleva con el repo. Duplicarlos en `bin/` crearía dos verdades; incrustarlos en el script los haría ineditables. `ropero test` los resuelve por `$ROPERO_ROOT/tests/` (absoluto, porque corre en un tmpdir).

**ADR-3 — mlx-serve directo, con `--api-key-env`.** DECIDIDO. mlx-serve tiene server OpenAI propio: va DIRECTO (sin endpoint.py). La auth se pasa con `--api-key-env ROPERO_MLX_API_KEY --api-key-strict` (ropero exporta la key al arranque): la key nunca aparece en argv — la tabla de procesos es world-readable — y `--api-key-strict` exige la key incluso desde loopback. `/health` queda abierto, así que el probe de readiness no necesita key. Instalación en dos pasos (mlx-lm por uv ANTES de mlx-serve por brew ddalcu/mlx-serve) porque mlx-serve depende de mlx-lm.

**ADR-4 — plugin de opencode restaurado y su contrato.** DECIDIDO. `opencode-plugin/index.ts` se recupera íntegro de `git show 84dd9dc^:opencode-plugin/index.ts` (arranca el modelo pedido vía `ropero use` y acota la petición de título). El plugin consume TRES salidas del CLI que quedan congeladas como contrato: (1) `ropero list` con nombres a dos espacios, (2) `ropero levels <modelo>` un nivel por línea, (3) `ropero use` idempotente que termina imprimiendo `ROPERO_PORT=<n>` cruda en stdout. `install opencode-plugin` COPIA a `~/.config/opencode/plugins/ropero-modelo/` (el watcher inotify de opencode no sigue symlinks).

**ADR-5 — qué se borra.** DECIDIDO. Fuera (queda en historial): `bin/_authproxy.py` (muerto); funciones muertas del monolito (`ropero_engine_cmd`, `ropero_engine_pinned_model`, `ropero_engine_env`, `_ropero_launch_engine`, `ropero_engine_internal_port`); lógica invertida de `ropero_install_mlx`; clone por SSH del motor (→HTTPS); `_render_status_compact` sin fallback sin alaja; `local -n` de 22-models.sh; selección como string de `cmd_install`; no-detección de choque alias↔nombre; bug del gguffer (§8.15); iGPU (todo rastro); tests obsoletos (`test-*.sh`, `_fakes.sh`, `bench_*`, `probar-*`, `genera_tsv.sh`, `bench_ropero.py`); `ropero.d/`, `prueba.json`, `scripts/` (sólo `__pycache__` sin trackear), `erl_crash.dump`. `tests/bench_ropero.sh` se consulta vía `git show main:tests/bench_ropero.sh`, no se porta.

---

## 11. Plan de implementación por FASES

Implementadores SECUENCIALES; cada fase tiene ficheros PROPIETARIOS (nadie edita ficheros de otra fase). Entre F1 y F7 el código legado vive en `bin/_legacy/` como referencia de portado (no ejecutable, `chmod -x`); F8 lo borra. Es esperado que comandos aún no portados den el error de §2.1 hasta que llegue su fase.

| Fase | Ficheros propietarios (crea/mueve/edita) |
|---|---|
| **F0 Limpieza y restauración** | borra: `bin/_authproxy.py`, `tests/{test-*.sh,_fakes.sh,bench_*,probar-*,genera_tsv.sh,bench_ropero.py}`, `ropero.d/`, `prueba.json`, `scripts/`, `erl_crash.dump`. Crea: `opencode-plugin/index.ts` (de `git show 84dd9dc^:opencode-plugin/index.ts`), dirs `bin/lib/engines`, `bin/python`, `bin/server`, `models/vllm/.gitkeep` |
| **F1 Dispatcher+utils+env** | `bin/ropero` (nuevo), `bin/lib/00-utils.sh`, `bin/lib/01-env.sh`; `git mv bin/{ropero,00-utils.sh,01-env.sh,_common.sh,_engines.sh,_styles.sh,20-compile-llama.sh,21-download-models.sh,22-models.sh}` → `bin/_legacy/` |
| **F2 modelconf+catálogo** | `git mv bin/_legacy/_modelconf.py bin/python/modelconf.py` (+§7.4), `bin/lib/02-modelconf.sh`, `bin/lib/cmd-catalog.sh`, `models/_template.toml`, `models/mlx/{metalnemotron,metalqwen}.toml` (+rm `*.toml.txt`), `models/mlx/fired/mlx_lm.toml` (1 línea) |
| **F3 Engines+doctor** | `bin/lib/03-engines.sh`, `bin/lib/engines/*.sh` (los 6), `git mv endpoints/server.py bin/server/endpoint.py` (ajustes de ruta mínimos), `bin/lib/cmd-doctor.sh` (doctor+config) |
| **F4 Runtime** | `bin/lib/04-launch.sh`, `bin/lib/cmd-runtime.sh` (use/stop/status/arranque directo) |
| **F5 Install** | `install.sh` (raíz), `bin/lib/cmd-install.sh`, `bin/lib/cmd-models.sh`, `bin/lib/05-download.sh` |
| **F6 gguffer+quant** | `git mv quant/* bin/quant/`, `git mv bin/_legacy/_ggufcheck.py bin/python/ggufcheck.py`, `git mv bin/quant/model-card.py bin/python/model_card.py`, `bin/lib/cmd-gguffer.sh` |
| **F7 test+suite** | `bin/lib/cmd-test.sh`, `tests/suite/**`, `tests/fakes/**` |
| **F8 Docs y cierre** | `README.md`, `CHANGELOG.md`, `git rm -r bin/_legacy/`, barrido de referencias muertas |

### Criterios de aceptación por fase (todo SIN red ni GPU)

- **F0**: `git status` sólo muestra lo listado; `diff <(git show 84dd9dc^:opencode-plugin/index.ts) opencode-plugin/index.ts` → vacío; no existen los borrados.
- **F1**: `bash -n bin/ropero bin/lib/*.sh` OK; `shellcheck -S warning bin/ropero bin/lib/*.sh` limpio; **`/bin/bash bin/ropero help`** (bash 3.2 real) exit 0; `bin/ropero --version` → `ropero 1.0.0`; `bin/ropero falso` → exit 2; `bin/ropero list` → error de "aún no portado" exit 2 (esperado).
- **F2**: `python3 bin/python/modelconf.py validate --root models` → `OK: N modelo(s)`; `bin/ropero list` muestra `metalnemotron`/`metalqwen` con nombre a 2 espacios; `bin/ropero args qwen38` imprime `--n-gpu-layers`/`66`; `bin/ropero args qwen38 --high` añade el nivel; `bin/ropero levels qwen38` → 4 líneas; `bin/ropero path qwen38`; `bin/ropero template | head -3`; `bin/ropero args metalnemotron` contiene `--mtp`, `--kv-quant`, `4`, `--prefix-cache-disk`, `20GB`, `--ctx-size`, `133072`.
- **F3**: `bash -n`/shellcheck de engines+doctor; `bin/ropero doctor` termina (con engines no instalados: avisos, exit según §8.12); `bin/ropero config` tabla con los 6 engines; con un fake `mlx-serve` ad-hoc en PATH, `config` lo reporta instalado; `python3 bin/server/endpoint.py --help` exit 0.
- **F4**: con fake `llama-server` en PATH (contrato §13.2) y `ROPERO_START_TIMEOUT=20`: `bin/ropero use qwen38 --port 41001` → exit 0 y ÚLTIMA línea stdout exactamente `ROPERO_PORT=41001` (sin ANSI); re-ejecutar → "ya está sirviéndose" y exit 0 sin reiniciar; con `--nivel high` → reinicia; `bin/ropero status` lista el slot; `bin/ropero status qwen38` exit 0; `bin/ropero stop` exit 0 y el puerto queda libre; `bin/ropero stop all`. Todo bajo `/bin/bash`.
- **F5**: `bash install.sh --help` exit 0; `install.sh --tarball <tgz fixture> --dir /tmp/rp --help-less` con `HOME` falso instala y crea el symlink; `install.sh --version` sin red → error limpio exit 1; `bin/ropero install setup --dry-run` exit 0 lista dependencias sin instalar nada; `echo | bin/ropero install` (sin TTY) → exit 2; `bin/ropero install models gptoss --build` → error "sólo tiene [download]" (uso), y `--download` planifica descarga (con `--dry-run` si se añade *(asumido)*); `bin/ropero install opencode-plugin` con `HOME` falso copia index.ts + backup.
- **F6**: `bin/ropero gguffer --help` exit 0; `bash -n bin/quant/*.sh`; `python3 bin/python/ggufcheck.py --help`-equivalente y `model_card.py --help` exit 0; `modelconf.py build` de un TOML con `[build]` produce el argv esperado (fixture de suite); caso "quant pedido == quant del repo" NO borra nada (fixture de suite).
- **F7**: `bin/ropero test --help` exit 0; `bin/ropero test qwen38` sin `opencode` en PATH → error claro exit 1; `tests/suite/run.sh` → TODO verde (§13).
- **F8**: `README.md` < 200 líneas con quickstart (curl install.sh → `ropero install` → `ropero use` → `ropero test`); `CHANGELOG.md` con 1.0.0; `grep -rn "_legacy\|_authproxy" bin/ README.md` sin resultados; `grep -rn "mlx_lm" bin/` sólo en modelconf.py (mapeo legacy); re-ejecutar TODOS los ACs de F1–F7.

---

## 12. Estrategia de test — `tests/suite/` (bash, sin red ni GPU)

- **Corredor**: `tests/suite/run.sh` — aísla `HOME`, `GGUF_DIR`, `SAFETENSORS_DIR`, `STATE_DIR`, `LOG_DIR`, `BIN_DIR` en tmpdirs; antepone `tests/fakes/` al `PATH`; fija `NO_COLOR=1`; resume `N ok · M fallos`; exit 1 si hay fallo. `jq` real es requisito de la suite (si falta: skip con aviso).
- **Fakes** (`tests/fakes/`, ejecutables): 
  - `llama-server` — acepta cualquier argv; escucha en `--port`; `GET /v1/models` → `{"data":[{"id":"<valor de --served-model-name o primer .gguf>"}]}` (python http.server mínimo); `GET /health` → 200; log a stderr.
  - `mlx-serve` — igual, más `serve` como subcomando y `--api-key-env` aceptado.
  - `vllm` — ídem.
  - `opencode` — `run <prompt> --format json` → JSON con `sessionID` fijo; `session export <id>` → JSON fijo.
  - `hf`, `git`, `uv`, `brew`, `pacman` — registran el argv en un log y exit 0 (para probar install/setup sin red).
- **Casos** (un fichero por contrato en `tests/suite/cases/`): ayuda y exit codes de todos los comandos; formato de `list` (regex del plugin); `levels`/`args`/`path`; `validate` del catálogo real + fixture TOML; `use` idempotente y `ROPERO_PORT`; cambio de nivel reinicia; `stop` por puerto y `stop all`; `status`/`status <m>`/`--watch` (1 iteración); `doctor` y `config` con fakes; `install setup --dry-run`; `install` sin TTY → 2; `install models` con `[download]+[build]` → exige flag; `gguffer` caso "quant ya aplicada" (fixture dirs tmp, sin GPU); `test` flujo completo con fake opencode → genera `ropero-test-<m>-prueba1-*.md` autocontenido (verifica bloques Juez/Prueba/Log/Salida/Sesión); `install.sh --tarball` con fixture.
- **Fuera de la suite**: `tests/bench_ropero.sh` (referencia por historial) y las pruebas reales con GPU (las hace el usuario con `ropero test`).

---

## 13. Verificación esperada (comandos literales, sin red ni GPU)

```bash
bash -n bin/ropero bin/lib/*.sh bin/lib/engines/*.sh          # ok
shellcheck -S warning bin/ropero bin/lib/*.sh bin/lib/engines/*.sh   # limpio
/bin/bash bin/ropero help && /bin/bash bin/ropero --version   # bash 3.2 real
python3 bin/python/modelconf.py validate --root models        # OK: N modelo(s)
bin/ropero list && bin/ropero args qwen38 && bin/ropero path qwen38
bin/ropero levels qwen38 && bin/ropero template | head -3
bin/ropero doctor; echo "doctor rc=$?"
bin/ropero status; bin/ropero status qwen38
bin/ropero install setup --dry-run
bin/ropero test --help
tests/suite/run.sh                                            # todo verde
```

---

## 14. Decisiones asumidas — DECIDIDO (asumido)

Si alguna cambia, revisar la parte afectada antes de implementar:

1. Dir de instalación de `install.sh`: `$HOME/.local/share/ropero` + symlink `$HOME/.local/bin/ropero`.
2. `STATE_DIR=$HOME/.local/state/ropero` (api_key + `ports/`).
3. `ropero --version` nuevo; alias `check`→doctor, `models`/`engines` top-level se mantienen.
4. `install setup --dry-run` y `install.sh --tarball <fichero>` (testeabilidad offline).
5. `--cpu` en macOS = error de uso (no hay slot CPU en macOS).
6. `use` idempotente por (modelo, nivel): nivel distinto reinicia (lo espera el plugin).
7. metalqwen comparte perfil de serve con metalnemotron; `min_ram_gb` orientativos (48/32).
8. Canales de instalación de tensorrt-llm y fastflowlm (wheel/binary exacto) a verificar al portar F3.
9. `ropero test` `--timeout` default 300 s; `--todas` continúa tras fallos.
10. Si falta brew, `install setup` lo ofrece (script oficial) tras confirmar.
11. `template` = volcado de `models/_template.toml` (sin flags).
12. Generación de API key: `openssl rand -hex 16` con fallback `/dev/urandom`+`od`.

## 15. Lo que NO debe hacer el ejecutor

- NO añadir dependencias nuevas más allá de las listadas (§0/§6/§8.13); NO tocar `models/llama/*.toml` activos; NO cambiar el formato dump de modelconf; NO usar namerefs/`declare -A`/mapfile; NO introducir red ni GPU en ningún AC; NO crear ficheros fuera de los listados en §11; NO hacer commit/push/deploy (eso lo decide Mavis).
- qa marca cada AC con ✅/❌/⚠️ y evidencia; reviewer reporta findings con ruta/línea y corrección concreta. Ninguno edita código.

## 16. Riesgos y preguntas abiertas

- **Compatibilidad del plugin**: el contrato de §8.3/§8.5/§8.7 es lo más frágil del refactor; la suite lo fija con regex exactas.
- **bash 3.2**: los ACs usan `/bin/bash` real de macOS para que un slip de sintaxis salga en la fase, no en producción.
- **Canal exacto de TRT-LLM/FLM** (asunción 8): bloqueante sólo para `install engines` de esos dos engines, no para el resto del contrato.
- **`hf download` y `--local-dir`**: verificar semántica de subdirectorio `<repo>` al portar F5 (el destino debe quedar exactamente en `$SAFETENSORS_DIR/<path-de-[model].path>`).
