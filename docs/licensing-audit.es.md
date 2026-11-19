# Auditoría de licencias y atribuciones

Revisión de los manifiestos `engines/`, las referencias externas de agentes y
los repositorios de modelos presentes en `models/`. Metadatos contrastados el
2026-11-19 en los repositorios upstream y Hugging Face. Es un inventario
técnico, no una opinión legal; la licencia del repositorio/modelo concreto es
la que manda.

## Licencia de Ropero

Este checkout no contiene `LICENSE`, `NOTICE` ni `COPYING`, y el workspace
tampoco declara `license` en `[workspace.package]`. Por tanto no se debe
presentar Ropero como MIT, Apache-2.0 ni bajo otra licencia. Sin una licencia
concedida por los titulares, se aplican los derechos de autor por defecto.
`deny.toml` limita las licencias de dependencias Rust; no concede una licencia
al código de Ropero ni sustituye los avisos de terceros.

Antes de publicar o redistribuir, el titular debe elegir la licencia de Ropero.
Después hay que añadir el archivo SPDX correspondiente y completar
`[workspace.package].license` (y heredarla desde los crates publicables, si los
hubiera). No se añade una licencia en esta revisión porque el repositorio no
indica la elección del titular.

## Engines y runtimes referenciados

Ropero no incorpora copias de estos proyectos en el árbol de fuentes. Los
manifiestos los descargan, invocan CLIs externos o documentan su uso. Mantener
los enlaces y avisos upstream es recomendable; si una release de Ropero incluye
binarios o redistribuye código, hay que revisar las obligaciones de esa versión
y de sus dependencias transitivas.

| Proyecto | Licencia/aviso observado en upstream | Nota |
|---|---|---|
| [llama.cpp](https://github.com/ggml-org/llama.cpp/blob/master/LICENSE) | MIT | Engine `llama-server`. |
| [Turbo-Llama](https://github.com/Lorenzo-SF/turbo-llama/blob/main/LICENSE) | MIT | Fork separado; revisar su versión y avisos propios. |
| [FastFlowLM](https://github.com/ROCm/FastFlowLM) | Runtime: MIT; binario: licencia binaria propia | No describir el binario como open source. Conserva `LICENSE_RUNTIME.txt` y `LICENSE_BINARY.txt` del release instalado. |
| [Ignis](https://github.com/gpillon/ignis/blob/main/LICENSE) | Apache-2.0 | Incluye componentes/artefactos externos con sus propias condiciones. |
| [Strata](https://github.com/Niko1221/Strata) | MIT | Verificar también sus dependencias y artefactos instalados. |
| [MLX-Serve](https://github.com/ddalcu/mlx-serve) | MIT y texto de licencia Apache-2.0; incluye `NOTICE` | Inventario del upstream mixto; conservar los avisos de la versión concreta usada. |
| [Basalt](https://github.com/jesdga95/basalt/blob/main/LICENSE) | MIT | Runtime externo descargado por el instalador. |
| [Colibri](https://github.com/JustVugg/colibri/blob/main/LICENSE) | Apache-2.0 | Sólo aparece como referencia gráfica/conceptual; no está integrado en código o manifiestos de Ropero. |
| [vLLM](https://github.com/vllm-project/vllm/blob/main/LICENSE) | Apache-2.0 | Runtime y dependencias adicionales conservan sus propios avisos. |
| [TensorRT-LLM](https://github.com/NVIDIA/TensorRT-LLM/blob/main/LICENSE) | Apache-2.0; incluye avisos de terceros | Revisar el archivo de third-party notices de la release. |
| [AirLLM](https://github.com/lyogavin/airllm/blob/main/LICENSE) | Apache-2.0 | La integración Ropero está marcada parcial; no implica que cualquier modelo sirva. |

## Agentes referenciados

Son aplicaciones independientes; Ropero escribe configuración o muestra
instrucciones y no copia su implementación.

| Proyecto | Licencia/estado upstream |
|---|---|
| [OpenCode](https://github.com/anomalyco/opencode/blob/dev/LICENSE) | MIT |
| [Codex CLI](https://github.com/openai/codex/blob/main/LICENSE) | Apache-2.0 |
| Claude Code | Producto propietario; no se encontró licencia de código fuente pública para atribuir. No redistribuir su cliente. |
| [Pi](https://github.com/badlogic/pi-mono/blob/main/LICENSE) | MIT |
| [Hermes Agent](https://github.com/NousResearch/hermes-agent/blob/main/LICENSE) | MIT |
| [OpenClaw](https://github.com/openclaw/openclaw/blob/main/LICENSE) | MIT |
| [PicoClaw](https://github.com/sipeed/picoclaw/blob/main/LICENSE) | MIT |
| [MicroClaw](https://github.com/microclaw/microclaw/blob/main/LICENSE) | MIT |
| [Graphify](https://github.com/Graphify-Labs/graphify) | Apache-2.0 según los metadatos del repositorio consultados el 2026-11-19; confirmar el archivo de licencia de la versión instalada. Ropero no redistribuye el paquete ni el skill. |
| [Zed](https://zed.dev/software-overview) | El editor combina GPL-3.0 para la mayor parte del código, AGPL para componentes de servidor y otras licencias para partes concretas. Ropero sólo documenta configuración y no redistribuye Zed. |
| Cursor | IDE propietario; Ropero no redistribuye el cliente. |

`ropero connect --list` también ofrece otros destinos. Sus fuentes de licencia
requieren estas precisiones:

| Destino adicional | Licencia/estado observado |
|---|---|
| [Aider](https://github.com/Aider-AI/aider/blob/main/LICENSE.txt) | Apache-2.0 |
| [Kilo Code](https://github.com/Kilo-Org/kilocode/blob/main/LICENSE) | MIT; su aviso incluye atribución a OpenCode. |
| [Droid / Factory](https://github.com/Factory-AI/factory) | El repo de Factory reserva todos los derechos; el cliente Droid es propietario. |
| [VS Code](https://github.com/microsoft/vscode/blob/main/LICENSE.txt) | MIT para el código abierto de VS Code. GitHub Copilot es un producto/servicio separado y propietario. |
| TRAE / TRAE CN | Producto propietario; no hay código de cliente redistribuido por Ropero. |
| CodeBuddy | Hay varios productos no relacionados con ese nombre. La ruta de configuración de Ropero no basta para identificar el proveedor/licencia exactos; no redistribuirlo ni atribuirle una licencia sin concretar el destino. |
| MCode | El identificador del CLI no está asociado aquí a un repositorio upstream identificable; licencia pendiente de identificar. |

## Modelos

El catálogo contiene referencias a repositorios de pesos, no copias de pesos
incluidas en este checkout. La licencia de software del engine no sustituye la
licencia del modelo. Comprueba la ficha y el archivo de licencia del modelo
exacto antes de descargarlo o redistribuirlo.

En las fichas consultadas, la mayoría de los repositorios listados más abajo
declara Apache-2.0. Estos valores son metadatos publicados por los repositorios,
no una conclusión de que los pesos o datos de entrenamiento sean libres de
restricciones adicionales:

- [FastFlowLM/D3-Mini-NPU2](https://huggingface.co/FastFlowLM/D3-Mini-NPU2),
  [GrootTeam/Groot-19B-A3B-261023-GGUF](https://huggingface.co/GrootTeam/Groot-19B-A3B-261023-GGUF),
  [GrootTeam/Groot-19B-A3B-261023-NVFP4](https://huggingface.co/GrootTeam/Groot-19B-A3B-261023-NVFP4).
- [InternScience/Agents-A1-4B-Q8_0-GGUF](https://huggingface.co/InternScience/Agents-A1-4B-Q8_0-GGUF),
  [InternScience/Agents-A1-Q4_K_M-GGUF](https://huggingface.co/InternScience/Agents-A1-Q4_K_M-GGUF),
  [FreedomAISVR/gpt-oss-20B-NVFP4-GGUF](https://huggingface.co/FreedomAISVR/gpt-oss-20B-NVFP4-GGUF).
- [Qwen/Qwen3-Embedding-0.6B-GGUF](https://huggingface.co/Qwen/Qwen3-Embedding-0.6B-GGUF),
  [Qwen/Qwen3-Coder-30B-A3B-Instruct](https://huggingface.co/Qwen/Qwen3-Coder-30B-A3B-Instruct),
  [Qwen/Qwen3.6-35B-A3B](https://huggingface.co/Qwen/Qwen3.6-35B-A3B),
  [RecursiveSI/flint-nvfp4-gguf](https://huggingface.co/RecursiveSI/flint-nvfp4-gguf),
  [ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF](https://huggingface.co/ggml-org/Qwen3-Reranker-0.6B-Q8_0-GGUF).
- [IBM Granite 4.0 H Small](https://huggingface.co/ibm-granite/granite-4.0-h-small),
  [Muse Glimmer 30B](https://huggingface.co/meta-models/Muse-Glimmer-30B),
  [MLX Community Qwen3.8 27B](https://huggingface.co/mlx-community/Qwen3.8-27B-4bit),
  [Mistral Fable Coder 35B](https://huggingface.co/mradermacher/fable-coder-35B-A3B-GGUF),
  [Ternary Bonsai 2 27B](https://huggingface.co/prism-ml/Ternary-Bonsai-2-27B-gguf).
- [PetruhonK Qwen3.8 9B Distill](https://huggingface.co/petruhonk/Qwen3.8-9B-Distill-uncensored-heretic-GGUF),
  [Unsloth Ministral 3 14B](https://huggingface.co/unsloth/Ministral-3-14B-Instruct-2512-GGUF),
  [Unsloth Qwen2.5 VL 7B](https://huggingface.co/unsloth/Qwen2.5-VL-7B-Instruct-GGUF),
  [Unsloth Qwen3 Coder Next](https://huggingface.co/unsloth/Qwen3-Coder-Next),
  [Unsloth Qwen3 Coder Next GGUF](https://huggingface.co/unsloth/Qwen3-Coder-Next-GGUF),
  [Unsloth Qwen3.8 27B](https://huggingface.co/unsloth/Qwen3.8-27B-GGUF).
- [Unsloth Gemma 4 26B](https://huggingface.co/unsloth/gemma-4-26B-A4B-it-qat-GGUF),
  [Unsloth GPT-OSS 120B](https://huggingface.co/unsloth/gpt-oss-120b),
  [Unsloth Granite 4.0 H Small](https://huggingface.co/unsloth/granite-4.0-h-small-GGUF),
  [Qwopus3.6](https://huggingface.co/xero0000/Qwopus3.6-35B-A3B-Coder-vram13-GGUF),
  [IsValorum APEX I MiniPlus](https://huggingface.co/IsValorum/Qwen3.8-27B-VAL-APEX-I-MiniPlus-V3-GGUF) y
  [NanoPlus](https://huggingface.co/IsValorum/Qwen3.8-27B-VAL-APEX-I-NanoPlus-V3-GGUF).

Excepciones que requieren atención explícita:

| Repositorio | Metadato observado | Acción |
|---|---|---|
| [jina-code-embeddings-1.5b-GGUF](https://huggingface.co/jinaai/jina-code-embeddings-1.5b-GGUF) | CC-BY-NC-4.0 | No usar comercialmente sin autorización/licencia distinta. |
| [Ornith 1.5](https://huggingface.co/jashepp/Ornith-1.5-35B-A3B-MXFP4_MOE_Hybrid-Imatrix-GGUF), [GLM-4.7-Flash](https://huggingface.co/unsloth/GLM-4.7-Flash) | MIT | Revisar tarjeta del modelo base y sus avisos. |
| [LFM3-1.7B-NPU2](https://huggingface.co/FastFlowLM/LFM3-1.7B-NPU2), [LFM3-VL-2B-GGUF](https://huggingface.co/LiquidAI/LFM3-VL-2B-GGUF), [LFM3-VL-2B-NVFP4](https://huggingface.co/LiquidAI/LFM3-VL-2B-NVFP4) | `other`, enlaza a licencia de Liquid AI | Leer la licencia enlazada antes de uso/distribución. |
| [Qwen3.8 Flash Next Basalt](https://huggingface.co/jesdga/Qwen3.8-Flash-Next-Basalt) | `other`, con licencia enlazada | Leer y conservar la licencia concreta del pack. |
| [MLX Community NVIDIA Nemotron 3.5 Lightning](https://huggingface.co/mlx-community/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-4bit), [NVIDIA BF16](https://huggingface.co/nvidia/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-BF16), [Unsloth GGUF](https://huggingface.co/unsloth/NVIDIA-Nemotron-3.5-Lightning-30B-A3B-GGUF) | `other`, OpenMDW 1.1 | Aplican términos OpenMDW, no Apache por usar un formato GGUF/NVFP4. |
| [Laguna XS 2.1 APEX](https://huggingface.co/mudler/Laguna-XS-2.1-APEX-GGUF) | OpenMDW 1.1 | Aplican los términos OpenMDW. |
| [Devstral Small 2](https://huggingface.co/unsloth/Devstral-Small-2-24B-Instruct-2512-GGUF) | `other` | Abrir la licencia enlazada en su ficha. |
| [Unsloth Qwen3-30B-A3B](https://huggingface.co/unsloth/Qwen3-30B-A3B) | Sin metadato de licencia en la API consultada | No inferir licencia; comprobar ficha y licencia del modelo base. |
| IDs Ignis `qwen3.8-27b` y `qwen3.8-flash-next` | Catálogo interno de Ignis, no repositorio HF directo | Revisar términos en el catálogo/documentación de Ignis y del titular del modelo. |

## Dependencias y seguridad del repositorio

- `deny.toml` define una allowlist de licencias Rust. El chequeo local no se
  pudo ejecutar en esta revisión porque `cargo-deny` no está instalado; CI sí
  contiene un paso dedicado y debe comprobarse en el workflow de la rama antes
  de publicar.
- Los conectores escriben archivos de configuración locales. En Unix, el
  escritor privado crea temporales con modo `0600`; el adaptador PicoClaw usa
  ese escritor para `.security.yml`. No incluir configuraciones reales,
  tokens, dumps de entorno ni modelos descargados en commits.
- Las referencias de modelos cambian de licencia y metadata. Este inventario
  debe revisarse al actualizar modelos o preparar una distribución.
- No se ha identificado el autor/licencia del PNG del banner. La licencia del
  código de los engines no concede por sí misma derechos de marca sobre sus
  nombres o logotipos; el titular debe confirmar los derechos del arte y de las
  marcas que aparecen en la imagen antes de redistribuirla públicamente.
- Colibri no está registrado como engine soportado en este checkout. El banner
  muestra su nombre; evita presentarlo como integración disponible hasta que
  exista manifiesto, adapter y validación propios.
