# Changelog

## Sin publicar

### Añadido
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

### Arreglado
- **Los checkpoints "mixed-precision" (NVFP4 + FP8 en el mismo repo) ya se
  convierten.** El conversor del motor decidia si el checkpoint era NVFP4
  con un `all(...)` sobre los grupos de `config_groups`: con precision
  mixta (un grupo NVFP4 y otro FP8, **el caso del Groot**) el `all` daba
  falso y la conversion reventaba con `NotImplementedError: Can't handle
  multiple config groups for compressed-tensors yet`. Upstream usa `any(...)`
  y ademas dequantiza el grupo FP8 con un `dequant_fp8` anidado que el fork
  no tenia.
  - El motor (`Lorenzo-SF/ropero-llama-turboquant`, commit `009b154`) ya lo
    trae: un `ropero install engines` nuevo lo clona con el fix.
  - Y el parche se entrega tambien como fichero
    (`bin/quant/patches/parche-motor-compressed-tensors-mixed-precision.patch`),
    para los clones del motor que ya tenias: `ropero/quant/aplicar-parche-motor.sh`
    lo pone y recompila.

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
