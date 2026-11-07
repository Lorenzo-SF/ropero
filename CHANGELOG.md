# Changelog

## Sin publicar

### Añadido
- **El motor STRATA (`engine = "strata"`).** Strata (Niko1221/Strata) no es un
  servidor de inferencia como `llama-server`: es un Terminal que baja un motor
  (el binario que habla con la GPU), deja el modelo en disco y sirve una API
  OpenAI-compatible. Su motor reparte los pesos de un MoE entre VRAM, RAM y SSD
  (lo que no cabe en la memoria se lee del disco: el modo *low-RAM*).
  - Se instala con su instalador oficial (sin root): `ropero install engines
    strata` ejecuta el `install.sh` del release, que deja `strata` en
    `~/.local/bin`. El motor y el modelo NO vienen con el instalador: los baja
    `strata launch` la primera vez, del mismo release y con la firma
    comprobada.
  - Los modelos NO son de ropero: su catálogo es el de Strata (`--family` y
    `--model`). Un TOML con `engine = "strata"` es un puntero, con
    `[model].path = "<familia>/<paquete>"` (`qwen/Q2_0`). Se añaden tres:
    `strata-qwen36-q4` (22,9 GB, el que rinde: 228 t/s medidos en una RTX 5080
    Laptop), `strata-qwen38-q2` (66,4 GB, el 125B cuantizado al mínimo) y
    `strata-gyro-s` (58,5 GB, la variante que ahorra memoria: 27,6 GiB de pesos
    + 28,8 GB de tabla n-gram dentro del fichero).
  - Va **detrás de `endpoint.py`** (backend `strata`), como trtllm y flm: la
    clave la pone el proxy (Strata arranca con `--access local --no-api-key`,
    sólo en 127.0.0.1) y el proxy reescribe el campo `model` al id de ropero.
  - El arranque va por un **puente** (`bin/server/strata-run.sh`), y no por
    `strata launch` a secas: `strata launch --no-tui` arranca el motor y
    DEVUELVE cuando el server ya sirve (el motor sigue vivo, pero lo gestiona
    Strata). El proxy necesita un proceso que no muera mientras el modelo
    sirve y que muera cuando hay que pararlo; si no, el proxy daría el backend
    por caído y el modelo seguiría ocupando la VRAM sin nadie que lo
    supervise. El puente además para **su** preset (`--save-preset
    ropero-<modelo>`), no todos los Strata del usuario: `strata launch --stop`
    a secas pararía también el suyo.
  - `ropero install models <modelo>` NO baja el modelo: en Strata no hay modo
    de "descargar sin arrancar". El hook `install_model` enseña el **plan**
    (qué se bajaría, cuánto y si cabe en esta máquina) y lo dice, en vez de
    fingir que ha bajado nada.
  - Guía: `docs/strata.md`.
- **Modelos para la NPU AMD (FastFlowLM): `models/flm/`.** TOMLs para los
  modelos del catálogo `llama` que FLM sabe servir: `gptoss-flm`
  (gpt-oss:20b), `qwen36-flm` (qwen3.6-moe:35b-a3b), `qwen38-flm`
  (qwen3.8-mtp:27b), `qwen38-dflash2-flm` (qwen3.8-dflash2:27b, el mismo
  modelo con el draft DFlash 2 de z-lab: 1,16x de decode) y
  `nemotron-bolt-flm` (nemotron3.5-bolt:e4b, que NO es el nemotron del
  catálogo sino el Bolt-E4B de 4 GB). También `deepseek-r1-flm`
  (deepseek-r1:8b) y `deepseek-r1-0528-flm` (deepseek-r1-0528:8b). El
  `[model].path` de estos TOMLs es el TAG del almacén de FLM, no una ruta.
  - `ropero install models <modelo-flm>` **no** baja un checkpoint a
    `models/safetensors`: delega en el engine, que usa `flm pull <tag>`
    (FLM tiene su propio almacén, `~/.config/flm/models`, y su propio build
    NPU2). Es el primer engine con su propio "conseguir el modelo"; los
    demás siguen el camino normal (download/build).
  - `ropero doctor` los comprueba contra el almacén de FLM (`flm list`) y, si
    FLM no está instalado, lo dice en vez de afirmar que los modelos faltan.
  - Los contextos NO son los del catálogo: son los que FLM aguanta. Qwen3.6
    con más de 32768 tokens devuelve basura (paged attention) y Qwen3.8 con
    prompts largos también (un passkey se corrompió a 10k y 28k tokens).
- **Modelos para la NPU AMD (FastFlowLM): `models/flm/`.** TOMLs para los
  modelos del catálogo `llama` que FLM sabe servir: `gptoss-flm`
  (gpt-oss:20b), `qwen36-flm` (qwen3.6-moe:35b-a3b), `qwen38-flm`
  (qwen3.8-mtp:27b), `qwen38-dflash2-flm` (qwen3.8-dflash2:27b, el mismo
  modelo con el draft DFlash 2 de z-lab: 1,16x de decode) y
  `nemotron-bolt-flm` (nemotron3.5-bolt:e4b, que NO es el nemotron del
  catálogo sino el Bolt-E4B de 4 GB). También `deepseek-r1-flm`
  (deepseek-r1:8b) y `deepseek-r1-0528-flm` (deepseek-r1-0528:8b). El
  `[model].path` de estos TOMLs es el TAG del almacén de FLM, no una ruta.
  - `ropero install models <modelo-flm>` **no** baja un checkpoint a
    `models/safetensors`: delega en el engine, que usa `flm pull <tag>`
    (FLM tiene su propio almacén, `~/.config/flm/models`, y su propio build
    NPU2). Es el primer engine con su propio "conseguir el modelo"; los
    demás siguen el camino normal (download/build).
  - `ropero doctor` los comprueba contra el almacén de FLM (`flm list`) y, si
    FLM no está instalado, lo dice en vez de afirmar que los modelos faltan.
  - Los contextos NO son los del catálogo: son los que FLM aguanta. Qwen3.6
    con más de 32768 tokens devuelve basura (paged attention) y Qwen3.8 con
    prompts largos también (un passkey se corrompió a 10k y 28k tokens).
- **Un GGUF que ya está en otro modelo ya no se vuelve a bajar.** Si dos
  TOMLs piden el mismo fichero del mismo repo (el caso real: `gptoss` y
  `gptoss-o`, el mismo modelo con nuestro fork y con el llama.cpp de
  upstream), el segundo modelo se **enlaza** al directorio del primero
  (`~/models/gguf/gptoss-o` → `~/models/gguf/gptoss`) en vez de bajarse
  12,7 GB otra vez.
  - La comparación es de **repo + nombre de fichero**, no sólo del nombre:
    `mmproj-BF16.gguf` aparece en repos distintos con contenidos distintos,
    y enlazar uno con otro serviría el modelo equivocado.
  - `--force` no enlaza nada (si quieres el fichero *aquí*, se baja).
  - Un enlace ya hecho (a mano o de antes) se respeta: no se toca.
  - Si el fichero está en disco con el nombre del repo en vez del que espera
    el modelo, no se enlaza un directorio que no serviría: se explica cómo
    renombrarlo (es un rename, no una descarga).
- **`docs/flm-memoria.md`**: cuánto RAM usa cada modelo en la NPU y si se
  puede usar más que el tope de ~15,6 GB. La respuesta: sí, el tope no es de
  FLM (la cadena ni aparece en su código) sino del reparto de RAM que hace
  el BIOS; con la UMA frame buffer del iGPU a 2 GB la NPU ve ~21 GB. La tabla
  trae pesos, coste por token de contexto y total a 8k/16k/32k, medidos de
  la doc de FLM (no estimados).
- **`test/test-endpoint-auth.sh`**: 17 comprobaciones de que el endpoint
  exige la API key con cualquier backend, FLM incluido (FLM no la comprueba:
  la pone el endpoint de delante). Se ejecuta el `authed()` real del fichero
  y se comprueba el cableado: el engine fastflowlm le pasa `--api-key` al
  endpoint y se niega a arrancar sin `LLAMA_API_KEY`.
- **`test/test-models-symlink.sh`**: 16 comprobaciones sobre el symlink
  (se enlaza, es idempotente, `--force` no enlaza, un repo distinto NO
  enlaza, sin fichero en disco se descarga, el nombre equivocado avisa).
- **`bin/diagnostico.sh`**: un solo script que recoge TODO en un fichero
  (ropero, el motor y sus flags de compilación, los procesos vivos con su
  cmdline real, los logs con las marcas clave, los TOML, los ajustes del
  sistema, la GPU, el `setup.sh` entero y el kernel/OOM). No pide sudo, no
  para nada, y cada comando va con `timeout 25`.
- **`tests/bench-motor.sh`**: banco A/B de motores. Mismo modelo, mismo argv
  del catálogo y mismo prompt; lo único que cambia es el binario (vía `PATH`).
  Mide `timings.predicted_per_second` y captura el estado de la GPU con sus
  throttle reasons.

### Fuera de ropero (esto vive en `Lorenzo-SF/setup`)
- El **tuning del sistema** (`swappiness`, THP, min_free, DPM, el cmdline del
  kernel, ananicy, el colchón de RAM) es del repo `setup`, no de ropero:
  está en `setup.sh performance` y se verifica con `setup.sh perf-check`.
  Aquí no queda ni un comando suelto.
- `bin/setup-performance.sh` y `bin/perf-check.sh` se han quitado de ropero
  (duplicaban lo de setup y se podían desincronizar). Lo que ropero sabe
  medir de sí mismo sigue aquí: el banco de motores y el diagnóstico.

### Arreglado
- **`ropero path <modelo-flm>` ya no enseña una ruta inventada.** El `case`
  del path probaba `safetensors/*` antes que `*/fastflowlm`, y como los TOMLs
  de FLM son `format = safetensors`, salía `models/safetensors/gpt-oss:20b`
  como si fuera un directorio. El `[model].path` de FLM es un TAG: se
  imprime tal cual.
- **El nombre del GGUF en disco no era el que esperaba el launcher.** En
  `gptoss` (y `gptoss-o`) el `[download].file` era el nombre del repo
  (`gpt-oss-20b-nvfp4.gguf`) pero `[files].gguf` era otro
  (`gpt-oss-20b-MXFP4_MOE.gguf`), sin `[download].name` que los casara: la
  descarga dejaba el fichero con un nombre que el preflight no busca, y el
  modelo no arrancaba. Ahora `[download].name` lo fija. Lo mismo en
  `nemotron`, donde además el `[download].file` apuntaba a un fichero que el
  repo no publica.
- **El validador del catálogo daba 8 problemas falsos.** Los modelos de
  `models/llama-official/` (el llama.cpp de upstream) declaraban un engine y
  una carpeta que el validador no conocía: 4 avisos por engine y 4 por
  carpeta. Oculto en los menús de install no es desconocido.
- **Los modelos `llama-official` vuelven a estar activos.** `flint-o`,
  `gptoss-o`, `groot-o` y `qwen38-o` estaban en `fired/` (retirados) desde
  que el motor principal pasó a ser nuestro fork, así que no se podían ni
  instalar ni arrancar: justamente los que hacen falta para comparar el
  mismo modelo con el motor de upstream.
- **Un engine que no publica `/v1/models` ya no hace esperar 300 s.** El
  fallback "ropero sabe a quién arrancó en este puerto" estaba **dentro**
  del `if` que comprueba que haya ids: si el engine no publica ids, la
  readiness no se cumplía nunca, ropero se comía el timeout y luego mataba
  a un server sano ("se queda esperando y luego dice que no responde"). El
  fallback ahora está fuera de ese `if`.
- **Dos `ropero` a la vez ya no se pisan.** No había bloqueo por puerto:
  los dos veían el puerto libre y los dos arrancaban, y el loser moría con
  "address already in use" sin decir de quién era el fallo. Ahora hay un
  bloqueo por puerto durante la ventana de arranque ("otro ropero está
  arrancando en :N"), y caduca solo si el ropero que lo tenía muere.
- **`status` distingue "arrancando" de "libre".** Un server que está
  cargando el modelo todavía no escucha: la tabla decía "libre" mientras el
  proceso vivía, y luego "sirviendo" sin explicación.
- **Un arranque fallido ya no deja un zombi.** Si el engine no llegaba a
  estar listo (el modelo tarda en cargar, o peta a mitad), `ropero` avisaba
  "no responde tras Ns" y **dejaba el proceso vivo**: el server seguía
  cargando en segundo plano, terminaba y se ponía a servir peticiones
  ("el server sigue recibiendo llamadas"), con el puerto ocupado. El
  siguiente arranque moría con "puerto ocupado" o "address already in use".
  Ahora, al fallar la espera, se mata el proceso que arrancó ropero (el PID
  del estado, no el que escucha en el puerto: si el puerto lo ocupaba otro,
  ese no es nuestro) y se limpia el estado.
- **El doctor mira el estado de los puertos** (antes no cargaba
  `04-launch.sh`, así que la comprobación entera se saltaba en silencio con
  un "command not found"). Ahora avisa de los cuatro casos en los que el
  estado y la máquina no cuadran: el proceso del estado **vivo pero sin
  escuchar** (cargando o colgado), el **estado huérfano** (el PID ya no
  existe), el **puerto ocupado por alguien que no es de ropero** (no hay
  estado) y el **estado sin sello** (ropero no puede probar que la config
  sea la misma, así que el próximo arranque reinicia y avisa).
- **Un PID reciclado ya no se mata por error.** El estado guarda el PID de
  un arranque; si ese arranque murió, el SO lo recicla y el estado pasa a
  apuntar a un proceso cualquiera. `ropero stop` lo mataba sin mirar. Ahora
  se comprueba que `/proc/<pid>/cmdline` mencione el binario del engine, y
  si no, se avisa y no se toca.
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
