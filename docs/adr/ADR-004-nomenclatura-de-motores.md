# ADR-004 · Nomenclatura de motores

**Estado**: aceptada
**Fecha**: 2026-11-14

## Contexto
La v1 nombraba los motores con ids que ya no describían lo que eran: `llama-server` era el **fork** turboquant del dueño, y el repo **oficial** de llama.cpp se llamaba `llama-server-official`. Los ids se confundían con los binarios (`mlx-serve` era una app, no el motor; `fastflowlm` frente al id corto `flm`), y no había ni color ni puerto por motor. El CLI v2 parte de cero ([`docs-especificacion/ropero-cli.md`](../../docs-especificacion/ropero-cli.md)) y necesita una nomenclatura canónica antes de escribir una sola línea de listados, colores o asignación de puertos.

## Decisión
La nomenclatura canónica de motores pasa a ser la **leyenda del dueño**: `llama-server` (oficial, naranja), `turbo-llama` (fork privado, amarillo), `flm` (rojo), `mlx` (gris), `airllm` (blanco), `tensorrt-llm` (verde pistacho), `vllm` (morado), `strata` (verde pálido). Los rangos siguen las bases del ropero viejo en bloques de diez: `llama-server` 9980-9989, `turbo-llama` 9990-9999, `fastflowlm` 9970-9979, `mlx-serve` 9960-9969, `tensorrt-llm` 9950-9959, `vllm` 9940-9949, `airllm` 9930-9939 y `strata` 9920-9929.

**Aclaración (2026-11-17):** el id canónico de FastFlowLM es `fastflowllm`, que
refleja la grafía del nombre del engine. `fastflowlm` y `flm` se conservan como
aliases de compatibilidad para configuraciones y catálogos existentes.

En la integración de Ignis se amplía la lista con `ignis` (cian), Linux x86_64
con NVIDIA RTX, puerto `9910-9919` y artefactos `.ninfer`. Ignis usa el
protocolo OpenAI y su propio descargador de modelos y companions.

El cambio semántico crítico es que **`llama-server` pasa a significar el repo oficial de llama.cpp** (antes era el fork), y el fork privado pasa a llamarse **`turbo-llama`** (antes `llama-server-official` era el oficial).

**Alias legacy** (aceptados en la entrada, normalizados a la forma canónica): `llama-server-official` → `llama-server`; `llama-official` → `llama-server`; `llama` → `llama-server`; `turbo` → `turbo-llama`.

## Alternativas consideradas
- Mantener los ids de la v1 — deja `llama-server` significando el fork, que es justo el error que este ADR corrige.
- Usar `llama-server` para el fork y `llama-upstream` para el oficial — descarta el nombre que el dueño usa para el oficial y deja el fork con el nombre del producto genérico.
- Nombrar los motores por su binario (`llama-server`, `mlx-server`, `strata`) — no distingue el fork del oficial, que comparten binario.

## Consecuencias
El catálogo `models/` debe renombrarse: **30 TOML** con `engine = "llama-server"` (fork) pasan a `turbo-llama`; **4 TOML** con `engine = "llama-server-official"` pasan a `llama-server`; los directorios `models/llama/` → `models/turbo-llama/` y `models/llama-official/` → `models/llama-server/`. Los alias legacy ensures que las invocaciones y los TOML de la v1 sigan funcionando tras el renombrado. Cada motor lleva color en todos los listados (`ropero list`, `ropero install engines`) y un rango propio derivado del puerto base que usaba el ropero viejo.
