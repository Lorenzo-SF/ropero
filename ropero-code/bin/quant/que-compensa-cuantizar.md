# Qué merece la pena cuantizar con el gguffer

La pregunta no es "qué quants no hay en HF" (eso casi siempre es "algunos IQ
mas"). La pregunta es: **¿el modelo te cabe en los 15.300 MiB de la RTX 5080
con el contexto que quieres?** Si ya cabe, bajalo de HF: rehacerlo te cuesta
40-60 GB de descarga y una hora de CPU para obtener lo mismo.

## El filtro

Pesos + KV (turbo3, 14 KiB/token) + overhead (~1.200 MiB) <= 15.300 MiB

Para ctx 262.144 eso deja **10,7 GiB de pesos** como maximo.

## Los que NO caben (si merece la pena fabricar)

| Modelo | Lo mas pequeno que hay | Con ctx 262144 | Que hay que fabricar |
|---|---|---|---|
| **fable** | Q4_K_M, 20,2 GiB | NO CABE (-10.192 MiB) | **IQ2_M** (~11 GiB) o **IQ2_XXS** (~8,8 GiB) |
| **nemotron** | Q2_K, 16,7 GiB | NO CABE (-6.563 MiB) | **IQ2_M** (~9,4 GiB) |
| **laguna** | Q4_K_M, 18,9 GiB | NO CABE (-8.819 MiB) | **IQ2_M** (~10,5 GiB) |
| **qwopus** | Q3_K_M, 16,0 GiB | NO CABE (-5.855 MiB) | **IQ2_M** (~10 GiB) |

**En HF no hay NADA por debajo de Q3_K_M de esos cuatro.** En el caso del
fable, el hueco es total: lo mas pequeno que hay es un Q4_K_M (de los peores
quants) y un IQ3_S.

Con un IQ2_M, los tres MoE grandes (fable, nemotron, laguna) entrarian con
ctx 262.144, que ahora mismo no tienen.

## Los que ya caben: bajalos de HF

| Modelo | Lo que hay | Por que no hay que rehacerlo |
|---|---|---|
| agenta1 | IQ2_M (KikoCis), 11,7 GB | ya cabe; rehacerlo da el mismo fichero |
| qwen | IQ2_M, 10,5 GB | ya cabe con margen |
| qwen36 | IQ3_XXS, 13,2 GB | ya cabe |
| ornith | IQ2_M, 12,5 GB | ya cabe |
| gemma | IQ3_M, 13,3 GB | ya cabe |
| muse | Q3_K_XL, 13,4 GB | ya cabe |
| devstral | Q3_K_M, 11,5 GB | ya cabe |
| qwencoder | (ya lo tienes) | |
| gptoss | MXFP4 nativo | se cuantiza solo, no hay nada que hacer |
| qwenvision | 7B, de sobra | |
| agenta-tiny | 4B, 1,7 GiB | de sobra |

## Los quants "nuevos" que no existen para casi nadie

Si lo que buscas es **velocidad** en Blackwell, no tamano: MXFP4 y NVFP4 usan
los tensor cores FP4 nativos de la RTX 5080, y TQ4_1S es el quant del fork
turboquant. De esos, no existe:

| Quant | No existe para |
|---|---|
| **TQ4_1S** | **ningun modelo del roster** (el fork es tuyo: seria tu contribution) |
| MXFP4 | qwen (denso), fable, devstral, agenta-tiny, qwenvision |
| NVFP4 | fable, muse, devstral, agenta-tiny, qwenvision, agenta1 |

Un TQ4_1S del qwen3.8 o del fable no lo tiene nadie, y es el quant que
aprovecha el trabajo del fork.

## La receta

```bash
# 1. el gguffer baja los pesos, el imatrix, y deja el GGUF en su carpeta
ropero gguffer --repo Achilles1089/fable-coder-35B-A3B \\
    --quant IQ2_M \\
    --imatrix-repo mradermacher/fable-coder-35B-A3B-i1-GGUF \\
    --imatrix-file fable-coder-35B-A3B.imatrix.gguf \\
    --out "$HOME/models/gguf/fable/fable-coder-35B-A3B.Q2_K.gguf"

# 2. el nemotron necesita --no-mtp: su bloque nextn no lo implementa el fork
ropero gguffer --repo nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16 \\
    --quant IQ2_M --no-mtp \\
    --imatrix-repo bartowski/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF \\
    --imatrix-file NVIDIA-Nemotron-3.5-Lightning-30B-A3B-imatrix.gguf \\
    --out "$HOME/models/gguf/nemotron/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-UD-IQ3_XXS.gguf"
```

Ojo con el nombre del fichero final: el del nemotron dice IQ3_XXS pero el
contenido va a ser IQ2_M. Si prefieres que el nombre case, ponle el que
corresponda y actualiza el `[files] gguf` del TOML.

