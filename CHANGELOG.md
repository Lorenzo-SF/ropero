# Changelog

## Sin publicar

### Añadido
- **`tests/bench-motor.sh`**: banco A/B de motores. El mismo modelo, el mismo
  argv del catálogo y el mismo prompt, cambiando **sólo el binario** de
  llama-server (se resuelve por `PATH`, así que la comparación es limpia por
  construcción). Mide el decode con los timings que devuelve el propio server
  (`timings.predicted_per_second`), repite rondas para ver si el rendimiento
  **se degrada con el tiempo en uso**, y captura el estado de la GPU en cada
  ronda (relojes, potencia, temperatura y **razones de throttle**). Es la
  herramienta para responder "¿el merge me quitó rendimiento?" sin meter a
  opencode ni a un juez en el medio.
- **`bin/perf-check.sh`**: el estado REAL de los ajustes de rendimiento de la
  máquina, con el valor que importa de cada uno (no si el fichero existe):
  limits.d, sysctl, min_free_kbytes, THP, read-ahead del NVMe, DPM de NVIDIA,
  persistencia y dónde vive el cmdline del kernel (systemd-boot o GRUB). A
 parte, la memoria (si el GGUF cabe en el page cache, si hay swap) y la GPU
  con sus razones de throttle y su techo de potencia. Es lo que hay que
  mirar **después de un rato de trabajo** cuando el rendimiento baja.

### Arreglado
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
