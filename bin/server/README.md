# bin/server/endpoint.py — endpoint OpenAI de ropero (airllm, tensorrt-llm, fastflowlm, strata)

Un script que sirve la API OpenAI-compatible (`/v1/models`, `/v1/chat/completions`,
`/v1/completions`, `/health`) por encima de los motores que no traen endpoint
con auth: **airllm** (biblioteca, no binario), **tensorrt-llm** (`trtllm-serve` no
tiene `--api_key`), **fastflowlm** (`flm serve` no comprueba la clave) y **strata**
(el server de Strata también acepta la clave, pero la pone el proxy: Strata
arranca con `--access local --no-api-key`, sólo en 127.0.0.1). Antes lo tapaba un
proxy aparte; aquí comparten endpoint, contrato y auth.
```
python3 bin/server/endpoint.py --backend <airllm|tensorrt|flm|strata> \
    --model-path <ruta|tag|familia/paquete> --model-id <nombre-ropero> \
    --host <host> --port <puerto> \
    [--api-key <clave>] [--context <tokens>] [--max-seq-len <tokens>] \
    [--backend-port <puerto>] [--extra-arg <arg>]...
```
`--model-id` es lo que anuncia `/v1/models` (`owned_by: "ropero"`): es lo que hace
que `ropero stop <modelo>` y la columna MODELO del status funcionen. `--model-path`
es una ruta (airllm, tensorrt), un **TAG** del almacén de FLM (p.ej. `llama3.2:1b`)
o el par **familia/paquete** del catálogo de Strata (p.ej. `qwen/Q2_0`).
Sin `--api-key` el endpoint queda abierto: lo avisa.

**Auth**: con `--api-key`, todas las rutas menos `/health` exigen `Authorization:
Bearer <clave>` y responden 401 con el formato de OpenAI. El backend SIEMPRE
escucha en `0.0.0.0.1` (`--backend-port`, por defecto el externo + 1000): el único
proceso expuesto es este, y muere con él (SIGTERM/SIGINT matan al sub-proceso y
liberan los dos puertos).

**El `model` del body se reescribe** al nombre que el backend conoce: el id de
ropero para tensorrt, el TAG para flm (FLM valida el campo: su error es `Model X is
not supported.`). Así el cliente puede mandar cualquier nombre.

Mientras el modelo carga: `503 {"error":{"message":"model loading"}}`; si el backend
se muere, `503` con el motivo. `stream: true` emite SSE y termina con `data: [DONE]`.

Depende de `fastapi` + `uvicorn` (y `httpx` sólo para el proxy; si no está, usa
`urllib.request` de la stdlib). El backend `airllm` necesita además la biblioteca
`airllm` en el mismo intérprete.
