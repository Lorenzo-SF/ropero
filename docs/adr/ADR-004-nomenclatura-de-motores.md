# ADR-004 · Nomenclatura de motores

**Estado**: aceptada
**Fecha**: 2026-11-14

## Contexto
La v1 nombraba los motores con ids que ya no describían lo que eran: `llama-server` era el **fork** turboquant del dueño, y el repo **oficial** de llama.cpp se llamaba `llama-server-official`. Los ids se confundían con los binarios (`mlx-serve` era una app, no el motor; `fastflowlm` frente al id corto `flm`), y no había ni color ni puerto por motor. El CLI v2 parte de cero ([`docs-especificacion/ropero-cli.md`](../../docs-especificacion/ropero-cli.md)) y necesita una nomenclatura canónica antes de escribir una sola línea de listados, colores o asignación de puertos.

## Decisión
La nomenclatura canónica de motores pasa a ser la **leyenda del dueño**: `llama-server` (oficial, naranja), `turbo-llama` (fork privado, amarillo), `flm` (rojo), `mlx` (gris), `airllm` (blanco), `tensorrt-llm` (verde pistacho), `vllm` (morado), `strata` (verde pálido). Cada motor tiene un **color** y un **rango de puertos** (`llama-server` 8100-8199, `turbo-llama` 8200-8299, `mlx` 8300-8399, `flm` 8400-8499, `tensorrt-llm` 8500-8599, `vllm` 8600-8699, `airllm` 8700-8799, `strata` 9900-9999).

El cambio semántico crítico es que **`llama-server` pasa a significar el repo oficial de llama.cpp** (antes era el fork), y el fork privado pasa a llamarse **`turbo-llama`** (antes `llama-server-official` era el oficial).

**Alias legacy** (aceptados en la entrada, normalizados a la forma canónica): `llama-server-official` → `llama-server`; `llama-official` → `llama-server`; `llama` → `llama-server`; `turbo` → `turbo-llama`.

## Alternativas consideradas
- Mantener los ids de la v1 — deja `llama-server` significando el fork, que es justo el error que este ADR corrige.
- Usar `llama-server` para el fork y `llama-upstream` para el oficial — descarta el nombre que el dueño usa para el oficial y deja el fork con el nombre del producto genérico.
- Nombrar los motores por su binario (`llama-server`, `mlx-server`, `strata`) — no distingue el fork del oficial, que comparten binario.

## Consecuencias
El catálogo `models/` debe renombrarse: **30 TOML** con `engine = "llama-server"` (fork) pasan a `turbo-llama`; **4 TOML** con `engine = "llama-server-official"` pasan a `llama-server`; los directorios `models/llama/` → `models/turbo-llama/` y `models/llama-official/` → `models/llama-server/`. Los alias legacy ensures que las invocaciones y los TOML de la v1 sigan funcionando tras el renombrado. Cada motor lleva color en todos los listados (`ropero list`, `ropero install engines`) y un rango de puertos propio, de modo que varios motores puedan servir a la vez sin pisarse; el rango de `strata` incluye el `9999` que usan por defecto `groot.sh`, `qwen-abliterated.sh` y `qwen36.sh`.
