# El acrit: clonar y fabricar los GGUFs

Cómo se fabrican las cuantizaciones propias, paso a paso. Lo que hay aquí no
es theory: es el procedimiento que funciona, con los comandos exactos.

## Lo primero: qué se fabrica y por qué

### La vía corta: la sección `[build]` del TOML

Desde el 2026-10-28, un modelo del roster puede **declarar cómo se fabrica**
en su propio TOML, y `ropero install models <modelo>` lo hace por ti:

```toml
[build]
repo         = "nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16"
quant        = "MXFP4_MOE"
imatrix_repo = "bartowski/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF"
imatrix_file = "NVIDIA-Nemotron-3.5-Lightning-30B-A3B-imatrix.gguf"
keep_source  = true
size         = "65.8 GB"
```

Las claves son las mismas que acepta `ropero gguffer`, y el que ejecuta es el
gguffer de verdad: **la receta del TOML y el comando a mano son lo mismo**.
El GGUF sale con el nombre de `[files] gguf`, en la carpeta del modelo, que es
donde el lanzador va a buscarlo.

Los siete modelos que se fabrican así (y sus comandos equivalentes):

| Modelo | Pesos | Quant | Imatrix |
|---|---|---|---|
| `qwen` | `Qwen/Qwen3.8-27B` | `NVFP4` | `bartowski/Qwen3.8-27B-GGUF` |
| `qwen3` | `Qwen/Qwen3-30B-A3B` | `NVFP4` | `bartowski/Qwen_Qwen3-30B-A3B-GGUF` |
| `qwencoder` | `Qwen/Qwen3-Coder-30B-A3B-Instruct` | `IQ4_NL` | `n19875624/...-code-imatrix-GGUF` |
| `granite` | `ibm-granite/granite-4.0-h-small` | `IQ4_NL` | `unsloth/granite-4.0-h-small-GGUF` |
| `nemotron` | `nvidia/...-BF16` | `NVFP4` | `bartowski/...-GGUF` |
| `ornith` | `ornith-ai/Ornith-1.5-35B-A3B` | `NVFP4` | `bartowski/Ornith-1.5-35B-A3B-GGUF` |
| `muse` | `meta-models/Muse-Glimmer-30B` | `IQ4_NL` | `AtomicChat/Muse-Glimmer-30B-GGUF` |

El `size` es el de los **pesos** que se bajan, no el del GGUF final (que
todavía no existe): es lo que ve el usuario cuando se le pregunta si fabricar
esto.

Y si el GGUF **ya está en su sitio**, el gguffer no hace nada (ni baja los
pesos ni convierte ni cuantiza). El check va antes de la descarga, que es
la parte cara: 60 GB y una hora. Con `--force` se rehace.

### Los quants que ya están publicados

Antes de fabricar nada, mira si el quant ya existe. El gguffer lo dice:

```bash
ropero gguffer --repo Qwen/Qwen3-30B-A3B --list-quants
```

Lista los quants que el motor soporta **y** los que ya están publicados para
ese modelo base (con el repo donde están). Fabricar un quant que ya existe es
una hora de trabajo para nada.

### El preflight: lo que se sabe antes de bajar 60 GB

```bash
ropero gguffer --repo nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-NVFP4 --quant NVFP4
```

Si el repo **ya viene cuantizado** (los de NVIDIA con `-NVFP4`/`-FP8`, los de
modelopt), el gguffer lo dice y propone el repo hermano sin cuantizar, sin
bajar nada. Sin ese preflight, el fallo salía al final del proceso, cuando ya
se habían bajado 60 GB y convertido, y el mensaje del motor
(`requantizing from type nvfp4 is disabled`) no decía de dónde venía el
problema.


La tabla de qué fabricar es **`modelos.tsv`** (este directorio). Es la fuente
de verdad: se edita la tabla, no el código. Cada línea es **una tirada**:

```
modelo  repo de los pesos  quants (comas)  repo del imatrix  fichero del imatrix  flags  fuente
```

La **última columna, "fuente"**, es opcional y decide de dónde sale el
GGUF:

| fuente | qué hace el gguffer |
|---|---|
| vacía | clona los pesos del repo de la 2ª columna y los convierte |
| `owner/repo` | baja el .gguf sin cuantizar de ese repo y lo cuantiza |
| `owner/repo:fichero` | baja ese fichero concreto |
| `/ruta/local.gguf` | cuantiza ese fichero, tal cual |

**Por qué existe la columna "fuente":** hay modelos cuyo conversor no sabe
convertirlos. El caso real es el **Qwen3.8-27B** (ver más abajo).

Descartados el 2026-10-27: **fable** y **nemotron** (ya caben, y con el
nemotron a 512K de ctx con 53 capas va muy bien: cuantizarlo más es tirar
rendimiento sin ganar nada).

## El为什么 de los quants múltiples

En una línea hay varios quants separados por comas. Bajar y convertir los
pesos es lo caro (40-60 GB y una hora); **cuantizar es solo CPU**. Por eso el
gguffer convierte el GGUF intermedio **una vez** y saca de ahí todos los
quants: dos quants cuestan lo mismo que uno, en disco y en tiempo.

El **primer** quant de la lista es el *principal*: va a la ruta que espera el
TOML del modelo. Los demás quedan en `$GGUF_DIR/_variantes/<modelo>/`, con su
nombre `<repo>-<quant>.gguf`.

> **Ojo con el nombre del TOML.** Si el TOML pide `Qwen3.8-27B-IQ2_M` y la
> tirada fabrica `IQ3_XS`, el IQ3_XS **no** puede ir ahi con ese nombre: sería
> un nombre mentiroso, y el launcher creería que el modelo es el IQ2_M. En ese
> caso el GGUF va a la carpeta del modelo con su nombre propio. El script lo
> comprueba solo.

## El procedimiento

### 0. Preparar

```bash
cd ~/ropero
git pull
echo "HF_USER=tu-usuario" > quant/hf.env
```

Y comprobar que hay disco. **Una tirada necesita ~120 GB**: el clon de los
pesos (40-60 GB) + el GGUF intermedio (del mismo tamaño) + el quant.

```bash
df -h ~/models ~/.cache/ropero
```

Si el disco va justo, `GGUF_WORK_DIR` y `GGUF_DIR` se pueden poner en otro
disco:

```bash
export GGUF_WORK_DIR=/mnt/disco-lento/gguffer-work
export GGUF_DIR=/mnt/disco-grande/models/gguf
```

### 1. Mirar qué hay y cuánto pesa

```bash
cd quant
./cuantizar-todos.sh
```

Imprime la tabla con el destino de cada modelo. Si algo no cuadra, se corrige
la tabla y se vuelve a mirar.

### 2. EL DIAGNÓSTICO (30 segundos, y ahorra horas)

**Antes de tirar 60 GB, mira por qué no se cuantiza.** El diagnóstico no
cuantiza: dice el espacio que hay, cuánto hace falta, cuál es el bloque de la
MTP, si el imatrix la cubre, y el argv exacto que iría al cuantizador.

```bash
./cuantizar-todos.sh qwen --diagnostico
```

La salida tiene este formato:

```
── DIAGNOSTICO ──────────────────────────────────────────────
  pesos clonados:   /home/usuario/.cache/ropero/gguffer/Qwen__Qwen3.8-27B
  tamano:           54.2GiB
  espacio libre:    210.5GiB
  espacio a porner: 92.8GiB  (GGUF intermedio + margen)
  ...
  GGUF intermedio:  /home/usuario/models/gguf/qwen/Qwen3.8-27B-F16.gguf
    ya existe (se reutiliza)
    bloque MTP:      64
    file_type:       1
  imatrix:          /home/usuario/.cache/ropero/gguffer/imatrix/bartowski__Qwen3.8-27B-GGUF/Qwen3.8-27B-imatrix.gguf
    fichero: existe (1.2GiB)
    repo:     bartowski/Qwen3.8-27B-GGUF
    cubre el bloque MTP (64): no
    => se anadira --tensor-type "blk\.64\..*=Q4_0"
  quant pedido:     IQ3_XS
  argv del cuantizador (lo que se ejecutaria):
    llama-quantize --imatrix <f> --tensor-type "blk\.64\..*=Q4_0" <f16> <salida> IQ3_XS
```

**Lo que hay que mirar:**

| si dice | es que | que hacer |
|---|---|---|
| `espacio libre` < `espacio a porner` | no cabe | libera disco, o `GGUF_WORK_DIR` a otro sitio |
| `bloque MTP: 64` y `cubre: no` | normal | el gguffer pone el bloque 64 en Q4_0 solo |
| `bloque MTP: ninguno` | el GGUF no lleva MTP | normal |
| `imatrix: NINGUNO` | no hay matriz | el quant sale, pero peor: sin matriz los tensores no importantes se degradan igual |
| `cuantizador: ...` sin `--tensor-type` y hay MTP | **el fix de la MTP no se aplicó** | hay que mirar por qué (imatrix que no cubre, o bloque mal detectado) |

### 3. Fabricar

```bash
./cuantizar-todos.sh qwen
```

O todos, de golpe:

```bash
./cuantizar-todos.sh todos
```

**Un fallo NO para el trabajo**: el script sigue con el siguiente modelo. Eso
es a propósito: un modelo que no se deja fabricar no puede impedir que los
demás sí. Para que pare en el primer fallo:

```bash
./cuantizar-todos.sh todos --parar
```

Cada tirada deja su log en `quant/logs/<modelo>.log`, y al final hay un
resumen con lo que se hizo y lo que falló.

### 4. Si falla

El gguffer **borra los safetensors** cuando algo falla (60 GB por intento
llenan el disco, y cuando el disco se llena falla también lo que ya estaba
bien). Para conservarlos y reintentar sin volver a bajar 60 GB:

```bash
KEEP=1 ./cuantizar-todos.sh qwen
```

Con `KEEP=1` el script pasa `--keep-source` al gguffer: los safetensors se
conservan y el reintento no tiene que volver a bajar 60 GB. Sin `KEEP=1` (lo
normal), se borran.

El mensaje del error dice siempre qué pasó: sin disco, sin memoria, ficheros
que faltan, o el tensor que el imatrix no cubre.

### 5. Verificar

```bash
./verificar-gguf.sh "$GGUF" IQ3_XS
```

Un GGUF que no pasa la verificación **no se sube**. Es mejor no subir que
subir un GGUF roto.

### 6. Subir a HuggingFace

```bash
./subir-hf.sh --listar     # lo que hay y donde acabaria
./subir-hf.sh --subir      # sube lo verificado
```

El marcador `.hf-uploaded` va junto al GGUF: si existe, no se vuelve a subir.

Cada repo lleva dos ficheros:

- **`README.md`**: la nota de que **esto es una cuantización de terceros y el
  modelo no es mío**, con el autor original y su licencia.
- **`CUANTIZATION.md`**: las condiciones (de qué repo salió, con qué quant, con
  qué importance matrix, qué bloque quedó fuera de la matriz y por qué).

## El Qwen3.8-27B: por qué no se fabricaba (medido el 2026-10-27)

Este es el caso quecostó una noche entera, así que va entero.

**Síntoma:** el repo está clonado, pero no sale ningún GGUF. Ni con
`--no-mtp`.

**Causa raíz (medida, no supuesta):**

1. El Qwen3.8-27B es **multimodal**: su `config.json` dice
   `architectures: ["Qwen3_5ForConditionalGeneration"]`, y el `text_config`
   va **anidado** (el conversor lo aplana en `ModelBase.__init__`).
2. Lleva **cabeza MTP**: `mtp_num_hidden_layers: 1`, y sus tensores se llaman
   `mtp.*` (15 en total) en `model.safetensors.index.json`.
3. El conversor debe remapearlos al bloque 64 (`model.layers.64.*`), que es
   lo que hace `_QwenMtpMixin.filter_tensors` en `conversion/qwen.py`.
4. **La clase que se usa no lo hace.** En el fork que usa ropero
   (`AtomicBot-ai/atomic-llama-cpp-turboquant`, que es el que clona
   `ropero`), la clase `Qwen3_5TextModel` (en `conversion/qwen.py`) **no usa
   `_QwenMtpMixin`**: el mixin existe, pero la clase no lo lleva. Los 15
   tensores `mtp.*` no se pueden mapear a ningún tensor del mapa (que tiene
   64 bloques) y la conversión aborta con:

   ```
   ValueError: Can not map tensor 'mtp.layers.0.attn_k.weight'
   ```

5. Y **`--no-mtp` no lo arregla**, porque esa clase ignora el flag: el
   bloque 64 se cuela igual. De ahí el "ni con --no-mtp".

**Los otros dos datos, medidos:**

- El **imatrix público** de bartowski (`Qwen3.8-27B-imatrix.gguf`, 13 MB)
  cubre **exactamente los bloques 0..63** (992 tensores). El bloque 64 no
  aparece **en absoluto**. Por eso, si el GGUF lleva la MTP, el cuantizador
  aborta con `Missing importance matrix for tensor blk.64.attn_k.weight`.
- El GGUF que publica bartowski **sí** lleva la MTP, pero como los tensores
  `blk.64.nextn.*` (4, en Q4_0), y su `block_count` es 65 con
  `nextn_predict_layers = 1`.

**La solución (la que usa el gguffer):**

El gguffer **compara el GGUF con el imatrix bloque a bloque** y pone en
`Q4_0` (que no exige imatrix) los bloques que el imatrix no cubre. Lo hace
por dos vías, y se unen:

- por la **metadata** (`block_count - nextn_predict_layers`), y
- por **comparación directa** con el imatrix (los bloques que el GGUF tiene y
  el imatrix no).

La comparación es la que salva el caso: si el conversor no escribió la
metadata, el bloque 64 sigue sin cubrir. Y es exacta: las normas no están en
ningún imatrix (no se cuantizan), así que se comparan **bloques**, no
tensores.

**Y la solución de fondo: no convertir.** Bartowski publica el BF16 del
Qwen3.8-27B, y ese GGUF ya tiene la MTP bien metida. Cuantizar ese da
exactamente lo mismo, sin tocar el conversor:

```bash
ropero gguffer \
  --gguf bartowski/Qwen3.8-27B-GGUF:Qwen3.8-27B-bf16/Qwen3.8-27B-bf16-00001-of-00002.gguf \
  --quant MXFP4 \
  --imatrix-repo bartowski/Qwen3.8-27B-GGUF \
  --imatrix-file Qwen3.8-27B-imatrix.gguf \
  --out-principal ~/models/gguf/qwen/
```

El GGUF va partido en 2 shards (39,96 GB + 14,70 GB) ⇒ el gguffer los baja
y los une con `llama-gguf-split --merge`. **El fichero de entrada no se
borra nunca**: es del usuario.

## La MTP, que es lo que más falla

El **imatrix se calcula sobre el modelo base (64 capas)**, pero el GGUF lleva
la cabeza MTP como **un bloque más** (el 65). Ese bloque no está en ningún
imatrix público, y con un quant very-low-bit (IQ2_M, IQ3_XXS…) el
cuantizador aborta:

```
Missing importance matrix for tensor blk.64.attn_k.weight in a very low-bit quantization
```

**La solución es la que usaba bartowski**: ese bloque se queda en **Q4_0**,
que no exige imatrix. El gguffer lo detecta solo y añade:

```bash
--tensor-type "blk\.64\..*=Q4_0"
```

**La MTP se localiza por la METADATA, no por el nombre del tensor.** Los
tensores de la MTP se llaman como los de cualquier otro bloque
(`blk.64.attn_k.weight`), SIN "nextn" en el nombre. Lo que sí lo dice:

```
bloque_mtp = block_count - nextn_predict_layers
```

El conversor suma los bloques de la MTP al `block_count`
(`conversion/qwen.py: self.block_count += n_mtp`).

**Si se quiere el GGUF SIN MTP**, el flag es `--no-mtp` (lo pasa el gguffer al
conversor, y solo si la arch lo admite: el nemotron-h-moe no lo admite, y
entonces no hace falta, porque su conversor ya deja fuera la nextn).

## Los errores que se ven de noche, y qué son

| el error | qué es | qué hacer |
|---|---|---|
| `No space left on device` | se llenó el disco con el GGUF intermedio | borrar el clon, liberar disco, `GGUF_WORK_DIR` a otro sitio |
| `Cannot allocate memory` / `Killed` | el conversor se comió la RAM | el GGUF intermedio es del tamaño de los pesos; con F32 es la mitad (`--intermedio f16` es el default) |
| `Missing importance matrix for tensor blk.NN` | la MTP sin cobertura | el gguffer lo debería arreglar solo: mira el diagnóstico |
| `Tokenizer class TokenizersBackend does not exist` | el repo se guardó con transformers v5 | el gguffer lo parchea a `PreTrainedTokenizerFast` y reintenta solo |
| `invalid nthread '--imatrix'` | las flags detrás de los posicionales | el gguffer las pone delante; si se llama a mano, `--imatrix` antes |
| `requantizing from type X is disabled` | se está cuantizando un GGUF **ya cuantizado** | los pesos se sacan del repo **sin cuantizar** (la columna de la tabla) |

### El caso `nvidia/...-NVFP4`: un repo que YA viene cuantizado

Los repos de NVIDIA con `NVFP4` o `FP8` en el nombre **no son los pesos
crudos**: son checkpoints ya cuantizados (con `modelopt`, ver su
`hf_quant_config.json`: `quant_algo: W4A16_NVFP4`). Al convertir uno de
ellos, el GGUF intermedio sale con los tensores de experts **ya en tipo
`nvfp4`**, y entonces:

```
llama_model_quantize: failed to quantize: requantizing from type nvfp4 is disabled
```

No es un fallo del imatrix ni del cuantizador: `llama-quantize` no sabe
re-cuantizar desde `nvfp4` (no hay pérdida que re-derivar). Para fabricar un
quant propio desde un repo de NVIDIA hay que usar el de **BF16**
(`nvidia/...-BF16`), que sí son los pesos sin cuantizar.

Si lo que se quiere es el NVFP4 ya hecho, no hay que fabricar nada: está
publicado, y además **sin MTP embebida**, que es lo que lo hace arrancable:

| Repo | Fichero | Tamaño | MTP |
|---|---|---|---|
| `tngtech/...-NVFP4-GGUF` | `Nemotron-3.5-Lightning-30B-A3B-NVFP4.gguf` | 17,22 GiB | suelta (`mtp-*.gguf`) |
| `vcruz305/...-GGUF` | `Nemotron-3.5-Lightning-30B-A3B-MXFP4_MOE.gguf` | 16,75 GiB | ninguna |

Los dos cargan. Y el `unsloth/...-GGUF` **no**: lleva la MTP embebida.

### Que un GGUF con MTP embebida NO arranca

Un GGUF con la MTP dentro (`block_count` = bloques + 1 y
`nextn_predict_layers = 1`) **no carga** con este motor:

```
done_getting_tensors: wrong number of tensors; expected 417, got 408
```

Porque el motor no lee `nextn_predict_layers` (la clave existe en
`llama-arch.cpp` pero nadie la consume), cuenta 53 capas en vez de 52, y el
bloque de la MTP (`blk.52`) aporta 16 tensores de los que solo crea 7. Los
9 que sobran son `blk.52.attn_k|attn_q|attn_v|attn_output.weight`, los
`blk.52.nextn.*` y `blk.52.post_attention_norm.weight`.

Esto no se arregla con un flag: es laMetadata del **fichero**, y se comprueba
sin arrancar:

```bash
python3 bin/_ggufcheck.py RUTA/FICHERO.gguf --meta
```

Si `nextn_predict_layers=1`, ese fichero no arranca. Para uno sin MTP, o
se descarga otro, o se fabrica desde los pesos sin cuantizar (el conversor
del nemotron-h-moe ya deja fuera la nextn: `conversion/nemotron.py` salta
los tensores `mtp.*`).
