# Compatibilidad de engines

[English](engine-support.en.md) · [Español](engine-support.es.md)

Esta guía describe lo que Ropero implementa hoy y separa las comprobaciones locales de la documentación del proveedor. Que un motor exista en un manifiesto o que un formato aparezca en su catálogo no demuestra que su arranque funcione.

## Evidencia local

En el entorno donde se revisó el checkout se encontraron `llama-server`, `turbo-llama-server`, `flm`, `ignis-server` y `strata`; se consultó su versión o ayuda. No se encontraron `vllm`, `mlx-serve`, `trtllm-serve` ni `airllm`, por lo que esos cuatro no se probaron localmente. Los resultados describen ese equipo y esas versiones, no todas las instalaciones de Ropero.

## Motores y límites conocidos

| Engine | Qué hace Ropero | Estado de evidencia y límites |
|---|---|---|
| `llama-server` | Lanza el binario con los argumentos del TOML, añade el GGUF cuando hace falta y fija `--alias` al alias público o al nombre Ropero. | Binario y ayuda comprobados localmente. La documentación oficial describe `-m/--model`, `--alias`, `--host` y `--port`. |
| `turbo-llama` | Usa el fork `turbo-llama-server` con la misma ruta de argumentos GGUF y alias que `llama-server`. | Binario y ayuda del fork comprobados localmente. La documentación de llama.cpp no garantiza que cada opción exista en el fork; hay que verificarla con la ayuda de la versión instalada. |
| `fastflowllm` / FLM | Delega `pull`, `remove`, `list` y `serve` a `flm`. El tag registrado es `[model].alias` si está definido; si no, procede de `[model].path` o de `--decision` en `[args].base` cuando `path` está vacío. Ropero no descarga ese artefacto en su propio directorio. | `flm` local comprobado. El CLI oficial usa tags, con `flm pull <tag>`, `flm serve <tag>` y `flm remove <tag>`. Formato, NPU y plataforma dependen de FastFlowLM/XRT. |
| `ignis` | Para `ninfer`, ejecuta `ignis-server model download <id> --out <ARTIFACTS_DIR>/<name>` y sirve el artefacto desde esa carpeta. Al arrancar usa `--model-id <alias>` cuando el TOML declara `alias`. | `ignis-server` local comprobado. Ignis documenta la descarga del catálogo con `--out` y el id que publica en la API. El runtime de Ropero sólo reconoce los ids/artifacts declarados para los modelos Ignis incluidos; no implica compatibilidad con cualquier modelo. |
| `strata` | Busca el perfil instalado con `strata models list --installed --output json`, lo resuelve desde `[model].path` y lanza `strata serve --profile …`. Añade las opciones y flags del modelo al comando. Si existe `[model].alias`, crea una copia privada del run config con ese id en `aliases`, sin modificar el perfil fuente. | Binario y `strata serve --help` comprobados localmente. La CLI oficial documenta los controles de serving y el run config permite aliases públicos separados del `model_name`. Ropero no descarga ni crea el perfil Strata. |
| `vllm` | El runtime puede pasar argumentos de modelo y ajusta `--log-level`; el binario esperado es `vllm`. | No instalado localmente: no se probó un arranque real. La documentación oficial documenta `vllm serve` y sus opciones, pero el catálogo actual no contiene modelos vLLM de ejemplo con una receta integral verificada. Tratar como integrado/documentado, no como probado. |
| `mlx-serve` | El runtime puede pasar argumentos del modelo y `--log-level`. Para modelos safetensors con `[model].alias`, escribe el alias en la entrada del modelo de `~/.mlx-serve/model-settings.json` y conserva el resto de ajustes y entradas. | No instalado localmente: no se probó un arranque real. Upstream documenta los aliases en la versión v26.10.1; se necesita esa versión o una posterior. El proyecto requiere macOS y Apple Silicon. Implementado según la documentación upstream, no probado localmente. |
| `tensorrt-llm` | El binario esperado es `trtllm-serve`; los argumentos del modelo vienen del TOML. | No instalado localmente: no se probó un arranque real. La documentación de NVIDIA describe `trtllm-serve` y sus subcomandos/configuración; la compatibilidad depende de versión, GPU, runtime y formato. El catálogo no demuestra por sí solo que cada formato declarado tenga una ruta de build/serving funcional. |
| `airllm` | No hay servidor ni adaptación de lanzamiento en el runtime de Ropero. El manifiesto se marca `partial`; los TOML bajo `models/airllm/` no deben interpretarse como una integración operativa. | No instalado localmente y sin prueba de serving. El proyecto AirLLM documenta una biblioteca Python para inferencia por capas; no se encontró en el repo una implementación de servidor/API que Ropero pueda lanzar. |

## Contrato de argumentos del TOML

`levels.list`, `flags.list` y `options.list` anuncian valores de interfaz. Cada entrada debe tener su bloque correspondiente en `[args.level.<id>]`, `[args.flag.<id>]` o `[args.option.<id>]`; el validador rechaza las que no tengan mapeo. Las opciones deben traducir a flags que acepte la CLI del engine concreto. Ropero no convierte automáticamente opciones de un engine a las de otro.

Los flags comunes de arranque (`--port`, clave API y nivel de log) se gestionan en el runtime sólo donde existe una adaptación específica. No se debe asumir que el mismo flag o comportamiento existe en todos los engines.

## Alias del modelo

`[model].name` identifica el modelo dentro de Ropero y debe coincidir con el nombre del TOML. `[model].alias` representa el id público del servidor cuando el engine lo permite. Ropero lo aplica a llama.cpp (`--alias`), Ignis (`--model-id`), Strata (`aliases` del run config), FLM (tag registrado usado en pull/serve/remove) y MLX-Serve (la entrada del modelo en `~/.mlx-serve/model-settings.json` para safetensors). Para vLLM y TensorRT-LLM se traduce a su opción de nombre servido. Los engines sin un mecanismo documentado de id público conservan el id nativo del modelo.

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
