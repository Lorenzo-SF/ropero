# Compatibilidad de engines

[English](engine-support.en.md) · [Español](engine-support.es.md)

Esta guía describe lo que Ropero implementa hoy y separa las comprobaciones locales de la documentación del proveedor. Que un motor exista en un manifiesto o que un formato aparezca en su catálogo no demuestra que su arranque funcione.

## Evidencia local

En el entorno donde se revisó el checkout se encontraron `llama-server`, `turbo-llama-server`, `flm`, `ignis-server` y `strata`; se consultó su versión o ayuda. No se encontraron `basalt`, `vllm`, `mlx-serve`, `trtllm-serve` ni `airllm`, por lo que esos motores no se probaron localmente. Los resultados describen ese equipo y esas versiones, no todas las instalaciones de Ropero.

## Motores y límites conocidos

| Engine | Qué hace Ropero | Estado de evidencia y límites |
|---|---|---|
| `llama-server` | Lanza el binario con los argumentos del TOML, añade el GGUF cuando hace falta y fija `--alias` al alias público o al nombre Ropero. | Binario y ayuda comprobados localmente. La documentación oficial describe `-m/--model`, `--alias`, `--host` y `--port`. |
| `turbo-llama` | Usa el fork `turbo-llama-server` con la misma ruta de argumentos GGUF y alias que `llama-server`. | Binario y ayuda del fork comprobados localmente. La documentación de llama.cpp no garantiza que cada opción exista en el fork; hay que verificarla con la ayuda de la versión instalada. |
| `fastflowllm` / FLM | Delega `pull`, `remove`, `list` y `serve` a `flm`. El tag registrado es `[model].alias` si está definido; si no, procede de `[model].path` o de `--decision` en `[args].base` cuando `path` está vacío. Ropero no descarga ese artefacto en su propio directorio. | `flm` local comprobado. El CLI oficial usa tags, con `flm pull <tag>`, `flm serve <tag>` y `flm remove <tag>`. Formato, NPU y plataforma dependen de FastFlowLM/XRT. |
| `ignis` | Para `ninfer`, ejecuta `ignis-server model download <id> --out <ARTIFACTS_DIR>/<name>` y sirve el artefacto desde esa carpeta. Al arrancar usa `--model-id <alias>` cuando el TOML declara `alias`. | `ignis-server` local comprobado. Ignis documenta la descarga del catálogo con `--out` y el id que publica en la API. El runtime de Ropero sólo reconoce los ids/artifacts declarados para los modelos Ignis incluidos; no implica compatibilidad con cualquier modelo. |
| `basalt` | `ropero install engines basalt` descarga e instala el runtime, verifica el SHA-256 del release, prepara `.venv` con sus requisitos Python y, si hace falta, las bibliotecas CUDA 13 de NVIDIA; después enlaza `bin/basalt`. El pack `.basalt` se instala desde `ARTIFACTS_DIR`. El lanzamiento usa `basalt serve <pack> --host 127.0.0.1 --alias <alias> ...` y el puerto asignado por Ropero. | El instalador comprueba CPU AVX2, NVIDIA Blackwell (compute capability 12.0) y driver Linux 580.95.05+. No se ha probado un arranque real en esta máquina. Basalt requiere RAM para los expertos que no caben en VRAM. |
| `strata` | Busca el perfil instalado con `strata models list --installed --output json`, convierte `[model].path` a su id de perfil (por ejemplo, `gyro/Gyro-S` → `gyro-gyro-s`) y lanza `strata serve --profile …`. Añade las opciones y flags del modelo. Si existe `[model].alias`, crea una copia privada del run config con ese id en `aliases`, sin modificar el perfil fuente. El formato Ropero `strata` representa un perfil preparado por Strata. | La ayuda local de `strata serve` confirma las opciones mapeadas (`--parallel`, `--batch-groups`, `--max-queue`, `--queue-timeout`, `--batch-telemetry`, `--offline`). `gyro-s` se detecta como instalado; `gyro-m` figura en Strata pero sus componentes faltan. Ropero no descarga ni crea el perfil. |
| `vllm` | El runtime puede pasar argumentos de modelo y ajusta `--log-level`; el binario esperado es `vllm`. | No instalado localmente: no se probó un arranque real. La documentación oficial documenta `vllm serve` y sus opciones, pero el catálogo actual no contiene modelos vLLM de ejemplo con una receta integral verificada. Tratar como integrado/documentado, no como probado. |
| `mlx-serve` | El runtime pasa `--model` con la ruta local resuelta, fija `--host 127.0.0.1`, añade el puerto y los argumentos del TOML. Para modelos safetensors con `[model].alias`, escribe el alias en `~/.mlx-serve/model-settings.json` y conserva el resto de ajustes. | No instalado localmente: no se probó un arranque real. Upstream documenta los aliases en la versión v26.10.1; se necesita esa versión o una posterior. El proyecto requiere macOS y Apple Silicon. Madurez `integrated`, no `proven`. |
| `tensorrt-llm` | El binario esperado es `trtllm-serve`; los argumentos del modelo vienen del TOML. | No instalado localmente: no se probó un arranque real. La documentación de NVIDIA describe `trtllm-serve`; la compatibilidad depende de versión, GPU, runtime y formato. `groot-trt` se conserva en `models/tensorrt/fired/`: TensorRT-LLM 1.4.0rc2.post1 rechazó su checkpoint NVFP4. Madurez `integrated`, no `proven`. |
| `airllm` | No hay servidor ni adaptación de lanzamiento en el runtime de Ropero. El manifiesto se marca `partial`; los TOML bajo `models/airllm/` no deben interpretarse como una integración operativa. | No instalado localmente y sin prueba de serving. El proyecto AirLLM documenta una biblioteca Python para inferencia por capas; no se encontró en el repo una implementación de servidor/API que Ropero pueda lanzar. |

## Contrato de argumentos del TOML

`levels.list`, `flags.list` y `options.list` anuncian valores de interfaz. Cada entrada debe tener su bloque correspondiente en `[args.level.<id>]`, `[args.flag.<id>]` o `[args.option.<id>]`; el validador rechaza las que no tengan mapeo. Las opciones deben traducir a flags que acepte la CLI del engine concreto. Ropero no convierte automáticamente opciones de un engine a las de otro.

Los flags comunes de arranque (`--port`, clave API y nivel de log) se gestionan en el runtime sólo donde existe una adaptación específica. No se debe asumir que el mismo flag o comportamiento existe en todos los engines.

## Instalar y usar Basalt

`ropero install engines basalt` instala el release Linux x64 precompilado de Basalt en `~/.local/share/ropero/engines/basalt/`, verifica su SHA-256 publicado por GitHub, extrae `bin/`, `serve/`, `tools/`, `data/` y `requirements.txt`, crea `.venv` e instala los requisitos Python. Así se sigue la vía rápida recomendada por Basalt upstream; Ropero no clona ni compila el código fuente. Si no encuentra `libcudart.so.13` y `libcublas.so.13` en el sistema, instala los paquetes CUDA 13 oficiales de NVIDIA dentro de ese entorno virtual y los añade al entorno del proceso al arrancar Basalt. Después enlaza `bin/basalt` en el directorio de binarios de Ropero. El menú de `ropero install engines` ejecuta el mismo flujo al elegir Basalt. Antes, Ropero comprueba CPU AVX2, NVIDIA Blackwell (compute capability 12.0) y driver Linux 580.95.05 o posterior; también prepara Python 3.10+ (con `python3` y `python3-venv` en apt, o `python` en pacman). El driver de NVIDIA debe estar instalado y operativo. Las releases no incluyen CUDA ni el entorno Python. La integración de Ropero sólo admite Linux x86-64, aunque Basalt upstream también publique un archivo para Windows.

Cuando el engine se instala correctamente, Ropero registra el TOML incluido de Qwen3.8-Flash-Next en el catálogo activo del usuario si ese fichero aún no existe. No sobrescribe un TOML existente. El pack del modelo se descarga por separado.

```sh
ropero install engines basalt
ropero install models qwen3.8-flash-next
ropero qwen3.8-flash-next
```

El TOML usa el pack oficial `Qwen3.8-FN-NVFP4-DQ6_K.basalt` (~122 GiB) de Hugging Face. `manual = true` lo deja fuera del menú automático de instalaciones masivas; se instala al nombrarlo explícitamente. La descarga requiere el CLI `hf` autenticado si Hugging Face lo exige. El artefacto se guarda bajo `ARTIFACTS_DIR/basalt` (por defecto `~/models/basalt`). Basalt ofrece APIs OpenAI y Anthropic Messages; Ropero las usa para clientes compatibles con OpenAI y Claude Code, respectivamente. Basalt sólo acepta API keys por argv, por lo que Ropero no las envía y fuerza loopback (`127.0.0.1`).

## Alias del modelo

`[model].name` identifica el modelo dentro de Ropero y debe coincidir con el nombre del TOML. `[model].alias` representa el id público del servidor cuando el engine lo permite. Ropero lo aplica a llama.cpp (`--alias`), Basalt (`--alias` en serving directo), Ignis (`--model-id`), Strata (`aliases` del run config), FLM (tag registrado usado en pull/serve/remove) y MLX-Serve (la entrada del modelo en `~/.mlx-serve/model-settings.json` para safetensors). Para vLLM y TensorRT-LLM se traduce a su opción de nombre servido. Los engines sin un mecanismo documentado de id público conservan el id nativo del modelo.

## Fuentes primarias

- [llama.cpp: servidor HTTP](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md)
- [Turbo-Llama: código del fork](https://github.com/Lorenzo-SF/turbo-llama)
- [FastFlowLM: instrucciones de CLI](https://github.com/ROCm/FastFlowLM/blob/main/docs/docs/instructions/cli.md)
- [Ignis: descarga y configuración del agente](https://github.com/gpillon/ignis/blob/main/docs/user/paths/agent.md)
- [Strata: comandos de terminal](https://github.com/Niko1221/Strata/blob/main/docs/TERMINAL.md)
- [Strata: aliases del run config](https://github.com/Niko1221/Strata/blob/main/serve/runconfig.py)
- [vLLM: `vllm serve`](https://docs.vllm.ai/en/latest/cli/serve/)
- [MLX-Serve: CLI y flags del servidor](https://github.com/ddalcu/mlx-serve/blob/main/docs/cli.md)
- [MLX-Serve: aliases por modelo](https://github.com/ddalcu/mlx-serve/pull/612)
- [MLX-Serve: ajustes indexados por ruta absoluta](https://github.com/ddalcu/mlx-serve/blob/main/src/host/registry/model_settings.zig)
- [MLX-Serve: versión v26.10.1](https://github.com/ddalcu/mlx-serve/discussions/672)
- [TensorRT-LLM: `trtllm-serve`](https://nvidia.github.io/TensorRT-LLM/commands/trtllm-serve/trtllm-serve.html)
- [AirLLM: repositorio oficial](https://github.com/lyogavin/airllm)
- [Basalt: repositorio y quick start](https://github.com/jesdga95/basalt)
- [Basalt: plataformas y releases](https://github.com/jesdga95/basalt/blob/main/docs/platforms.md)
- [Basalt: opciones del servidor y engine](https://github.com/jesdga95/basalt/blob/main/docs/configuration/index.md)
- [Basalt: guía de Qwen3.8-Flash-Next](https://github.com/jesdga95/basalt/blob/main/docs/models/qwen38-flash-next.md)
- [Basalt: lista de packs](https://huggingface.co/jesdga/Qwen3.8-Flash-Next-Basalt)
