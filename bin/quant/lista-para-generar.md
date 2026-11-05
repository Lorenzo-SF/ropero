# Que generar y subir a HuggingFace

La regla: **el quant que fabrique tiene que ser el que NO CABE con lo que hay
en HF**, o el que aporta velocidad (MXFP4/NVFP4 en Blackwell) o el que nadie
tiene (TQ4_1S, el quant de tu fork).

Presupuesto de VRAM (RTX 5080, 15.300 MiB libres): pesos + KV turbo3
(14 KiB/token) + overhead (~1.200 MiB) <= 15.300 MiB.

---

## VARIOS QUANTS EN UNA TIRADA

Desde el commit `f7446b8`, el gguffer acepta varios quants de golpe:

```bash
ropero gguffer --repo owner/repo --quant IQ2_XXS --quant IQ3_XS
ropero gguffer --repo owner/repo --quant IQ2_XXS,IQ3_XS
```

**Por que importa: bajar y convertir los pesos es lo caro (40-60 GB y una
hora). Cuantizar es solo CPU.** El GGUF intermedio se convierte UNA vez y se
reutiliza para todos los quants de la tirada. Dos quants cuestan lo mismo que
uno, en disco y en tiempo.

El **primer** quant de la lista es el "principal": ese va a la ruta que espera
el TOML. Los demas quedan en `$GGUF_DIR/_variantes/<modelo>/`, con su nombre
`<repo>-<quant>.gguf`, para comparar o subir a HF.

El script `/workspace/output/cuantizar-todos.sh` ya lo hace asi: cada linea
lleva la lista de quants separados por comas.

---

## TIER 1: los que NO CABEN hoy


### 1. fable -> IQ2_XXS (principal) + IQ2_M

El hueco mas grande: en HF no hay NADA por debajo de IQ3_S. El Q4_K_M son 20,2 GiB y no caben ni con ctx 131072.

```bash
ropero gguffer --repo Achilles1089/fable-coder-35B-A3B \
    --quant IQ2_XXS --quant IQ2_M \
    --imatrix-repo mradermacher/fable-coder-35B-A3B-i1-GGUF \
    --imatrix-file fable-coder-35B-A3B.imatrix.gguf \
    --out-principal "$HOME/models/gguf/fable/fable-coder-35B-A3B.Q2_K.gguf" \
    --out "$HOME/models/gguf/_variantes/fable"
```

El IQ2_XXS son 8,4 GiB y entra con ctx 262144. El IQ2_M son 11,0 GiB: con
ctx 262144 no cabe, pero sirve para comparar calidad contra el published.

El nombre del principal (`fable-coder-35B-A3B.Q2_K.gguf`) es el `[files] gguf`
del TOML de ese modelo: el launcher lo busca asi, sin tocar nada.


### 2. nemotron -> IQ2_M

Lo mas pequeno que hay es un Q2_K de 16,7 GiB: no cabe. IQ2_M son 9,4 GiB y entra con ctx 262144.

```bash
ropero gguffer --repo nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16 \
    --quant IQ2_M \
    --imatrix-repo bartowski/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF \
    --imatrix-file NVIDIA-Nemotron-3.5-Lightning-30B-A3B-imatrix.gguf \
    --out-principal "$HOME/models/gguf/nemotron/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-UD-IQ3_XXS.gguf"
```

**SIN `--no-mtp`.** El conversor no admite esa flag para
`NemotronHForCausalLM` y el repo BF16 de NVIDIA no trae los pesos de la
nextn: el conversor ya la deja fuera sola. El imatrix de bartowski cubre
justo los bloques 0..51 que salen.

El nombre del principal es el `[files] gguf` del TOML de ese modelo.


### 3. qwopus -> IQ2_M

Lo mas pequeno que hay es un Q3_K_M de 16,0 GiB: no cabe. IQ2_M son 10,1 GiB y entra con ctx 262144.

```bash
ropero gguffer --repo Jackrong/Qwopus3.6-35B-A3B-Coder \
    --quant IQ2_M \
    --imatrix-repo mradermacher/Qwopus3.6-35B-A3B-Coder-i1-GGUF \
    --imatrix-file Qwopus3.6-35B-A3B-Coder.imatrix.gguf \
    --out-principal "$HOME/models/gguf/qwopus/Qwopus3.6-35B-A3B-Coder-vram13-q2ex-imat.gguf"
```


### 4. laguna -> IQ2_XXS (principal) + IQ2_M

Solo hay un Q4_K_M (18,9 GiB) y un BF16: no cabe. IQ2_XXS son 8,0 GiB y entra con ctx 262144.

```bash
ropero gguffer --repo poolside/Laguna-XS-2.1 \
    --quant IQ2_XXS --quant IQ2_M \
    --imatrix-repo bartowski/Laguna-XS-2.1-GGUF \
    --imatrix-file Laguna-XS-2.1-imatrix.gguf \
    --out-principal "$HOME/models/gguf/laguna/Laguna-XS-2.1-APEX-I-Mini.gguf" \
    --out "$HOME/models/gguf/_variantes/laguna"
```


### 5. agenta1 -> IQ2_XXS

El IQ2_M ya existe (KikoCis), pero el IQ2_XXS no: 8,4 GiB, y da ctx 262144 en vez de 131072.

```bash
ropero gguffer --repo InternScience/Agents-A1 \
    --quant IQ2_XXS \
    --imatrix-repo KikoCis/Agents-A1-IQ2_M-GGUF \
    --imatrix-file Agents-A1.imatrix \
    --out-principal "$HOME/models/gguf/agenta1/Agents-A1-IQ2_M.gguf"
```


---

## TIER 2: por velocidad o por contribution al fork

Aqui la tirada multiple es la que mas compensa: el quant "bueno" y el rapido
salen de la MISMA descarga.


### 6. devstral -> Q3_K_M (principal) + MXFP4

No es por tamano (el Q3_K_M de 11,5 GB ya cabe): es por VELOCIDAD. MXFP4 usa los tensor cores FP4 nativos de Blackwell y no existe para este modelo.

```bash
ropero gguffer --repo mistralai/Devstral-Small-2-24B-Instruct-2512 \
    --quant Q3_K_M --quant MXFP4 \
    --imatrix-repo unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF \
    --imatrix-file imatrix_unsloth.gguf_file \
    --out-principal "$HOME/models/gguf/devstral/<NOMBRE>.gguf" \
    --out "$HOME/models/gguf/_variantes/devstral"
```


### 7. qwen -> IQ3_XS (principal) + MXFP4

Tambien por velocidad: MXFP4 en los tensor cores FP4 de Blackwell. No existe para el qwen denso. 13,4 GiB -> ctx 32768.

```bash
ropero gguffer --repo Qwen/Qwen3.8-27B \
    --quant IQ3_XS --quant MXFP4 \
    --imatrix-repo bartowski/Qwen3.8-27B-GGUF \
    --imatrix-file Qwen3.8-27B-imatrix.gguf \
    --out-principal "$HOME/models/gguf/qwen/Qwen3.8-27B-IQ3_XS.gguf" \
    --out "$HOME/models/gguf/_variantes/qwen"
```

OJO: el imatrix no cubre el bloque 64 (la MTP). El gguffer lo pone solo en
Q4_0, que no lo exige. El MXFP4 tambien lo lleva.


### 8. agenta-tiny -> Q8_0 (principal) + TQ4_1S

El TQ4_1S del fork turboquant no existe para NINGUN modelo del roster. En un 4B cabe de sobra y es el contribution al fork.

```bash
ropero gguffer --repo InternScience/Agents-A1-4B \
    --quant Q8_0 --quant TQ4_1S \
    --imatrix-repo bartowski/InternScience_Agents-A1-4B-GGUF \
    --imatrix-file InternScience_Agents-A1-4B-imatrix.gguf \
    --out-principal "$HOME/models/gguf/agenta-tiny/<NOMBRE>.gguf" \
    --out "$HOME/models/gguf/_variantes/agenta-tiny"
```

El TQ4_1S solo existe en los forks de turboquant. Si tu motor no lo lista en
`ropero gguffer --list-quants`, no lo tiene: hay que compilar el fork.


---

## Lo que NO hay que fabricar (bajalo de HF)

| Modelo | Lo que hay | Por que |
|---|---|---|
| qwen | IQ2_M, 10,5 GB | ya cabe con ctx de sobra |
| qwen36 | IQ3_XXS | ya cabe |
| ornith | IQ2_M | ya cabe |
| gemma | IQ3_M | ya cabe |
| muse | Q3_K_XL | ya cabe |
| qwencoder | IQ3_XXS | ya cabe |
| qwenvision | Q4_K_XL | ya cabe |
| gptoss | MXFP4 | ya viene en MXFP4 nativo |
| agenta-tiny | Q8_0 | ya cabe (el TQ4_1S si es tuyo) |

---

## Subir a HuggingFace

```bash
./subir-hf.sh fable <usuario>/fable-coder-35B-A3B-IQ2_XXS-GGUF \
    --base Achilles1089/fable-coder-35B-A3B --quant IQ2_XXS
```

El script crea el repo (si no existe), sube el GGUF y su imatrix, y genera el
README con el `base_model` y el quant.

**En el README, di si el bloque de la MTP se quedo en Q4_0** por falta de
cobertura del imatrix. Es la diferencia entre "este quant es raro" y "este
quant es raro por una razon que se puede ver" — y en tu caso es un detalle
que solo tu puedes explicar, porque el imatrix no cubre la MTP por
construccion.
