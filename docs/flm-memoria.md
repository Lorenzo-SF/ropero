# FastFlowLM en la NPU: la memoria y el tope de ~15,6 GB

Documento de trabajo: **qué** usa cada modelo, **cuánta** RAM y **si se
puede usar más** que el tope que anuncia FLM.

Fuentes (todas comprobadas, nada de memoria):

- `src/model_list.json` de ROCm/FastFlowLM → el `footprint` de cada modelo
  (la RAM que ocupa) y su contexto por defecto.
- `docs/docs/instructions/server/prefix_cache.md` de FastFlowLM → el coste
  **por token guardado** de cada modelo, medido del log del servidor. Ese
  coste por token ES el tamaño de la KV cache: es la copia del estado del
  modelo que se guarda por token.
- `docs/docs/models/*.md` de FastFlowLM → el contexto máximo y mínimo de
  cada modelo.
- La búsqueda en el código de FastFlowLM: la cadena "15.6" **no aparece en
  ningún `.cpp`**, sólo en la documentación. El tope no es de FLM.

## El tope: de dónde sale y si se puede subir

FLM avisa en la ficha de gpt-oss:

> The model itself uses ~15.1 GB of memory in FLM, and there is an internal
> cap (~15.6 GB) on NPU memory allocation enforced by AMD/Microsoft, which
> makes only about half of the total system RAM available to the NPU.

Ese tope **no está en el código de FLM** (verificado: la cadena sólo sale en
los `.md`). Está por debajo, en la plataforma:

1. **La NPU no tiene RAM propia**: usa un *carveout* de la RAM del sistema
   (la región TOU del firmware), y ese reparto lo decide el **BIOS**.
2. **El iGPU se lleva su parte con "Auto"**: la UMA frame buffer, en modo
   automático, reserva RAM dinámicamente y se la come.
3. **`memlock`**: el runtime de XRT necesita poder fijar memoria; con el
   límite por defecto (8 MB) las reservas del NPU fallan en silencio. Ropero
   ya pone `memlock unlimited` al instalar el engine
   (`/etc/security/limits.d/99-fastflowlm.conf`).

### La palanca: BIOS → Advanced → AMD CBS → UMA Frame Buffer Size

Ponerla a **2 GB** en vez de "Auto" devuelve esa RAM al sistema y la NPU
llega a ver mucho más. En una guía medida sobre un **HX 370 con CachyOS**
(misma CPU que aquí) el resultado es:

> ~21,1 GB de RAM disponible para los modelos del NPU

Es decir: **sí se puede usar más**. Con el reparto por defecto te quedas
cerca de los ~15,6 GB; con la UMA a 2 GB, la NPU ve ~21 GB.

Además, cuanto más RAM sistema haya, más grande puede ser la region que el
firmware reparte: el tope de 15,6 GB es una proporción de la RAM total, no
un número fijo. En una máquina con 64 GB el margen es mucho mayor.

Cómo comprobarlo desde Linux:

```bash
flm validate                 # kernel, NPU, FW y el límite de memlock
xrt-smi examine              # el estado real del dispositivo
watch -n 0.1 xrt-smi examine --report aie-partitions   # mientras corre
```

## La tabla: pesos, contexto y total

`KiB/token` es el coste de la KV cache por token (medido por FLM). El total
es `pesos + (KiB/token × tokens) + fijos`.

| modelo | pesos | KiB/token | total a 8k | total a 16k | total a 32k | ctx máx (FLM) |
|---|---|---|---|---|---|---|
| `gpt-oss:20b` | ~13,8 GB | 24 | ~14,0 GB | ~14,2 GB | ~14,6 GB | 128k |
| `nemotron3.5-bolt:e4b` | ~4,9 GB | 16 | ~5,0 GB | ~5,2 GB | ~5,4 GB | 64k |
| `deepseek-r1:8b` | ~3,4 GB | 128 | ~4,4 GB | ~5,4 GB | ~7,4 GB | 128k |
| `deepseek-r1-0528:8b` | ~3,3 GB | 144 | ~4,5 GB | ~5,6 GB | ~7,8 GB | 64k |
| `qwen3.8-mtp:27b` | ~17,6 GB | 84 | ~18,4 GB | — | — | 8k |
| `qwen3.8-dflash2:27b` | ~19,7 GB | 84 | ~20,5 GB | — | — | 8k |

Y el coste del contexto **solo**, por bloques de 8k (sin los pesos), que es
lo que se multiplica para contextos mayores:

| modelo | por 8k de contexto | fijos por prompt guardado |
|---|---|---|
| `gpt-oss:20b` | 192 MiB | 3 MiB |
| `nemotron3.5-bolt:e4b` | 128 MiB | 20 MiB |
| `deepseek-r1:8b` | 1024 MiB | — |
| `deepseek-r1-0528:8b` | 1152 MiB | — |
| `qwen3.8-mtp:27b` | 672 MiB | 151 MiB |
| `qwen3.8-dflash2:27b` | 672 MiB | 151 MiB |

Los "fijos" (3/20/151 MiB) son el estado recurrente del modelo (Qwen3.5,
Qwen3.6, Qwen3.8, Gemma 3/4 y Nemotron-Bolt son híbridos con estado
recurrente; los Llama y los Qwen3 "puros" no lo tienen) y se cobran una vez
por prompt guardado en la caché de prefijos, no por token.

### Con el reparto por defecto (~15,6 GB para la NPU)

| modelo | pesos | ¿cabe? |
|---|---|---|
| `nemotron3.5-bolt:e4b` | 4,9 GB | sí, de sobra |
| `deepseek-r1-0528:8b` | 3,3 GB | sí, de sobra |
| `deepseek-r1:8b` | 3,4 GB | sí, de sobra |
| `gpt-oss:20b` | 13,8 GB | sí, pero JUSTO (no hay margen para el contexto) |
| `qwen3.8-mtp:27b` | 17,6 GB | **no cabe** |
| `qwen3.8-dflash2:27b` | 19,7 GB | **no cabe** |

### Con la UMA frame buffer a 2 GB (~21 GB para la NPU)

Los seis caben. `qwen3.8-dflash2:27b` sigue siendo el más apretado (20,5 GB
a 8k de contexto sobre un total de ~21 GB).

## Los que no son de la lista, por si acaso

Del catálogo completo de FLM (71 tags), los que mejor relación
contexto/RAM tienen:

| modelo | KiB/token | total a su ctx por defecto |
|---|---|---|
| `gemma3:1b` | 4 | ~1,2 GB |
| `qwen3.5:0.8b` / `:2b` | 12 | ~0,9 / 2,2 GB |
| `qwen3:0.6b` | 112 | ~0,7 GB |
| `qwen3.5:4b` | 32 | ~3,9 GB |
| `qwen3.5:9b` | 32 | ~8,7 GB |
| `llama3.2:1b` | 32 | ~1,3 GB |
| `llama3.2:3b` | 112 | ~2,7 GB |
| `llama3.1:8b` | 128 | ~5,4 GB |
| `phi4-mini-it:4b` | 128 | ~3,4 GB |
| `gemma4-it:e4b` | 16 | ~5,4 GB |
| `qwen3.6-moe:35b-a3b` | 20 | ~22,1 GB (no cabe sin el BIOS) |

## Avisos de la propia doc de FLM

- **Qwen3.6 con más de 32768 tokens devuelve basura** (paged attention: un
  passkey pedido tras ~48k tokens salió como "!" repetido).
- **Qwen3.8 con prompts largos devuelve basura** (un passkey salió bien a 6k
  y 14k tokens y se corrompió a 10k y 28k). Su máximo son 8k.
- **Qwen3.5 y Qwen3.8 derivan** del modelo de referencia cuanto más páginas
  leen (PAGED_DECODE_PAGE_DRIFT).
- **gpt-oss:20b en una máquina de 32 GB "a veces funciona y a veces no"**.
