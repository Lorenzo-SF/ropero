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
| `ropero levels <modelo>` | los niveles de razonamiento que **declara** ese modelo |
| `ropero args <modelo> [nivel]` | el argv final que recibiría el server, **sin arrancar nada** |
| `ropero stop <modelo>\|all` | para lo que esté corriendo |
| `ropero use <modelo>` | asegura que **ese** modelo sea el que se sirve, y espera a que esté listo |
| `ropero install [engines\|models\|link]` | instala engines, modelos o enlaces |
| `ropero config [engine]` | la configuración actual |
| `ropero check` | verifica la instalación (no modifica nada) |
| `ropero template` | enseña la plantilla de un modelo |
| `ropero gguffer` | fabrica un GGUF desde un repo de HuggingFace (descarga → convierte → cuantiza → verifica) |

`ropero install` sin destino pasa por los engines y luego por los modelos.

`ropero levels` y `ropero args` son de **preguntar, no de arrancar**: el
nivel (`--low` / `--medium` / `--high`) solo existe si el modelo lo declara
en su TOML, y un nivel que no declara es un error de uso que deja el
modelo sin arrancar. Por eso hay que poder preguntar antes.

### `ropero use`: que el modelo que pides sea el que te responde

El server de llama.cpp **no valida el campo `"model"` del body**: atiende con
el modelo que tenga cargado. Así que si pides `qwen` por HTTP y hay un
`gptoss` cargado, te contesta el `gptoss`. Sin error, sin aviso.

`ropero use <modelo>` cierra ese hueco:

- Si ese modelo ya se está sirviendo en el puerto destino, no toca nada y
  sale con 0 (idempotente: se puede llamar en cada petición).
- Si hay **otro** modelo en ese puerto, lo para **por puerto** (no por
  nombre: `ropero stop <modelo>` mata en el primer puerto gestionado que
  encaje, que puede no ser el suyo) y arranca el pedido.
- Espera a que el modelo esté **listo de verdad**, no a que el puerto
  escuche (esto ya lo hacía el arranque en background).

El puerto es, por orden: `--port`, el `MODEL_PORT` del modelo, y el puerto
base. Con `--port` el modelo se sirve ahí aunque esté pineado, que es lo que
necesita un cliente que habla con un único endpoint.

```bash
ropero use qwen --port 9999
```

### `ropero gguffer`: fabricar el GGUF en casa

Los GGUFs que publica la gente (bartowski, unsloth) los hace el mismo
`llama-quantize` que lleva el motor de llama.cpp. Este comando es para
cuando quieres el tuyo: con **tu** imatrix, con el GGUF de otro modelo, o
con un quant que nadie ha hecho.

```bash
ropero gguffer --list-quants               # los quants que acepta el motor
ropero gguffer --repo Qwen/Qwen3.8-27B --quant IQ3_XS
ropero gguffer --repo Qwen/Qwen3.8-27B --quant IQ3_XS --imatrix mi.imatrix
ropero gguffer --repo Qwen/Qwen3.8-27B --quant IQ3_XS --no-mtp
```

#### El imatrix: también se baja solo

El imatrix es un fichero aparte del modelo (unos 200 MB) que publica otra
gente. Se puede pasar con `--imatrix` (una ruta local) o dejar que el gguffer
lo baje él, con el repo y el fichero:

```bash
ropero gguffer --repo Qwen/Qwen3.8-27B --quant IQ3_XS \
    --imatrix-repo bartowski/Qwen3.8-27B-GGUF \
    --imatrix-file Qwen3.8-27B-imatrix.gguf
```

Se cachea en `~/.cache/ropero/gguffer/imatrix/<owner>__<repo>/`, así que la
segunda vez no se vuelve a bajar. El mismo imatrix sirve para cualquier quant
del mismo modelo base: es una tabla de importancia por tensor, no depende del
quant.

#### `--out`: dónde acaba el GGUF

El lanzador busca cada modelo en `$GGUF_DIR/<carpeta>/<fichero>`, que es lo que
dicen `[model] name` y `[files] gguf` de su `.toml`. `--out` acepta tres
formas:

| Qué le pasas | Dónde acaba |
|---|---|
| `--out mi-modelo` | `$GGUF_DIR/mi-modelo-<quant>.gguf` |
| `--out ~/models/gguf/qwen` | ahí, con el nombre generado |
| `--out ~/models/gguf/qwen/Qwen3.8-27B-IQ2_M.gguf` | exactamente ahí |

La tercera es la que interesa: **el nombre lo pone el TOML, no el quant**. El
GGUF que fabrique el gguffer se llama `<repo>-<quant>.gguf`, y eso casi nunca
coincide con lo que el `.toml` espera (`Qwen3.8-27B-IQ2_M.gguf`, con punto y
todo). Con la ruta exacta el fichero sale directamente donde el modelo lo va a
buscar, sin renombrar nada a mano.

Una cuantización por modelo, con los quant que salen de la VRAM de la
máquina (15.300 MiB) y de la calidad por byte. La última columna es la ruta
completa que espera el `.toml` de cada modelo:

| Modelo | Repo de los pesos (sin cuantizar) | Quant | Dónde acaba |
|---|---|---|---|
| `qwen` | `Qwen/Qwen3.8-27B` | `IQ2_M` | `$GGUF_DIR/qwen/Qwen3.8-27B-IQ2_M.gguf` |
| `qwen36` | `Qwen/Qwen3.6-35B-A3B` | `IQ3_XXS` | `$GGUF_DIR/qwen36/Qwen3.6-35B-A3B-UD-IQ3_XXS.gguf` |
| `qwencoder` | `Qwen/Qwen3-Coder-30B-A3B-Instruct` | `IQ3_XXS` | `$GGUF_DIR/qwencoder/Qwen3-Coder-30B-A3B-Instruct-UD-IQ3_XXS.gguf` |
| `ornith` | `ornith-ai/Ornith-1.5-35B-A3B` | `IQ2_M` | `$GGUF_DIR/ornith/Ornith-1.5-35B-A3B-IQ2_M.gguf` |
| `nemotron` | `nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16` | `IQ3_XXS` | `$GGUF_DIR/nemotron/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-UD-IQ3_XXS.gguf` |
| `laguna` | `poolside/Laguna-XS-2.1` | `IQ3_M` | `$GGUF_DIR/laguna/Laguna-XS-2.1-APEX-I-Mini.gguf` |
| `gemma` | `google/gemma-4-26B-A4B-it` | `IQ3_M` | `$GGUF_DIR/gemma/google_gemma-4-26B-A4B-it-IQ3_M.gguf` |
| `fable` | `Achilles1089/fable-coder-35B-A3B` | `Q2_K` | `$GGUF_DIR/fable/fable-coder-35B-A3B.Q2_K.gguf` |
| `qwopus` | `Jackrong/Qwopus3.6-35B-A3B-Coder` | `Q2_K` | `$GGUF_DIR/qwopus/Qwopus3.6-35B-A3B-Coder-vram13-q2ex-imat.gguf` |
| `agenta1` | `InternScience/Agents-A1` | `IQ2_M` | `$GGUF_DIR/agenta1/Agents-A1-IQ2_M.gguf` |
| `muse` | `meta-models/Muse-Glimmer-30B` | `Q3_K_XL` | `$GGUF_DIR/muse/Muse-Glimmer-30B-UD-Q3_K_XL.gguf` |
| `devstral` | `mistralai/Devstral-Small-2-24B-Instruct-2512` | `Q3_K_M` | `$GGUF_DIR/devstral/Devstral-Small-2-24B-Instruct-2512-Q3_K_M.gguf` |
| `agenta-tiny` | `InternScience/Agents-A1-4B` | `Q8_0` | `$GGUF_DIR/agenta-tiny/Agents-A1-4B-Q8_0.gguf` |
| `gptoss` | `openai/gpt-oss-20b` | `MXFP4` | `$GGUF_DIR/gptoss/openai_gpt-oss-20b-MXFP4.gguf` |

Los repos son de los pesos **sin cuantizar** (el `base_model` de cada GGUF
publicado). Con `--from-subdir original` se cuantiza de la copia en BF16
que traen algunos repos, que es lo correcto cuando los pesos de arriba ya
vienen cuantizados.

Y el imatrix va **antes** de los posicionales, porque `llama-quantize` solo
parsea las opciones mientras el argumento empiece por `--`: si la flag va
detrás, se come el parseo del número de hilos y el cuantizador muere con
`invalid nthread '--imatrix'`. El gguffer ya lo hace bien por su cuenta; si
llamas al cuantizador a mano, el orden es:

```bash
llama-quantize --imatrix mi.imatrix entrada.gguf salida.gguf IQ2_M
```

Lo que no se ve en el comando es lo que pasa **cuando algo falla**, que es
lo que importa cuando estás descargando 60 GB:

| Si falla | Qué pasa |
|---|---|
| la descarga | se borra el clon a medias **y** la caché de HF |
| la conversión | se conservan los safetensors y se dice cómo reintentar solo la conversión |
| la cuantización | se conservan los safetensors **y** el GGUF intermedio, y se dice cómo reintentar solo la cuantización |
| la verificación del GGUF | no se borra nada |
| nada (todo bien) | se borran los safetensors, porque el GGUF ya está validado |

`--force` borra lo cacheado de ese repo y lo rehace desde cero. El clon de
los pesos vive en `~/.cache/ropero/gguffer/<owner>__<repo>` y se reutiliza
si ya está: reintentar la cuantización no vuelve a bajar el modelo.

Un detalle que marea: hay repos cuyos pesos **ya vienen cuantizados** (el
gpt-oss viene en MXFP4). Cuantizar encima de eso es dequantizar y
re-cuantizar: se pierde calidad sin ganar nada. El gguffer lo mira en el
`file_type` del GGUF convertido y te avisa; para cuantizar de los pesos sin
cuantizar está `original/` en el repo (`--from-subdir original`).

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

> La lista **vigente** está en `ropero list`: sale de los TOMLs, con el
> ctx, el offload (`ngl` / `cpu-moe`) y el tipo de KV de cada modelo.
>
> ```console
> $ ropero list
>   qwen         slot :9999 / :9998    ctx=163840     ngl=66                     turbo3   engine=llama-server   gguf=Qwen3.8-27B-IQ2_M.gguf
>   nemotron     slot :9999 / :9998    ctx=1048576    ngl=52 cpu-moe=18          q4_0    engine=llama-server   gguf=NVIDIA-Nemotron-3.5-Lightning-30B-A3B-UD-IQ3_XXS.gguf
> ```
>
> No hay una tabla de VRAM aquí a propósito: el margen libre depende del tipo
> de KV (`turbo3` vs `q4_0` ocupan distinto) y de la cuantización, así que
> cualquier número escrito en el README se desincroniza en cuanto cambia un
> TOML. Para calcularlo, el modelo se arranca y se mira su log.

**Cada modelo lleva su ctx nativo** (el `context_length` de su propio GGUF) y
el offload justo para que quepa en los 15.300 MiB libres de la RTX 5080.

Las dos vías de offload, y por qué se elige una u otra:

| vía | qué manda a la CPU | qué NO toca | coste en decode |
|---|---|---|---|
| `--n-cpu-moe N` | los **experts** de las N primeras capas | la atención ni el KV | cada token cruza el PCIe |
| `--n-gpu-layers N` | **capas enteras** (y su KV con ellas) | — | cada token cruza el PCIe N veces |

`--n-cpu-moe` es la vía **barata** (solo MoE): la atención y el KV cache
siguen enteros en la GPU, así que el modelo conserva su velocidad de
atención. `--n-gpu-layers` parcial es la vía **cara** y solo se usa cuando el
modelo es denso (sin experts que mandar a la CPU): cada token cruza el bus
PCIe una vez por cada capa en CPU.

Los que pagan offload:

- **`nemotron`**: sus pesos (19.76 GB) no caben de ninguna manera en 16 GB,
  ni con la cuantización más pequeña que existe. Va con `--n-cpu-moe 18`:
  el 88 % de sus pesos son los expertos routed, y con 18 de sus 23 capas de
  experts en la CPU el ctx nativo de **1.000.000** de tokens cabe con 291 MiB
  de margen. Su KV es diminuto (solo 6 de sus 52 capas son de atención; las
  otras 46 son Mamba-2, con estado recurrente).
- **`qwencoder`**: 48 capas de atención completa, su KV son 6.75 GB a 262144.
  Con `--n-cpu-moe 22` entra. Es el que peor sale en decode del roster.
- **`devstral`** y **`qwen`**: son densos, así que no hay experts que mandar
  a la CPU: el offload es por capas (`--n-gpu-layers` en su TOML).

Si el ctx largo no es lo prioritario en un modelo concreto, bajar su ctx
(`ROPERO_<MODELO>_CTX`) aligera el offload y el decode recupera velocidad.

### El KV `turbo3` y el `--context-shift`

Los TOMLs usan `turbo3` (3 bits con rotación Hadamard, del fork
`atomic-llama-cpp-turboquant`): el KV entra casi 4 veces más apretado que
en f16, que es lo que permite el ctx largo.

Pero hay una contrapartida que **no es de ropero, es del fork**: con
`turbo3`/`turbo2`/`turbo4` en la **K**, el `--context-shift` se **salta**
(sin crash, pero la calidad de la salida baja después del shift). Los tipos
`q8_0`/`q4_0` sí desplazan bien.

O sea: si un modelo lleva `--context-shift` **y** `turbo3` en la K, hay que
elegir:

- bajar el ctx para que **no** llegue a fullness (entonces no hay shift que
  saltarse), o
- poner `q8_0` en la K y `turbo3` en la V (la V no participa del shift).

El `ropero list` muestra el tipo de KV de cada modelo: si pone `turbo3` y el
modelo trae `--context-shift`, es un caso de estos.

Los demás engines (`airllm`, `mlx_lm`) tienen sus modelos aparte, en
`models/airllm/` y `models/mlx_lm/`.

La lista completa y vigente está en `models/` — que es la fuente de verdad: un TOML
nuevo aparece en `ropero` sin tocar esta tabla.

Los alias son formas de llamar al mismo modelo con otro nivel de razonamiento:
`ropero verifier` es `ropero gptoss --high`.

### Overrides por entorno

| Variable | Qué hace |
|---|---|
| `ROPERO_<MODELO>_CTX` | el contexto del modelo |
| `ROPERO_<MODELO>_GGUF` | otra cuantización del mismo repo |
| `ROPERO_<MODELO>_PORT` | el puerto del modelo |
| `ROPERO_TEMP` | el `--temp` del muestreo, **sustituye** al del TOML |
| `LLAMA_API_KEY` | la clave de la API (por defecto `sk-local-dev-key`) |
| `HOST` | dónde escuchar (por defecto `0.0.0.0`) |
| `GGUF_DIR` | dónde están los GGUF (por defecto `~/models/gguf`) |
| `SAFETENSORS_DIR` | dónde están los checkpoints (por defecto `~/models/safetensors`) |

`ROPERO_TEMP` es global a propósito: el bench tiene que medir a todos los
modelos con el **mismo** temp, y cada TOML trae el suyo. Sustituye, no
añade (en el argv gana el último `--temp`, que es el del override).

Los overrides propios de cada modelo (temperatura del drafter, tipo de KV…)
también existen: `ROPERO_GEMMA_TEMP`, `ROPERO_GEMMA_MTP_NMAX`,
`ROPERO_GPTOSS_KV`… y están declarados en el TOML de cada modelo.

### `--n-gpu-layers`: el número real de capas

En los TOMLs de `models/llama/`, `--n-gpu-layers` lleva **el número real de
capas del GGUF** (leído de `<arch>.block_count` en la cabecera del fichero, no
de un README ni de una suposición). La razón está en el código de `llama.cpp`:

`common/fit.cpp` hace dos cosas, **en este orden**:

1. Si el modelo no cabe, **reduce el contexto** (no las capas):
   `"entire model can be fit by reducing context"`.
2. Y si el usuario fijó `n_gpu_layers != -1`, **aborta**:
   `"n_gpu_layers already set by user to N, abort"`.

O sea: un `-ngl` fijo desactiva el fitter y el modelo se pide entero en la
VRAM de golpe. Si no cabe, la carga falla:

```
allocating 12036.68 MiB on device 0: cudaMalloc failed: out of memory
llama_server: exiting due to model loading error
```

Medido el 2026-10-25 con `codoss` (`-ngl 25` fijo): no llegó a responder ni la
primera tarea.

Por eso la política es:

- **La cuantización se elige para que el modelo entero quepa**, y entonces el
  `-ngl` con el número exacto de capas es correcto: no hay nada que recortar.
- Si el modelo **no** cabe entero, hay dos salidas, y se elige según el
  modelo:
  - **MoE** (`--n-cpu-moe N`): se mandan los experts de las N primeras capas a
    la CPU. La atención y el KV siguen en la GPU. Es la vía barata.
  - **Denso** (`--n-gpu-layers N`): se mandan N capas enteras a la CPU, con su
    KV. Es la vía cara: cada token cruza el bus PCIe N veces.
  - O el `-ngl` a `-1`, para que el fitter recorte el contexto en vez de las
    capas: sale un modelo rápido con menos contexto.
- Un `-ngl` fijo con un número **menor** que el total de capas es un offload
  parcial deliberado: cada token de las capas en CPU cruza el bus PCIe. Es
  lento, y el bench lo detecta (el log dice `offloaded X/Y layers`).

Los ctx que se necesitan con el `--n-gpu-layers` fijo salen de la aritmética
de VRAM de cada TOML, que está escrita en su cabecera y es reproducible con
las reglas de la sección siguiente.

### El MTP y la VRAM

`--spec-type draft-mtp` crea un **draft context** con su propio espacio de VRAM
y su KV en `f16`, y el fitter lo mide **junto al modelo principal**
(`common.cpp`: el MTP se pasa como `extra` a `common_fit_params`). Con el
margen justo, el MTP se come la VRAM que necesitan las capas del modelo, y el
server no arranca.

En un portátil de 16 GB el MTP no compensa: el draft context son ~0.5 GB de
pesos + KV en `f16` (el doble que en `q8_0`), y lo que se gana en t/s no llega
a lo que se pierde en capacidad de contexto y en el riesgo de que el modelo no
arranque.

**Política: el roster activo va sin MTP.** Se quitó de todos los TOMLs. Cuando
el GGUF trae la cabeza MTP incrustada (`<arch>.nextn_predict_layers = 1`), los
pesos están en el fichero pero **no se cargan** si no se pide el draft context
(`llama-model.cpp`: la capa nextn sólo se crea con
`LLAMA_CONTEXT_TYPE_MTP`).

Si algún día hace falta el MTP, se documenta en la cabecera del TOML y se
comprueba con `ropero status` que el server arranca y que el log dice
`offloaded N/N layers`.

### Cómo se calcula el KV cache (y por qué no es "capas × ctx")

El KV cache **no lo gastan todas las capas**, y esto cambia el resultado por
un factor de 4 en los modelos híbridos. La fórmula de llama.cpp
(`src/llama-kv-cache.cpp`) es:

```
bytes_por_capa = n_embd_k * sizeof(type_k) + n_embd_v * sizeof(type_v)
KV = celdas * capas * bytes_por_capa
```

pero hay que saber **cuántas capas** de cada tipo hay, y eso NO sale de una
sola regla:

| Tipo de modelo | Cuántas capas gastan KV | Dónde sale |
|---|---|---|
| Denso normal | todas | — |
| Sliding window | 1 de cada N plenas; el resto, con ventana | `<arch>.sliding_window_pattern` (número o lista) |
| Híbrido recurrente | 1 de cada 4 plenas | `<arch>.full_attention_interval = 4` |
| Mamba (nemotron) | las de atención, y con `head_count_kv` por capa | lista por capa, 0 = capa mamba |

En `gemma4` además las capas sliding window tienen **otro `head_dim`** (256
en vez de 512), así que el KV de ese grupo sale por la mitad.

Los números del KV se pueden leer directamente del log del server, que es lo
que hay que hacer para no adivinar:

```
llama_kv_cache: size = 1152.00 MiB ( 65536 cells, 16 layers, 1/1 seqs),
                K (q4_0): 576.00 MiB, V (q4_0): 576.00 MiB
```

**Calibrado contra los memory breakdown medidos el 2026-10-25** (RTX 5080
Laptop, 15933 MiB), el error de la fórmula es del **0,00 %** en los 5 casos:

| modelo | calculado | medido |
|---|---|---|
| gptoss (12 plenas + 12 SWA-128) | 1632.00 MiB | 1632.00 MiB |
| gptoss (grupo SWA, 512 celdas) | 6.38 MiB | 6.38 MiB |
| qwen3.8-27B (16 de 64 de atención) | 1152.00 MiB | 1152.00 MiB |
| agenta1 (10 de 40 de atención) | 720.00 MiB | 720.00 MiB |
| agents-tiny (8 de 32 de atención) | 4352.00 MiB | 4352.00 MiB |

El MTP tiene su propio KV (en f16) y su propio compute buffer, aparte: medido
256 MiB + 66 MiB para el draft context del qwen.

### Cuánta VRAM hace falta

```
pesos + KV(ctx) + compute buffer + server + holgura <= VRAM libre
```

El compute buffer escala con `--ubatch-size` (medido: 338 MiB con ubatch 256,
1182 MiB con ubatch 1024), no con el batch total. El "server + holgura" son
~475 MiB (contexto CUDA, allocator, fragmentación).

### El modelo excepcional: `qwen36`

Para el caso de "un solo modelo que sirva de coder y de analista, con el
razonamiento ajustable", el candidato es **`qwen36`** (Qwen3.6-35B-A3B):

- **73.4 % SWE-bench Verified**, 49.5 % SWE-bench Pro, 51.5 % Terminal-Bench 2.0
- MoE: 35B totales pero **3B activados** por token → el decode va rápido
- **262144 de contexto nativo**, y el modelo avisa de que por debajo de
  128K se pierde capacidad de thinking
- Niveles de razonamiento **low / medium / xhigh** (el template no conoce
  "high": ese valor da error)

Los runners-up, para cada perfil:

| Modelo | Para qué | Lo bueno | Lo malo |
|---|---|---|---|
| `qwen36` | coder + analista, el más fuerte que entra | 73.4 % SWE-bench V, 3B activos, 262K | MoE: la calidad del MoE es menor que la de un denso del mismo tamaño |
| `muse` | el más listo y con razonamiento fino | SWE-bench Pro **51.2 %** (el más alto de la tabla), IFBench 77.0, AIME 94.7, y **4 niveles** (low/medium/high/xhigh) vía system prompt | Es **denso** 30B: sin MTP va a 15-25 t/s |
| `laguna` | agente con herramientas | thinking **por petición**, APEX (cuantización por capa, no plana) | 37.5 % TB 2.0; es denso en atención (su KV es 8× la de un MoE), así que paga `--n-cpu-moe 4` para el ctx nativo |
| `qwen` | el denso de referencia | Qwen3.8-27B, IQ2_M | 61.7 % SWE-bench Pro |
| `fable` | coder con el mejor tool-use | Opus-4.8 tool use, 51.5 % TB | Su GGUF trae la cabeza MTP y hay que quitarla |

**Muse Glimmer** (Meta, Apache 2.0) es probablemente el mejor "analista" del
roster: gana en SWE-Bench Pro (51.2 vs 50.2), en IFBench (77.0 vs 70.8), en
AIME 2026 (94.7) y en GPQA Diamond, y su razonamiento se ajusta en cuatro
niveles con `Reasoning strength: <valor>` en el system prompt. El precio es
que es denso: sin MTP va lento.

Con offload se puede subir de nivel, pero el offload de un denso sale caro:
cada token de las capas en CPU cruza el bus PCIe. Con un MoE (que es lo que
hace rápido al `qwen36`) el offload penaliza mucho menos, porque solo se
van los expertos.

Los ctx nativos que lleva cada modelo están en la tabla de "Los modelos". El
cuadro completo: cada TOML tiene en su cabecera la aritmética de VRAM con la
que se calculó su offload.

### `--n-cpu-moe`: la vía de offload de los MoE

Mandar expertos MoE a la CPU (`--n-cpu-moe N`) es la otra forma de que el
modelo "quepa". No es gratis: cada token recorre los expertos por el bus
PCIe y el decode baja. Pero es **mucho más barato** que mandar capas
enteras, porque:

- solo se van los **experts** (el 44 % de los pesos en un MoE routed típico);
  la atención, el lm head y los embeddings se quedan en la GPU;
- y el **KV cache no se toca**: sigue entero en la VRAM, así que la atención
  no va a la RAM.

En llama.cpp el flag manda a la CPU los tensores `ffn_*_exps` de los bloques
`0..N-1` (`common/common.h`, `llm_add_n_cpu_ffn_overrides`): es un override
del **buffer type del tensor**, no del device de la capa. Por eso el KV (que
se asigna en el backend de la capa) se queda en la GPU.

En el roster activo lo usan cuatro modelos: `nemotron` (18), `qwencoder` (22),
`laguna` (4) y `fable` (2). Los demás entran enteros.

### El `--n-cpu-moe` y el ctx nativo

El contexto que el servidor usa es `min(n_ctx_seq, n_ctx_train)`
(`server-context.cpp`): pedir más que el ctx train **no** da más contexto.
El `ctx_train` de cada modelo sale de su propio GGUF, y ese es el ctx que
llevan los TOMLs.

El nemotron es el caso extremo: su ctx train es de **1.000.000** de tokens
(su rope no lleva scaling), y entra con 291 MiB de margen porque su KV es
ridículamente pequeño (solo 6 de sus 52 capas son de atención; las otras 46
son Mamba-2, con estado recurrente).


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
