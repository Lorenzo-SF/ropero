# Cuantizar los GGUFs a mano, a su carpeta

**Un comando por modelo.** El gguffer se encarga de todo: baja los pesos
del repo, los convierte a GGUF, baja el imatrix (y lo cachea), cuantiza y
deja el fichero en su sitio. No hay que clonar nada a mano.

`--out` lleva la ruta EXACTA que espera el `.toml` del modelo, asi que el
GGUF sale ya en su carpeta y `ropero <modelo>` lo encuentra sin tocar nada.

Ojo con el disco: cada modelo baja el repo de pesos entero (40-60 GB) y
genera un GGUF intermedio del mismo tamano. El pico son ~120 GB por modelo.

---

## agenta-tiny  (Q8_0)

El `.toml` lo busca en `$HOME/models/gguf/agenta-tiny/Agents-A1-4B-Q8_0.gguf`

```bash
ropero gguffer --repo InternScience/Agents-A1-4B \
    --quant Q8_0 \
    --imatrix-repo bartowski/InternScience_Agents-A1-4B-GGUF \
    --imatrix-file InternScience_Agents-A1-4B-imatrix.gguf \
    --out "$HOME/models/gguf/agenta-tiny/Agents-A1-4B-Q8_0.gguf"
```

## agenta1  (IQ2_M)

El `.toml` lo busca en `$HOME/models/gguf/agenta1/Agents-A1-IQ2_M.gguf`

```bash
ropero gguffer --repo InternScience/Agents-A1 \
    --quant IQ2_M \
    --imatrix-repo KikoCis/Agents-A1-IQ2_M-GGUF \
    --imatrix-file Agents-A1.imatrix \
    --out "$HOME/models/gguf/agenta1/Agents-A1-IQ2_M.gguf"
```

## devstral  (Q3_K_M)

El `.toml` lo busca en `$HOME/models/gguf/devstral/Devstral-Small-2-24B-Instruct-2512-Q3_K_M.gguf`

```bash
ropero gguffer --repo mistralai/Devstral-Small-2-24B-Instruct-2512 \
    --quant Q3_K_M \
    --imatrix-repo unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF \
    --imatrix-file imatrix_unsloth.gguf_file \
    --out "$HOME/models/gguf/devstral/Devstral-Small-2-24B-Instruct-2512-Q3_K_M.gguf"
```

## fable  (Q2_K)

El `.toml` lo busca en `$HOME/models/gguf/fable/fable-coder-35B-A3B.Q2_K.gguf`

```bash
ropero gguffer --repo Achilles1089/fable-coder-35B-A3B \
    --quant Q2_K \
    --imatrix-repo mradermacher/fable-coder-35B-A3B-i1-GGUF \
    --imatrix-file fable-coder-35B-A3B.imatrix.gguf \
    --out "$HOME/models/gguf/fable/fable-coder-35B-A3B.Q2_K.gguf"
```

## gemma  (IQ3_M)

El `.toml` lo busca en `$HOME/models/gguf/gemma/google_gemma-4-26B-A4B-it-IQ3_M.gguf`

```bash
ropero gguffer --repo google/gemma-4-26B-A4B-it \
    --quant IQ3_M \
    --imatrix-repo bartowski/google_gemma-4-26B-A4B-it-GGUF \
    --imatrix-file google_gemma-4-26B-A4B-it-imatrix.gguf \
    --out "$HOME/models/gguf/gemma/google_gemma-4-26B-A4B-it-IQ3_M.gguf"
```

## gptoss  (MXFP4)

El `.toml` lo busca en `$HOME/models/gguf/gptoss/openai_gpt-oss-20b-MXFP4.gguf`

```bash
ropero gguffer --repo openai/gpt-oss-20b \
    --quant MXFP4 \
    --imatrix-repo bartowski/openai_gpt-oss-20b-GGUF \
    --imatrix-file openai_gpt-oss-20b-imatrix.gguf \
    --out "$HOME/models/gguf/gptoss/openai_gpt-oss-20b-MXFP4.gguf" \
    --from-subdir original
```

## laguna  (IQ3_M)

El `.toml` lo busca en `$HOME/models/gguf/laguna/Laguna-XS-2.1-APEX-I-Mini.gguf`

```bash
ropero gguffer --repo poolside/Laguna-XS-2.1 \
    --quant IQ3_M \
    --imatrix-repo bartowski/Laguna-XS-2.1-GGUF \
    --imatrix-file Laguna-XS-2.1-imatrix.gguf \
    --out "$HOME/models/gguf/laguna/Laguna-XS-2.1-APEX-I-Mini.gguf"
```

## muse  (Q3_K_XL)

El `.toml` lo busca en `$HOME/models/gguf/muse/Muse-Glimmer-30B-UD-Q3_K_XL.gguf`

```bash
ropero gguffer --repo meta-models/Muse-Glimmer-30B \
    --quant Q3_K_XL \
    --imatrix-repo AtomicChat/Muse-Glimmer-30B-GGUF \
    --imatrix-file imatrix/muse-glimmer.imatrix.gguf \
    --out "$HOME/models/gguf/muse/Muse-Glimmer-30B-UD-Q3_K_XL.gguf"
```

## nemotron  (IQ3_XXS)

El `.toml` lo busca en `$HOME/models/gguf/nemotron/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-UD-IQ3_XXS.gguf`

```bash
ropero gguffer --repo nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16 \
    --quant IQ3_XXS \
    --imatrix-repo bartowski/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF \
    --imatrix-file NVIDIA-Nemotron-3.5-Lightning-30B-A3B-imatrix.gguf \
    --out "$HOME/models/gguf/nemotron/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-UD-IQ3_XXS.gguf" \
    --no-mtp
```

## ornith  (IQ2_M)

El `.toml` lo busca en `$HOME/models/gguf/ornith/Ornith-1.5-35B-A3B-IQ2_M.gguf`

```bash
ropero gguffer --repo ornith-ai/Ornith-1.5-35B-A3B \
    --quant IQ2_M \
    --imatrix-repo bartowski/Ornith-1.5-35B-A3B-GGUF \
    --imatrix-file Ornith-1.5-35B-A3B-imatrix.gguf \
    --out "$HOME/models/gguf/ornith/Ornith-1.5-35B-A3B-IQ2_M.gguf"
```

## qwen  (IQ2_M)

El `.toml` lo busca en `$HOME/models/gguf/qwen/Qwen3.8-27B-IQ2_M.gguf`

```bash
ropero gguffer --repo Qwen/Qwen3.8-27B \
    --quant IQ2_M \
    --imatrix-repo bartowski/Qwen3.8-27B-GGUF \
    --imatrix-file Qwen3.8-27B-imatrix.gguf \
    --out "$HOME/models/gguf/qwen/Qwen3.8-27B-IQ2_M.gguf"
```

## qwen36  (IQ3_XXS)

El `.toml` lo busca en `$HOME/models/gguf/qwen36/Qwen3.6-35B-A3B-UD-IQ3_XXS.gguf`

```bash
ropero gguffer --repo Qwen/Qwen3.6-35B-A3B \
    --quant IQ3_XXS \
    --imatrix-repo unsloth/Qwen3.6-35B-A3B-GGUF \
    --imatrix-file imatrix_unsloth.gguf_file \
    --out "$HOME/models/gguf/qwen36/Qwen3.6-35B-A3B-UD-IQ3_XXS.gguf"
```

## qwencoder  (IQ3_XXS)

El `.toml` lo busca en `$HOME/models/gguf/qwencoder/Qwen3-Coder-30B-A3B-Instruct-UD-IQ3_XXS.gguf`

```bash
ropero gguffer --repo Qwen/Qwen3-Coder-30B-A3B-Instruct \
    --quant IQ3_XXS \
    --imatrix-repo unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF \
    --imatrix-file imatrix_unsloth.gguf_file \
    --out "$HOME/models/gguf/qwencoder/Qwen3-Coder-30B-A3B-Instruct-UD-IQ3_XXS.gguf"
```

## qwenvision  (Q4_K_XL)

El `.toml` lo busca en `$HOME/models/gguf/qwenvision/Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf`

```bash
ropero gguffer --repo Qwen/Qwen2.5-VL-7B-Instruct \
    --quant Q4_K_XL \
    --imatrix-repo unsloth/Qwen2.5-VL-7B-Instruct-GGUF \
    --imatrix-file imatrix_unsloth.dat \
    --out "$HOME/models/gguf/qwenvision/Qwen2.5-VL-7B-Instruct-UD-Q4_K_XL.gguf"
```

## qwopus  (Q2_K)

El `.toml` lo busca en `$HOME/models/gguf/qwopus/Qwopus3.6-35B-A3B-Coder-vram13-q2ex-imat.gguf`

```bash
ropero gguffer --repo Jackrong/Qwopus3.6-35B-A3B-Coder \
    --quant Q2_K \
    --imatrix-repo mradermacher/Qwopus3.6-35B-A3B-Coder-i1-GGUF \
    --imatrix-file Qwopus3.6-35B-A3B-Coder.imatrix.gguf \
    --out "$HOME/models/gguf/qwopus/Qwopus3.6-35B-A3B-Coder-vram13-q2ex-imat.gguf"
```

---
## Un par de cosas

**El nombre lo pone el `--out`, no el quant.** Por eso un IQ2_M puede
acabar en un fichero que se llama `...UD-IQ3_XXS.gguf`. Si prefieres que
el nombre case con el quant, quita el `--out` y renombra a mano:

```bash
ropero gguffer --repo Qwen/Qwen3.8-27B --quant IQ2_M \
    --imatrix-repo bartowski/Qwen3.8-27B-GGUF \
    --imatrix-file Qwen3.8-27B-imatrix.gguf
```

Pero entonces el fichero sale plano en `$GGUF_DIR/`, y el lanzador lo
busca en `$GGUF_DIR/qwen/`: no lo encontrara.

**El imatrix va antes de los posicionales** si llamas al cuantizador a
mano, porque `llama-quantize` solo parsea las opciones mientras el
argumento empiece por `--`:

```bash
llama-quantize --imatrix mi.imatrix entrada.gguf salida.gguf IQ2_M
```

Si la flag va detras, se come el parseo del numero de hilos y el
cuantizador muere con `invalid nthread '--imatrix'`, sin decir nada del
imatrix. El gguffer ya lo hace bien por su cuenta.
