# ropero

Un lanzador de modelos de IA locales. Instalas una vez, y luego escribes
`ropero gemma` y tienes un servidor OpenAI-compatible escuchando en tu
máquina.

No es un framework ni una biblioteca: es un lanzador con opinión sobre cómo se
deberían vivir los modelos locales.

```bash
ropero gemma                     # arranca Gemma 4 en :9999
ropero gemma --vision            # además, lee imágenes
ropero gemma --high              # más razonamiento
ropero status                    # qué está corriendo
ropero stop gemma                # pararlo
```

---

## Instalación

Ropero se instala con una línea. El script descarga la versión publicada, la
extrae y deja el lanzador en el PATH:

```bash
curl -fsSL https://raw.githubusercontent.com/Lorenzo-SF/ropero/main/install.sh | bash
```

Eso instala **el lanzador**. Ni los engines ni los modelos: esos se instalan
después, y a propósito, porque son compilaciones y descargas de gigas que
deben ser una decisión explícita.

```bash
ropero install engines           # llama.cpp, airllm, tensorrt-llm, mlx-lm, flm
ropero install models            # los pesos de los modelos
ropero list                      # qué hay disponible
```

Si el repositorio es privado, el instalador necesita un token:

```bash
curl -fsSL -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://raw.githubusercontent.com/Lorenzo-SF/ropero/main/install.sh | bash
```

### Opciones del instalador

| Flag | Qué hace | Por defecto |
|---|---|---|
| `--prefix <dir>` | dónde se instala ropero | `~/.local/share/ropero` |
| `--bin-dir <dir>` | dónde se crea el symlink | `~/.local/bin` |
| `--version <ref>` | versión a instalar (`main` o un tag) | `main` |
| `--force` | reinstala aunque ya exista | — |
| `--no-link` | no crea el symlink | — |

Los equivalentes como variables de entorno son `ROPERO_PREFIX`,
`ROPERO_BIN_DIR`, `ROPERO_VERSION`, `ROPERO_FORCE`, `ROPERO_NO_LINK` y
`ROPERO_REPO`. El flag siempre manda sobre la variable.

No hace falta `sudo`. Todo va a `--prefix`, `--bin-dir` o `~/.cache`.

---

## Cómo funciona

Ropero tiene cuatro piezas y ninguna más:

```
ropero/
├── install.sh        el instalador de una línea
├── bin/              el lanzador y sus módulos
│   ├── ropero            el CLI
│   ├── 22-models.sh      carga los modelos (los TOML)
│   ├── 21-download-models.sh   la máquina de descarga
│   └── _engines.sh       el registro de engines
├── endpoints/        el servidor OpenAI-compatible
│   └── server.py
└── models/           la configuración de cada modelo
    ├── llama/
    │   ├── gptoss.toml
    │   ├── gemma.toml
    │   └── fired/        (modelos retirados)
    └── ...
```

### 1. Los modelos son datos, no código

Cada modelo es un fichero TOML en `models/<engine>/`. El mismo formato para
todos los engines, activos y retirados:

```toml
[model]
name    = "gemma"
engine  = "llama-server"
context = 131072

[files]
gguf   = "gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf"
mtp    = "mtp-gemma-4-26B-A4B-it.gguf"
mmproj = "gemma-4-26B-A4B-it-mmproj-BF16.gguf"

[download]
repo = "unsloth/gemma-4-26B-A4B-it-qat-GGUF"
file = "gemma-4-26B-A4B-it-qat-UD-Q4_K_XL.gguf"
size = "15.7 GB"

[args]
base = ["--n-gpu-layers", "99", "--jinja", "--temp", "1.0"]
```

Antes esto eran scripts de bash que se cargaban con `source`, y eso convertía
la configuración en código ejecutable. Ahora un modelo es un fichero de datos
que no puede hacer nada por su cuenta, y **el catálogo de descargas sale de
los mismos TOML**: no hay dos listas que se puedan desincronizar. Antes pasaba
—y pasaba— que `ropero install models` bajaba un fichero que el modelo luego
no encontraba.

La plantilla con el esquema completo está en `models/_template.toml`, y
`ropero template` la enseña.

### 2. Sólo los modelos de `models/` se pueden usar

Los que cuelgan de `models/<engine>/fired/` están **retirados**: no se
arrancan, no se descargan y no aparecen en el catálogo. Pero no son
desconocidos:

```console
$ ropero devstral
El modelo 'devstral' ya no está disponible (está retirado).
  engine: llama-server
  config: .../models/llama/fired/devstral.toml
```

La diferencia importa. "No existe" hace que busques otro; "está retirado" te
dice que no busques nada porque no va a funcionar.

### 3. Los engines que no traen endpoint, lo tienen

AirLLM es una biblioteca, no un servidor. TensorRT-LLM y FLM traen uno, pero
ninguno de los dos comprueba la API key. Los tres pasan por
`endpoints/server.py`, un único servidor FastAPI que pone la API key, el
nombre del modelo y el ciclo de vida por igual:

```
opencode ──► :9999 (endpoint, exige la key) ──► backend en 127.0.0.1
```

El único proceso que escucha en la red es el de ropero, y exige la key. El
backend nativo se queda siempre en loopback.

### 4. Todo habla la misma API

`llama-server`, el endpoint unificado y el proxy de auth exponen lo mismo:
`/v1/models`, `/v1/chat/completions`, `/v1/completions`. Por eso cambiar de
engine no cambia tu cliente.

```bash
curl http://localhost:9999/v1/chat/completions \
  -H "Authorization: Bearer $LLAMA_API_KEY" \
  -H 'Content-Type: application/json' \
  -d '{"model":"gemma","messages":[{"role":"user","content":"hola"}]}'
```

---

## Comandos

| Comando | Qué hace |
|---|---|
| `ropero <modelo>` | arranca el modelo |
| `ropero list` | los modelos disponibles y los retirados |
| `ropero status` | qué está corriendo, en qué puerto, con qué RAM |
| `ropero stop <modelo>\|all` | para lo que esté corriendo |
| `ropero install [engines\|models\|link]` | instala engines, modelos o enlaces |
| `ropero config [engine]` | la configuración actual |
| `ropero check` | verifica la instalación (no modifica nada) |
| `ropero template` | enseña la plantilla de un modelo |

`ropero install` sin destino pasa por los engines y luego por los modelos.

### Flags del lanzador

| Flag | Qué hace |
|---|---|
| `--background` | arranca en segundo plano y sigue |
| `--cpu` | fuerza CPU, sin GPU |
| `--port N` | el puerto en el que escuchar |
| `--force` | rotura el puerto si está ocupado |
| `--low` / `--medium` / `--high` | nivel de razonamiento |
| `--vision` | activa la torre de visión (si el modelo la tiene) |
| `--n-cpu-moe N` | ajustes propios de un modelo |

El vocabulario es del modelo, no del CLI: si un modelo no declara `--vision`,
`ropero <modelo> --vision` es un error de uso, no un flag ignorado.

---

## Los modelos

Activos:

| Modelo | Engine | Notas |
|---|---|---|
| `gemma` | llama-server | Gemma 4 26B-A4B, con MTP y visión |
| `gptoss` | llama-server | GPT-OSS 20B, alias `verifier` y `designer` |
| `qwencoder` | llama-server | Qwen3-Coder 30B, alias `coder` |
| `qwenvision` | llama-server | Qwen2.5-VL 7B |
| `agents-a1-4b` | llama-server | InternScience Agents-A1 4B |
| `agents-a1-35b` | llama-server | InternScience Agents-A1 35B-A3B |
| `qwen38-apex` | llama-server | Qwen3.8 27B, con MTP y visión |
| `airllm` | airllm | GPT-OSS 120B sobre AirLLM |
| `metaloss` | mlx_lm | GPT-OSS 20B en MLX (Apple Silicon) |

Los alias son formas de llamar al mismo modelo con otro nivel de razonamiento:
`ropero verifier` es `ropero gptoss --high`.

### Overrides por entorno

| Variable | Qué hace |
|---|---|
| `ROPERO_<MODELO>_CTX` | el contexto del modelo |
| `ROPERO_<MODELO>_GGUF` | otra cuantización del mismo repo |
| `ROPERO_<MODELO>_PORT` | el puerto del modelo |
| `ROPERO_MODEL_OPT_<OPCIÓN>` | el valor de una opción con valor |
| `LLAMA_API_KEY` | la clave de la API (por defecto `sk-local-dev-key`) |
| `HOST` | dónde escuchar (por defecto `0.0.0.0`) |
| `GGUF_DIR` | dónde están los GGUF (por defecto `~/models/gguf`) |
| `SAFETENSORS_DIR` | dónde están los checkpoints (por defecto `~/models/safetensors`) |

Los overrides propios de cada modelo (temperatura del drafter, tipo de KV…)
también existen: `ROPERO_GEMMA_TEMP`, `ROPERO_GEMMA_MTP_NMAX`,
`ROPERO_GPTOSS_KV`… y están declarados en el TOML de cada modelo.

---

## Engines

| Engine | Sistema | Qué necesita |
|---|---|---|
| `llama-server` (llama.cpp) | todos | GPU NVIDIA en Linux, Metal en macOS |
| `airllm` | todos | mucha RAM: inferencia capa a capa |
| `tensorrt-llm` | Linux NVIDIA | un `trtllm-serve` compatible |
| `mlx_lm` | macOS Apple Silicon | `mlx-lm` |
| `fastflowlm` | Linux con NPU AMD | `flm` |

`ropero install engines` instala sólo los que tu máquina soporta.

---

## Requisitos

- **bash 4+**, `curl` o `wget`, `tar`
- **python3 ≥ 3.8**: los modelos se configuran en TOML y su lector es un
  script de Python. Es la única dependencia de ropero que no viene con el
  sistema. Si tienes varios, `ROPERO_PYTHON=/ruta/a/python3` dice cuál.
- engines: cada uno tiene los suyos (ver la tabla de arriba)

---

## Desarrollo

```bash
bash -n bin/ropero                 # sintaxis
python3 bin/_modelconf.py validate --root models   # esquema de los modelos
bash tests/test-argparse.sh        # contrato del CLI (66 casos)
bash tests/test-gemma4.sh          # descarga de bundles (17 casos)
```

Las pruebas son offline: no tocan la red ni la máquina. Todo lo que podría
salir (curl, wget, git, pip…) está bloqueado por stubs que lo anotan si se
llega a llamar.

### Añadir un modelo

1. Copia `models/_template.toml` a `models/<engine>/<nombre>.toml`.
2. Rellena `[model]`, `[files]`, `[download]` y `[args]`.
3. `ropero template` y `models/_template.toml` tienen el esquema completo.
4. `python3 bin/_modelconf.py validate --root models` te dice si te falta algo.

No hay que tocar ningún `.sh`: el catálogo de descargas, la lista de modelos
y el launcher se entero. Un modelo nuevo es un fichero nuevo.

---

## Licencia

Ver el repositorio.
