# Changelog

## Sin publicar

### Añadido
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
- **Los checkpoints que YA vienen cuantizados de fábrica (NVFP4, MXFP4) ya
  no son un callejón sin salida.** Antes el gguffer abortaba con "el repo
  YA VIENE CUANTIZADO" para cualquier repo pre-cuantizado, **incluso sin
  `--quant`**: no había forma de convertir y colocar un NVFP4 sin que se
  quejara. Y el error, cuando salía, salía después de descargar 40-60 GB.
  - Ahora se mira la metadata del repo (`config.json` →
    `quantization_config`, y `hf_quant_config.json` → `quantization.quant_algo`,
    que va **anidada**) y se distingue lo que el conversor **conserva**
    (NVFP4, MXFP4) de lo que **dequantiza** (FP8, AWQ, GPTQ → F16):
    - sin `--quant` → se convierte y se coloca (el GGUF sale en el quant que
      ya traían);
    - con `--quant` del **mismo** tipo → se convierte y se coloca, **sin
      cuantizar otra vez** (y sin pedir imatrix);
    - con `--quant` **distinto** → error claro, **antes de descargar**, con
      el repo hermano sin cuantizar que sí serviría;
    - los que se dequantizan (FP8/AWQ/GPTQ) sí se pueden cuantizar, pero
      avisando de que el quant de fábrica se pierde por el camino.
- **El `general.file_type` se comparaba en el espacio de números
  equivocado.** El `file_type` del GGUF es un **llama_ftype** (el enum de
  `include/llama.h`: NVFP4 = 39, MXFP4_MOE = 38, Q4_K_M = 15), y es
  JUSTO el espacio de los números que imprime `llama-quantize --help`. Pero
  el atajo de "el GGUF ya está en ese quant" comparaba ese número con el
  **ggml_type** del tensor predominante (otro enum: NVFP4 = 40, MXFP4 = 39):
  nunca coincidía, así que **el atajo nunca se disparaba**. Y el aviso que
  comparaba un *nombre* con un *número* salía siempre.
  - Nuevo modo `--file-type` en `bin/python/ggufcheck.py` (imprime el
    `general.file_type`), y el gguffer ya compara en el espacio correcto.
  - `bin/quant/tipos.txt` estaba en espacio **ggml_type** cuando todos sus
    consumidores (el model card, `subir-hf.sh`, `verificar-gguf.sh`) leen el
    `general.file_type`, que es **llama_ftype**: por eso un GGUF en NVFP4 se
    llamaba "MXFP4" y un Q4_K_M se llamaba "Q8_K". La lista está ahora en
    llama_ftype, con los alias ("Q4_K" es alias de "Q4_K_M") detrás del
    canónico, y el nombre de un número se busca en la lista sin alias.
- **La procedencia (`.fuente.json`) decía cosas que no eran ciertas.** Se
  leía el `file_type`, el `base_model` y el `base_model_name` del modo
  `--meta` de `ggufcheck.py`, que **solo imprime** `arch`, `block_count`,
  `nextn_predict_layers` y `bloque_mtp`: esos tres campos salían siempre
  vacíos y el `quant` caía al nombre del fichero. Ahora se leen del `--info`
  (JSON) y el `quant` se guarda con el **nombre canónico** del quant.
- **El intermedio ya no se llama `-F16` cuando no lleva un F16.** Con pesos
  que vienen en NVFP4, el GGUF intermedio es un NVFP4, y llamarlo `-F16`
  hacía que el reuso del intermedio (y el nombre final) mintieran.


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
- **`--quant` y el imatrix son una pareja: uno sin el otro no se acepta.**
  - `--imatrix` (fichero local) es **excluyente** con el par
    `--imatrix-repo`/`--imatrix-file` (el imatrix de otro repo), y los dos
    últimos **van siempre juntos**.
  - **Cuantizar exige imatrix**: sin él el gguffer dice que falta y **no
    baja los pesos** (el error sale antes de los 60 GB, no después). Con el
    imatrix ya puesto no hace falta el `--imatrix` local.
  - Un imatrix **sin** `--quant` es un error ("no hay nada que cuantizar"),
    salvo con `--diagnostico`, que sí lo usa.
  - Y al revés: si el GGUF **ya está** en el quant pedido, no se cuantiza
    (y por tanto **no hace falta imatrix, y no se baja**: son 200 MB).
- El imatrix de `--imatrix-repo` **solo se baja si de verdad se va a
  cuantizar**.

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
