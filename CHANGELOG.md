# Changelog

## Sin publicar

### Arreglado
- **El contexto del modelo ya llega al motor.** `[model].context` (o
  `ROPERO_<NOMBRE>_CTX`) estaba en el catálogo, en el dump y en `ropero
  config`, y **nunca se pasaba**: llama-server arrancaba con el contexto
  **nativo** del modelo (262144 en flint, 266144 en nemotron). Tres
  consecuencias, todas medidas:
  - la KV cache se comía la VRAM (2,3 GiB en flint) y no quedaba sitio para
    los expertos ni para la moe-cache → **flint abortaba al arrancar**;
  - los expertos se quedaban en la CPU → el modelo iba a **la mitad de
    velocidad** (nemotron: 60 t/s en vez de 100-120);
  - el contexto no estaba en el argv, así que tampoco estaba en el sello de
    configuración → cambiar `context =` en el TOML **no reiniciaba nada** y el
    servidor viejo seguía sirviendo con el contexto viejo.
  El `-c` va antes de los args del TOML: si el TOML declara su propio `-c`,
  ese gana.
- **El puerto se detecta sin `lsof`.** Todo el arranque (el check de "puerto
  ocupado", el "puerto del sello", el `stop`) preguntaba a `lsof`, que no
  está en todas las máquinas. **Sin `lsof` esas preguntas devolvían siempre
  "libre"**: ropero arrancaba un servidor encima del que ya estaba (que
  moría con *address already in use*), el `stop` no mataba nada y el "puerto
  ocupado" nunca saltaba. Ahora se pregunta a `lsof`, y si no está, a `ss`,
  a `fuser` y, como último recurso, a `/proc/net/tcp` (+ `/proc/*/fd` para
  el PID), que siempre está en Linux.
- **El `stop` mata al servidor que se está cargando.** Antes, si el puerto
  no estaba en LISTEN, `rp_stop_port` borraba el estado y **no mataba
  nada**: un `stop` mientras el modelo se cargaba lo dejaba vivo, y cuando
  terminaba de cargar se ponía a servir peticiones ("el servidor sigue
  recibiendo llamadas"). Ahora se mata al PID del estado aunque el puerto no
  escuche todavía, y la espera mira a los PIDs y no sólo al puerto.
- **`stop <modelo>` encuentra al modelo aunque no esté sirviendo todavía:**
  se busca por el estado (`MODEL`) y no sólo por lo que responde el puerto.
- **El Ctrl-C en primer plano mata al servidor sin `lsof`.** El trap del
  launcher llama a `rp_stop_port`, que sin `lsof` no encontraba a nadie.
  Además la espera en primer plano ya no es un `wait` sino un bucle con
  `kill -0`: el engine arranca con `nohup ... &`, y bash pone SIGINT y
  SIGQUIT en `SIG_IGN` en los comandos en background de un shell sin job
  control, disposición que hereda el `exec` (medido: `SigIgn 0x7`), así que
  llama-server **ignora el SIGINT para siempre** y no puede morir con él.
- **Un estado sin sello de configuración ya no cuenta como "la misma
  config"**: antes ropero respondía "'x' ya está corriendo" y se callaba,
  dejando la config vieja para siempre. Ahora, sin sello no se puede probar
  que la config sea la misma → se reinicia y se avisa.

### Cambiado
- El motor principal pasa de `AtomicBot-ai/atomic-llama-cpp-turboquant` a
  **nuestro motor**, `Lorenzo-SF/ropero-llama-turboquant`, que es **privado**:
  un fork de un repo público no puede ser privado (GitHub responde 422
  *"Public forks can't be made private"*), así que este repo es independiente
  del fork, con el mismo contenido y la misma rama `dev`. Además de TurboQuant
  y MTP/NextN trae:
  - **turbo5/turbo6** (los tipos de KV `TQ5_0`/`TQ6_0`), tomados del motor de
    TheTom;
  - **flint** (la arquitectura nueva de upstream, PR #30928): el `models/llama/flint.toml`
    ya no necesita el motor oficial;
  - **los parches de ropero horneados en el motor** (la regla del NVFP4 y los
    patterns sin anclar): `bin/quant/aplicar-parche-motor.sh` los detecta y no
    hace nada, pero sigue siendo el que los aplica si el clon es de otro
    dueño (el oficial).
- Como el motor es privado, su clone necesita credenciales. Se prueban dos
  vías: **SSH** (la clave vive en `~/.ssh`) y **HTTPS con el token del
  entorno** (`ROPERO_GITHUB_TOKEN` o `GITHUB_TOKEN`), pasado como credential
  helper de un solo comando — **el token no se escribe en ningún fichero**
  (ni en el `.git/config` del clon). Si no hay ninguna de las dos, se avisa
  con las dos instrucciones y se cae al fork público (mismo contenido), que se
  clona sin credenciales.
- Engine `llama-server-official` **oculto**: no se anuncia, no se instala con
  `install engines` y el doctor no lo comprueba. Se puede instalar a mano
  (`install engines llama-official`). Los cuatro modelos que lo usaban
  (`flint`, `gptoss`, `groot`, `qwen38`) pasan a
  `models/llama-official/fired/`: ahora los arranca el motor principal.

## 1.0.0 — 2026-10-30

Refactor completo del monolito a módulos. Mismo producto, una sola fuente de
verdad por pieza.

### Añadido
- `ropero test <modelo>`: baterías `prueba1..prueba_gorda` con opencode,
  confirmación previa y markdown autocontenido (juez + prueba + log + JSON +
  sesión).
- `ropero status <modelo>` (exit 0/1/2), `ropero doctor` (alias `check`),
  `ropero --version`.
- `install engines` / `install models` con selección interactiva; sin TTY
  exigen selección explícita (nunca instalan todo por accidente).
- `[download.aux.*]` para mtp, mmproj, chat_template, imatrix, tokenizer y
  draft; `[build]` integrado con `ropero gguffer`.
- Engines `mlx-serve` (macOS) y `vllm`; los motores sin server propio
  (airllm, TensorRT-LLM, FastFlowLM) van por `bin/server/endpoint.py`.
- TOMLs de `metalnemotron` y `metalqwen`.
- Suite offline `tests/suite/run.sh` con fakes (sin red ni GPU).

### Cambiado
- `bin/ropero` es un dispatcher fino; la lógica vive en `bin/lib/`.
- Un motor = `bin/lib/engines/<id>.sh` con interfaz de 8 hooks.
- `quant/` → `bin/quant/`; `endpoints/server.py` → `bin/server/endpoint.py`.
- `install.sh` sólo obtiene ropero (tarball/clone HTTPS, `--version`, `--dir`,
  symlink); las dependencias son `ropero install setup`.
- Clone del motor por HTTPS; API key de mlx-serve vía `--api-key-env` (nunca
  en argv); `--cpu` sólo en Linux.
- Objetivo de sintaxis bash 3.2 (sin namerefs ni arrays asociativos).

### Corregido
- `ropero gguffer` ya no trata como error una cuantización ya aplicada en el
  repo: coloca el GGUF y sale 0 sin borrar nada.
- `ropero_install_mlx` con la lógica invertida; choques alias↔nombre;
  `status` sin dependencia obligatoria de `alaja`.
- Código muerto eliminado (`_authproxy.py`, hooks sin llamada, iGPU).

### Eliminado
- Tests artesanales 0.x (`test-*.sh`, `bench_*`, `probar-*`), `ropero.d/`,
  `prueba.json` y el monolito legado.
