# endpoints/server.py — endpoint OpenAI de ropero (airllm, tensorrt-llm, fastflowlm)

Un script que sirve la API OpenAI-compatible (`/v1/models`, `/v1/chat/completions`,
`/v1/completions`, `/health`) por encima de los tres motores que no traen endpoint
con auth: **airllm** (biblioteca, no binario), **tensorrt-llm** (`trtllm-serve` no
tiene `--api_key`) y **fastflowlm** (`flm serve` no comprueba la clave). Antes lo
tapaba un proxy aparte; aquí los tres comparten endpoint, contrato y auth.
```
python3 endpoints/server.py --backend <airllm|tensorrt|flm> \
    --model-path <ruta|tag> --model-id <nombre-ropero> \
    --host <host> --port <puerto> \
    [--api-key <clave>] [--context <tokens>] [--max-seq-len <tokens>] \
    [--backend-port <puerto>] [--extra-arg <arg>]...
```
`--model-id` es lo que anuncia `/v1/models` (`owned_by: "ropero"`): es lo que hace
que `ropero stop <modelo>` y la columna MODELO del status funcionen. `--model-path`
es una ruta (airllm, tensorrt) o un **TAG** del almacén de FLM (p.ej. `llama3.2:1b`).
Sin `--api-key` el endpoint queda abierto: lo avisa.

**Auth**: con `--api-key`, todas las rutas menos `/health` exigen `Authorization:
Bearer <clave>` y responden 401 con el formato de OpenAI. El backend SIEMPRE
escucha en `127.0.0.1` (`--backend-port`, por defecto el externo + 1000): el único
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
