# ropero

CLI para usar LLMs locales con el motor que mejor rinda en cada máquina:
**llama-server** (GGUF), **mlx-serve** (Apple Silicon), **vLLM**, y
**airllm / TensorRT-LLM / FastFlowLM** a través de un endpoint OpenAI común.

Funciona en **macOS (Apple Silicon)** y **CachyOS/Arch Linux**. La
configuración de cada modelo es un TOML: si el motor cambia, cambia el dato;
el código no se toca.

## Instalación

```bash
curl -fsSL https://raw.githubusercontent.com/Lorenzo-SF/ropero/main/install.sh | bash
```

El instalador **sólo obtiene ropero** y crea `~/.local/bin/ropero`. Después:

```bash
ropero install setup      # dependencias base (python3, git, uv, hf, jq, cmake…)
ropero install engines     # elige motores e instala SUS dependencias
ropero install models      # elige modelos (descarga o fabrica el GGUF)
```

`ropero install` sin argumentos hace las tres cosas en orden.

## Uso diario

```bash
ropero list                     # modelos activos (--all incluye retirados)
ropero nemotron                 # arranca en primer plano (Ctrl-C lo para)
ropero nemotron --background    # arranca y devuelve ROPERO_PORT=<n>
ropero use nemotron             # idempotente: el que debe estar, está
ropero use nemotron --nivel high
ropero status                   # puertos gestionados y estado real
ropero status nemotron          # 0 sirviéndose · 1 no · 2 desconocido
ropero stop                     # para todo
ropero stop nemotron

ropero args nemotron --high     # argv final, un argumento por línea
ropero levels nemotron
ropero path nemotron            # GGUF, binario, log
ropero doctor                   # chequeo de la herramienta (alias: check)
```

Puertos: GPU `:9999`, CPU `:9998` (sólo Linux). `--cpu` sirve en el slot CPU;
`--port N` manda sobre todo.

## Modelos (TOML)

Un modelo es un fichero en `models/<engine>/<nombre>.toml`. Los activos están
directamente en el directorio del motor; los retirados, en
`models/<engine>/fired/` (informativos: no se instalan ni arrancan).

- `[download]`: repo/file de HF (GGUF o safetensors).
- `[download.aux.*]`: mtp, mmproj, chat_template, imatrix, tokenizer, draft,
  cada uno con su repo opcional.
- `[build]`: receta de fabricación del GGUF (la misma que `ropero gguffer`).
- `[args]`: argv del motor; `[levels]`, `[flags]`, `[options]`: lo que el
  usuario puede pedir por CLI sin editar el TOML.
- `[limits]`: RAM mínima orientativa.

Si un TOML tiene `[download]` y `[build]`, `ropero install models <modelo>`
exige `--download` o `--build`; en el menú interactivo se pregunta.
Plantilla completa y documentada: `ropero template`.

## Probar un modelo

```bash
ropero test nemotron                 # prueba1, confirmación previa
ropero test nemotron --todas         # las cinco baterías
ropero test nemotron --prueba prueba_gorda --yes
```

Deja en el directorio actual un `ropero-test-<modelo>-<prueba>-<fecha>.md`
autocontenido (prompt del juez + prueba + log del motor + JSON de opencode +
sesión exportada). **Aviso**: el test para el entorno y mata
`llama-server/opencode/mcode/claude` (`pkill -9`) tras confirmación.

## Fabricar GGUFs

```bash
ropero gguffer --repo unsloth/Qwen3.8-27B-GGUF --quant IQ3_XXS
ropero gguffer --gguf nvidia/Qwen3.8-27B-NVFP4 --quant NVFP4   # ya aplicada: sólo coloca
ropero gguffer --list-quants
```

El pipeline completo (cuantizar, verificar, subir a HF) vive en
`bin/quant/`; `ropero gguffer` es la puerta de entrada y la misma
implementación que usa `[build]`.

## Integración con opencode

```bash
ropero install opencode-plugin
```

Copia el plugin a `~/.config/opencode/plugins/ropero-modelo/`: arranca el
modelo que pides vía `ropero use` y acota la petición de título de sesión.
Contrato estable: `list` (dos espacios + nombre), `levels` (uno por línea) y
`use` (última línea `ROPERO_PORT=<n>`).

## Estructura

```
bin/ropero              dispatcher (~200 líneas)
bin/lib/*.sh            módulos: utils, env, modelconf, engines, launch, cmd-*
bin/lib/engines/*.sh    un motor = un fichero (8 hooks)
bin/python/             modelconf, ggufcheck, model card
bin/server/endpoint.py  OpenAI-compatible para motores sin server propio
bin/quant/              pipeline de cuantización y parches del motor
models/                 TOMLs (activos + fired/)
tests/                  pruebas de modelos + suite offline (tests/suite/run.sh)
docs/specs/             contrato del refactor 1.0
```

Añadir un motor = añadir `bin/lib/engines/<id>.sh` (y, si tiene modelos,
`models/<dir>/`). Nada más.

## Variables

`GGUF_DIR` (`~/models/gguf`), `SAFETENSORS_DIR` (`~/models/safetensors`),
`LOG_DIR`, `STATE_DIR`, `HOST` (`127.0.0.1`), `PORT` (`9999`),
`CPU_PORT` (`9998`), `LLAMA_API_KEY` (se genera una si no existe),
`ROPERO_START_TIMEOUT` (`300`), `HF_TOKEN`.

## Desarrollo

```bash
tests/suite/run.sh        # suite offline (sin red, sin GPU, con fakes)
bash -n bin/ropero bin/lib/*.sh bin/lib/engines/*.sh
shellcheck -S warning bin/ropero bin/lib/*.sh bin/lib/engines/*.sh
```

Requisito de sintaxis: **bash 3.2** (el `/bin/bash` de macOS): sin `nameref`,
sin `declare -A`, sin `mapfile`.
