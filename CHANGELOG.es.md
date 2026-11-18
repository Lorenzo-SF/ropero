# Changelog

[English](CHANGELOG.md) · **Español**

## Unreleased — 2026-11-18

### Español

- `status` presenta CPU/GPU, RAM/SWP/zram en un diseño compacto y lista los
  modelos activos de Ropero; `stop` lee sólo el estado de procesos necesario.
- Los colores de terminal usan RGB truecolor derivados de la paleta hexadecimal.
  La tabla de engines ya no incluye una columna con el nombre del color.
- Strata busca primero el run config exacto y consulta sólo los perfiles
  referenciados por el catálogo para conocer su disponibilidad.
- El asistente de `models add` permite elegir engines instalados, configurar
  primero los args comunes, previsualizar el TOML normalizado y editar un bloque
  o cancelar. Admite rutas de build, repositorios y varios ficheros auxiliares.
- La plantilla del modelo es más breve y deja `[args].base` vacío.
- `ropero test` acepta `--model` y exactamente uno de `--test` o
  `--test-group`, además de `--path`. El modo interactivo ofrece modelos,
  grupos y tests disponibles; cada ejecución crea un directorio Markdown y
  detiene el servidor que haya iniciado.

- La validación compara el formato de cada modelo con el manifiesto del engine;
  MLX recibe su ruta resuelta y queda enlazado a loopback por defecto.
- Las etiquetas de madurez de MLX y TensorRT reflejan la evidencia registrada.
  Se retira el checkpoint TensorRT NVFP4 rechazado por la versión probada.
- Strata admite el formato `strata` respaldado por perfiles; sus TOML y la guía
  describen los ids que devuelve el CLI upstream.
- El manifiesto de instalación de FLM usa claves compatibles con TOML 1.0.

- Se completan `models add`, `info` y `command`; `models` sustituye a `list`.
- `config --lang ES|EN` guarda la preferencia de idioma. FLM usa el id
  canónico `fastflowllm`, y los modelos Ignis separan nombre interno y alias
  público.
- `connect` usa `--remove`; `--list` enumera sólo integraciones implementadas.
- `connect` muestra instrucciones manuales para TRAE y TRAE CN; OpenCode conserva
  comentarios y comas finales al editar `opencode.jsonc` y deja intacto el
  archivo si el JSONC es inválido.
- El arranque acepta niveles, flags y opciones declarados por cada TOML.
- Codex guarda el provider y el perfil por engine en `CODEX_HOME` (o
  `~/.codex`) y rechaza engines sin Responses API documentada. llama.cpp recibe
  el alias público mediante `--alias`.
- `ropero test` usa `--model`, `--test`, `--test-group` y `--path`.
- Ignis guarda y busca cada descarga bajo `ARTIFACTS_DIR/<nombre-interno>`;
  Codex valida las tablas TOML opcionales sin accesos que puedan provocar panic.
- `models` consulta `models inspect` de Strata para cada run config del catálogo.
- `info`, `config`, la ayuda y el informe Markdown de `test` respetan el idioma
  guardado. El informe enlaza las trazas y métricas de los subagentes.
- Los alias públicos se aplican donde el engine lo admite, incluidos perfiles
  Strata mediante una copia privada y MLX-Serve en sus ajustes globales por
  ruta; los motores parciales se identifican como tales en `models --all`.
- `connect --agent codex` ya no entra en pánico si `config.toml` no contiene
  `[model_providers]`; crea la tabla con accesos validados.

### English

- `models add`, `info`, and `command` are implemented; `models` replaces `list`.
- `config --lang ES|EN` stores the language preference. FLM uses the canonical
  `fastflowllm` id, and Ignis models separate their internal name and public
  alias.
- `connect` uses `--remove`; `--list` reports only implemented integrations.
- `connect` prints manual setup instructions for TRAE and TRAE CN. OpenCode
  preserves comments and trailing commas when editing `opencode.jsonc`, and
  leaves invalid JSONC files untouched.
- Model startup accepts levels, flags, and options declared by each TOML.
- Codex stores the provider and per-engine profile in `CODEX_HOME` (or
  `~/.codex`) and rejects engines without documented Responses API support.
  llama.cpp receives the public alias through `--alias`.
- `ropero test` uses `--model`, `--group`, `--test`, `--dimension`, and `--all`.
- Ignis stores and checks each download under `ARTIFACTS_DIR/<internal-name>`;
  Codex validates optional TOML tables without panic-prone indexing.
- `models` queries `strata models list --installed --output json` and marks
  complete Strata profiles as available.
- `info`, `config`, help, and the `test` Markdown report follow the saved
  language. The report links each subagent's trace and includes its metrics.
- Public aliases are applied where supported, including Strata through a
  private profile copy and MLX-Serve's global per-path settings; partial
  engines are marked as such in `models --all`.
- `connect --agent codex` no longer panics when `config.toml` lacks
  `[model_providers]`; it creates the table and validates its structure.

## En trabajo — instalación de motores
- `ropero install engines` ejecuta recetas declaradas `[install]` para compilar
  `llama-server` upstream y el fallback público de `turbo-llama` antes de crear
  el enlace. La receta turbo no aplica los parches privados turboquant; otros
  motores sin receta siguen requiriendo su runtime/binario.

Este fichero registra los cambios del workspace Rust de **ropero**. El
historial de cambios de la v1 no está incluido en este checkout.

El formato sigue [Keep a Changelog](https://keepachangelog.com/es/1.1.0/) y el
versionado sigue [SemVer](https://semver.org/lang/es/).

## [Sin publicar] (working tree)

Cambios de contrato/operación documentados para el árbol de trabajo actual:

- **Configuración y catálogo (ADR-008):** catálogo externo por defecto en
  `<config>/models/<motor>/` con manifiestos en `<config>/engines/`; bootstrap
  idempotente de `<config>/` y `config.toml` sin sobrescribir; precedencia
  `entorno > config.toml > defecto` para `models_dir`, `gguf_dir`,
  `safetensors_dir`, `bin_dir`, `logs_dir` (por defecto `<config>/logs`).
  Overrides `ROPERO_MODELS_DIR`/`ROPERO_ROOT` preservados; TOML/manifiestos del
  repositorio para desarrollo/pruebas sin autodiscovery en uso normal.
- **Superficie CLI:** `ropero config` muestra rutas/origen; `ropero doctor`
  incluye estado de configuración; `ropero template` sugiere plantilla sin
  escribirla. Estos comandos no modifican artefactos de modelos (puede ocurrir
  bootstrap idempotente de configuración).
- **FLM delegado (ADR-010):** tag primario `[model].path` o `--decision`;
  instalación/eliminación/listado delegados al manifiesto cuando aplica;
  `flm serve <tag>` con defaults y `--ctx-len` si falta; estado por
  `flm list --json` de **mejor esfuerzo** (sin afirmar verificación end-to-end).
- **Ejecución y logs (ADR-010):** `--background` desacoplado sin salida terminal
  ni `ROPERO_PORT`, logs en `logs_dir`; foreground con salida en vivo
  (colorizado semántico común más patrones característicos por motor) y
  log de modelo; registro de uso append-only en `logs_dir/usage.log` (sin API
  keys ni flags/argumentos).
- **Eliminación de modelos (ADR-010):** `ropero <alias> remove` con plan de
  artefactos derivables, confirmación interactiva en TTY, sin borrar
  compartidos/no declarados, retirada del directorio de modelo en `GGUF_DIR`
  sólo si queda vacío, eliminación del checkpoint safetensors exacto declarado y
  eliminación delegada de todos los tags FLM declarados que se instalaron.
- **Migración:** copiar TOML heredados a `~/.config/ropero/models/<motor>/` y
  manifiestos a `~/.config/ropero/engines/` (o rutas configuradas).

## [0.1.0] - 2026-11-15

Es la versión del workspace (`Cargo.toml`). Reúne lo publicado de la v2 hasta
la forja, en orden de fase: **F0** cimientos, **F1** modelo de datos, **F2**
motores, **F3 y F4** runtime y estado, **F5** catálogo y descargas, **F6**
forja (`ropero gguffer`).

### Añadido

**F0 · Cimientos**

- **Workspace Rust de seis crates** (doc B T-01: `ropero-core`, `ropero-runtime`,
  `ropero-download`, `ropero-forge`, `ropero-server` y `ropero-cli`), con Rust
  1.100.0 fijado en `rust-toolchain.toml`, edition 2024, `Cargo.lock` y perfil
  release (`lto = thin`, `codegen-units = 1`, símbolos cortados,
  `panic = abort`). El trabajo vivía en el disco sin versionar, a un fallo de
  persistencia de distancia: versionarlo es lo que permite evolucionarlo y
  verificarlo de forma continua.
- **Toolchain, configuración y CI reproducibles**: `rustfmt.toml`,
  `clippy.toml`, `deny.toml` y un workflow para Linux y macOS con `fmt`,
  `clippy --all-targets --all-features -D warnings`, `cargo-deny`, `test` y
  `nextest` con `--all-features`, y cobertura con `cargo llvm-cov --lcov`.
  Fijan las versiones y hacen comparables los cambios que siguen la
  especificación de construcción.
- **Test de arquitectura** (`R-ARQ-1`, puerta T-02): `ropero-core` no depende
  de E/S prohibidas ni de crates superiores, el grafo de crates no admite
  ciclos y `ropero-cli` es un dispatcher por debajo de un umbral de líneas. El
  umbral se ha recalibrado tres veces **con medición, no con estimación**:
  400 → 800 ([ADR-005](docs/adr/ADR-005-umbral-de-lineas-del-cli.md)),
  800 → 1100
  ([ADR-006](docs/adr/ADR-006-umbral-de-lineas-del-cli-tras-install.md)) y
  1100 → 1200
  ([ADR-007](docs/adr/ADR-007-umbral-de-lineas-del-cli-tras-gguffer.md)). Lo
  que se mide sigue siendo lo mismo: que la lógica no se instale en la CLI.
- **Los tres ADR de F0, aceptados**: [ADR-001](docs/adr/ADR-001-por-que-rust.md)
  decide usar Rust; [ADR-002](docs/adr/ADR-002-extraccion-vs-reescritura.md)
  decide extraer por capas y mantener la convivencia v1/v2 hasta F9; y
  [ADR-003](docs/adr/ADR-003-ropero-code-es-referencia.md) decide que
  `ropero-code/` es **referencia, no dependencia**, y que el catálogo de la v2
  es nuevo e independiente. La segunda evita un salto único que rompería
  contratos y estado en disco; la tercera evita que la v2 herede la forma de
  la v1 y sus datos defectuosos.

**F1 · Modelo de datos**

- **Modelo de datos tipado y validador del catálogo** con las **15 reglas** del
  contrato (doc B §4.1.1) y errores `ruta:línea`. La validación pura comprueba
  el nombre y su unicidad, que el nombre case con el fichero, el formato, los
  ficheros, la descarga, los placeholders, los niveles, los alias, las
  opciones, la RAM mínima, las claves desconocidas y que `context`, cuando
  está declarado, sea múltiplo de 32. Los errores de parseo de clave
  desconocida y motor desconocido se clasifican como `UnknownKey` y
  `EngineInvalid`, y el fallo de sintaxis es `ParseInvalid`, que no es una
  regla: así la regla que se reporta es la que de verdad se rompió. Los tipos
  del TOML cierran `R-DATA-1…16` y el validador `R-DATA-17…23` (doc B T-06 y
  T-07): un TOML inválido no pasa sin explicar dónde falla.
- **Catálogo con unicidad y alias**, descubrimiento de modelos activos y
  retirados, y resolución de nombres y alias. Un directorio de motor
  desconocido se informa como error del catálogo, pero no aborta el escaneo
  del resto de modelos.
- **Catálogo propio de la v2 en `models/`** (doc B §2.4): 46 TOML versionados
  (17 activos + 28 retirados en `fired/`, cada uno bajo el directorio de su
  motor, + `_template.toml`), generados desde los TOML de la v1 como **fuente
  de referencia, en modo lectura**. Dos correcciones: `context = 133072` →
  `131072` en `mlx/metalnemotron.toml` y `mlx/metalqwen.toml` (el valor
  estándar 2¹⁷ = 128 K, múltiplo de 32; 133072 era un typo), y el renombrado
  de `tensorrt/groot.toml` a `tensorrt/groot-trt.toml` para que el fichero
  coincida con `name = "groot-trt"` (el nombre **no** se toca: los nombres son
  únicos globales y `llama/groot.toml` ya ocupa `groot`). Todo lo demás es
  idéntico a la fuente, y `ropero validate` sobre el catálogo nuevo da
  `OK: 45 modelo(s) validados` (exit 0).
- **`ModelSpec` y sus rutas**: el formato declara dónde vive el artefacto
  (R-MOD-6) — `gguf` es `$GGUF_DIR/<modelo>/<[files].gguf>`,
  `safetensors` es `$SAFETENSORS_DIR/<[model].path>`, y en `q4nx`
  `[model].path` es un tag de FLM y en `strata` es `<familia>/<paquete>`. Un
  único resolutor calcula las rutas del runtime, de la descarga y del listado:
  así un modelo no puede vivir en un sitio y buscarse en otro.
- **Montaje del argv** en el orden `base → level → flags → options`, con
  placeholders, comprobación de `requires`, alias de nivel y `replace`.
  `replace` sustituye la pareja del base en su posición para no duplicar una
  opción y dejar que el motor gane por casualidad. Cubre `R-DATA-16` y
  `R-DATA-17`, y `R-DATA-13` queda ligado al contrato de motor.
- **Golden master del argv** congelado con la salida de la v1 para cuatro
  modelos reales. Los TOML de referencia están congelados en
  `crates/ropero-core/tests/golden/models/`, de modo que la prueba no depende
  del catálogo real. Antes de reimplementar una función se fija su
  comportamiento observable, para detectar regresiones durante la extracción.
- **Identificadores de motor y alias legacy**: los ids canónicos y los alias
  heredados se canonicalizan con aviso, nunca en silencio. Cierra `R-ENG-1` en
  el alcance del enum `EngineId`.

**F2 · Motores**

- **Ocho manifiestos de motor** (`engines/*.toml`), uno por motor, al lado del
  catálogo: id, etiqueta, color, descripción, formatos, madurez, rango de
  puertos, dependencias (lo que chequea `ropero doctor`), reglas `compat` de
  SO y hardware, y `[detect]` con binario y rutas de instalación. Los motores
  pasan a ser **datos**: la leyenda, los puertos y las dependencias tienen un
  solo sitio, y un campo desconocido se rechaza nombrando el campo culpable.
  `maturity` distingue los seis motores arrancados de verdad en la máquina del
  dueño (`proven`) de los dos integrados y nunca arrancados (`vllm`, `strata`),
  y el criterio está escrito en el TOML de cada motor.
- **La nomenclatura canónica de motores**
  ([ADR-004](docs/adr/ADR-004-nomenclatura-de-motores.md)): `llama-server` es
  el repo **oficial** de llama.cpp y `turbo-llama` es el **fork privado** del
  dueño. El cambio **invierte** los dos nombres de la v1, donde `llama-server`
  era el fork; los alias legacy (`llama-server-official`, `llama-official`,
  `llama`, `turbo`) siguen entrando, con aviso. El catálogo migra con el
  renombrado: `models/llama/` → `models/turbo-llama/` y `models/llama-official/`
  → `models/llama-server/`; los directorios legacy se siguen mapeando, para que
  un catálogo viejo se lea igual.
- **Evaluación de compatibilidad que acumula todas las razones**: las reglas de
  un manifiesto son **alternativas** (la máquina vale si cumple al menos una) y
  dentro de una regla las condiciones son **acumulativas**. El usuario que está
  en Linux y quiere TensorRT-LLM necesita saber que le faltan *dos* cosas (una
  NVIDIA y 32 GB de RAM), no una sola. La captura de la máquina
  entra por parámetro —`ropero-core` sigue sin leer disco— y un dato que no se
  pudo leer se trata como ausente: nunca tumba el escaneo.
- **Puertos por motor, sin solapes**: `llama-server` 8100-8199, `turbo-llama`
  8200-8299, `mlx-serve` 8300-8399, `fastflowlm` 8400-8499, `tensorrt-llm`
  8500-8599, `vllm` 8600-8699, `airllm` 8700-8799 y `strata` 9900-9999. Varios
  motores pueden servir a la vez sin pisarse, y cada arranque comprueba que
  nadie escuche en el puerto que va a usar.

**F3 y F4 · Runtime y estado**

- **Preflight del arranque y elección de puerto, puros**: el preflight (motor
  instalado, modelo instalado, VRAM/RAM) acumula **todas** las razones y cada
  razón dice qué hacer; `pick_port` da el menor puerto libre del rango del
  motor, y un rango invertido se reporta como error en vez de arreglarse
  invirtiendo los extremos. Son decisiones, no lecturas: se prueban sin
  máquina.
- **Arranque de modelos con preflight, sonda, sello y estado en disco** (doc B
  §3.3): preflight, `--force`, puerto, `spawn`, sonda y estado. El **sello** es
  la identidad completa (`PID` + `start_time` + digest del binario), que es lo
  que impide matar procesos ajenos (P-3). El estado se escribe de forma atómica
  (tmp + rename) y conserva las claves que no vienen; las claves nuevas son
  adiciones compatibles: un lector de la v1 sigue entendiendo el fichero. El
  motor se lanza en su propio grupo de procesos, así que su salida va al log:
  sin eso, un motor que aborta se queda sin explicar por qué.
- **Sonda de readiness en tres niveles**: TCP (el puerto acepta conexiones) →
  HTTP (el endpoint de salud responde 200) → inferencia (`GET /v1/models`). Se
  prueban en orden y el primero que pasa es suficiente: esperar a la inferencia
  en un modelo que carga rápido es tirar el tiempo del usuario, y un puerto
  abierto antes de tiempo es mentir. La prueba es la misma de la v1, con la
  clave de API, y dice **qué** modelo está sirviendo.
- **Primer plano con Ctrl-C propagado**: ropero espera al motor, coge el SIGINT
  y se lo manda al grupo; si no, el Ctrl-C deja el motor sirviendo y ropero
  muerto. Sale con **130** (doc CLI v2 §3). El puerto se anuncia con
  `ROPERO_PORT=<n>` en cuanto el motor está listo: es la última línea de stdout
  y es lo que leen los scripts.
- **Parada por PID verificado, nunca por nombre** (R-EST-8): hay estado, el
  proceso sigue vivo, el `start_time` coincide, el digest coincide; a partir de
  ahí, SIGTERM al grupo, espera y SIGKILL. No mata por nombre de proceso (un
  `pkill -f` mataría los motores de otro proyecto), no mata un PID reciclado y
  no toca un estado sin `start_time`. Parar un modelo respeta a los demás
  modelos que comparten motor, porque se mata por PID.
- **Doce comandos y dos invocaciones**: `install`, `stop`, `gguffer`,
  `status`, `doctor`, `list`, `engines`, `template`, `validate`, `args`,
  `levels` y `path`, más `ropero` a secas y `ropero <modelo>` (seis flags:
  `--force`, `--cpu`, `--port`, `--api-key`, `--no-api-key`, `--ctx`). El
  contrato vive en el dominio (`ropero_core::surface`) y la CLI lo traduce: un
  comando que aún no se ejecuta **se parsea** y dice qué falta (exit 1), y un
  flag inválido sigue siendo error de uso (2). `build.rs` fija versión, fecha de
  compilación (con `SOURCE_DATE_EPOCH`) y autor.
- **Resumen, `list` con colores, `status` y `doctor`**: `ropero` a secas muestra
  nombre, build, autor, motores y modelos (disponibles e instalados) y la
  sugerencia de `--help`; `ropero list` lista **sólo** los modelos disponibles
  (motor instalado **y** modelo descargado) agrupados por motor, con la leyenda
  de colores en ANSI de 256 colores, y `--all` añade los no disponibles,
  marcados; un motor no instalado sale **plegado**, con su conteo y la razón.
  `ropero status` da la vista de la máquina (CPU, dGPU, iGPU, RAM/SWAP, con el
  zram enseñado aparte para que no se confunda), y lo que no se puede leer sale
  «no disponible»: un `status` que miente es peor que un `status` con huecos.
  `ropero doctor` comprueba primero las dependencias del propio ropero y
  después las de **cada** motor, diciendo para cada una si se cumple y, si no,
  qué hacer.

**F5 · Catálogo y descargas**

- **`ropero install engines [<lista>]` y `ropero install models [<lista>]`**:
  con lista, directo (modo desatendido); sin lista, menú, y el de modelos
  ofrece **sólo los modelos de los motores instalados**. Sin terminal el menú es
  error de uso (2): un menú que se traga un EOF acabaría eligiendo por su
  cuenta. La instalación de un motor es **sólo el enlace** (R-INST-1) a su
  binario, resuelto con la precedencia **PATH → `$BIN_DIR` → rutas largas**
  (R-INST-24) y enlazado con su `basename` (R-INST-25): ni copia, ni descarga,
  ni compila, y el motor que se usa es el del `PATH`, no el recién compilado. Un
  motor ya enlazado se dice y no se toca, y un fichero real en el destino tampoco
  se pisa.
- **El contrato de `--prio`** (doc B §4.10.7), entero: `--prio=download|build`
  puede ser **global** (antes del primer modelo) o **por modelo** (se aplica al
  último modelo que lo precede, y su ámbito termina en el siguiente modelo), con
  **precedencia del por modelo**; se **ignora** si el modelo no tiene ese camino
  —y el aviso **no** aborta los demás modelos—, y se **pregunta al usuario**
  cuando el modelo declara las dos vías y no hay `--prio`. Un valor inválido es
  error de uso **con el valor recibido íntegro**, y `--prio` en
  `install engines` es error de uso: sólo vale para `install models`. Es dominio
  puro, así que el contrato entero se prueba con entradas.
- **La descarga real con `hf` según la secuencia exacta de R-MOD-16**: el plan
  puro calcula destino, repo, fichero, tamaño informativo, excludes,
  auxiliares y los argumentos exactos de `hf`; la ejecución sigue el doc (final
  presente y no vacío → «ya descargado»; fichero con el nombre del repo → `mv`;
  **`hf` antes de preguntar**; confirmación con el tamaño y default **no**;
  `hf download` al staging; búsqueda por `basename`; rename comparando rutas
  absolutas; `rm -rf` del staging). El GGUF usa staging propio **dentro del
  destino** (el `mv` es atómico y dos descargas con el mismo nombre de fichero no
  se pisan) y el checkpoint **no** (R-MOD-17): `hf` es reanudable con
  `--local-dir` fijo, y un staging mataría la reanudación de una descarga de
  12 GB cortada a medias. La verificación es por contenido real (R-MOD-18) y se
  **re-verifica aunque `hf` salga con 0** (R-MOD-19): la salida de `hf` no
  prueba que el fichero esté. No hay verificación por hash, y el estado se
  deduce del disco. Los auxiliares `use = run` se bajan al directorio del GGUF,
  y la salida de `hf` va a stderr: stdout es para los datos.
- **`ropero install models <lista>` instala de verdad la vía `download`**, y la
  línea por modelo dice qué pasó (`ya descargado`, `descargado`, `omitido por el
  usuario` o `error: …`), con códigos 0 (todo instalado u omitido), 1 (algo
  falló) y 2 (uso). La vía `build` dice la verdad: fabricar el GGUF es la forja.

**F6 · Forja**

- **`ropero gguffer`**, el último comando del contrato del CLI v2 (§11):
  fabrica un GGUF desde un repo de HuggingFace, desde un directorio de pesos
  locales o desde un GGUF que ya existe. Sus **diecisiete flags** —los quince de
  la tabla del doc (§11.2) más `--gguf` y `--out-principal`, que vienen de
  R-GGUF-3/4—, con `--quant` repetible y con coma, y la pareja
  `--imatrix-repo`/`--imatrix-file` comprobada **por el dominio** y no por
  `clap`, para que el mensaje sea el del doc, en español, y diga qué hacer.
  `--list-quants` enseña los quants que publica el motor (nombre ↔
  `file_type`) y los ya publicados de ese modelo; sin motor, la tabla de
  reserva y qué hacer. `--diagnostico` enseña qué motor hay, dónde y qué falta,
  y está exento de la regla del imatrix. La lógica no vive en la CLI: `gguffer`
  son 77 líneas, 19 de ellas comentarios.
- **Las seis fases de la tirada** (R-GGUF-15), en orden fijo: fuente →
  conversión → cuantización → verificación → limpieza → procedencia. La
  conversión produce el intermedio **sólo** si va a haber cuantización; una
  colocación o una conversión de dtype plana sale directo al fichero final, sin
  imatrix ni horas de CPU; el cuantizador corre con log aparte y su cola (20
  líneas) va al error, y el error de imatrix se traduce a las dos salidas de la
  cabeza MTP.
- **Checkpoint por fase con la huella de la petición**
  (`<work>/.ropero-forge.json`): la huella es la forma canónica de la petición
  (fuente, quants en orden, intermedio, imatrix, `out`, `out-principal`,
  `no-mtp`/`no-dflash`/`force`); los `--keep-*` no entran porque no cambian lo
  que se produce. Una huella distinta descarta el checkpoint y **rehace** la
  tirada, y `--force` lo descarta siempre; el informe lo dice. Lo que hace que
  una tirada cara no se repita de gratis: si la cuantización falla a las tres de
  la madrugada, la descarga (una hora, 60 GB) y la conversión **no** se repiten.
  Un checkpoint sin huella o corrupto no reanuda.
- **Política de limpieza** (R-GGUF-16/17/18) en **una función pura** que la fase
  5 ejecuta: al fallar la cuantización **no se borra nada** —un fallo de
  cuantización no debe costar 60 GB de descarga—; los pesos sólo se borran si la
  verificación pasó, porque un GGUF corrupto con los safetensors vale más que 60
  GB; el GGUF que trajo el usuario **jamás** se borra; el intermedio se borra
  después de todos los quants, y si el F16 **es** el resultado, no se toca. El
  checkpoint de la limpieza se escribe **antes** de los borrados, porque la
  limpieza puede eliminar el work con el checkpoint dentro.
- **La procedencia es obligatoria** (`<out>.fuente.json`, R-GGUF-19): de dónde
  salió el modelo, el quant **por nombre** (no por número: es lo que va al model
  card), la importance matrix con su fichero, su repo y su sha256, y la cabeza
  MTP si quedó en un quant aparte. El JSON se escribe a mano, como el resto de la
  forja, para que el crate no arrastre dependencias. **Un GGUF sin sidecar no
  sale con éxito**: el camino «ya está» salía con 0 y sin procedencia, y ahora la
  tirada falla con un error que dice qué hacer (borrar el GGUF y rehacer, o
  rehacer con `--force`). «Un GGUF sin saber CÓMO se hizo no se puede publicar
  con vergüenza.»
- **El estado real de cada salida** (R-GGUF-20): **convertido** (lo generó el
  conversor), **colocado** (el GGUF de origen ya estaba en ese quant y se copió
  tal cual), **cuantizado** (pasó por el cuantizador) o **ya existía** (no se
  rehizo). El sidecar y el informe los nombran, y el checkpoint los guarda para
  que una tirada reanudada siga diciendo la verdad: lo que se cuenta es lo que
  las fases hicieron, no lo que la tirada iba a hacer.
- **La cabecera GGUF se lee con las constantes del formato**: magic `GGUF`,
  versión `u32` (1, 2 o 3), `n_tensors` y `n_kv` (`u64`) y los pares
  clave/valor con la tabla de tipos de `gguf.h`, con longitudes `u32` hasta la
  v1 y `u64` desde la v2. La lectura es **acotada** (1 MiB) y devuelve `None` si
  el fichero está truncado, si la versión no vale o si `general.file_type` no
  está. La clase del GGUF sale de su cabecera, no de su nombre: un fichero que
  dice BF16 puede contener un IQ2_M.
- **La precondición del motor, antes de descargar** (R-GGUF-56, T-29): sin
  `llama.cpp` no hay conversión, ni cuantización, ni unión de shards, y **no se
  baja ni un byte**. El conversor (`convert_hf_to_gguf.py`) **no admite
  sustituto**; el cuantizador y el unidor sí tienen alternativa en el `PATH`. El
  motor se busca en `$LLAMA_DIR` (el fork) y `$LLAMA_OFFICIAL_DIR` (el
  oficial), y el error nombra el motor que falta y la variable que lo apunta. La
  viabilidad de la clase se comprueba con **sólo la metadata** del repo (un par
  de KB, R-GGUF-8), antes de bajar los pesos: si el GGUF está en un quant que no
  se puede re-cuantizar, el error sugiere el repo hermano sin cuantizar. Las
  validaciones duras de la petición se comprueban **antes de nada** (R-GGUF-6):
  bajan 60 GB tontos con la misma facilidad que un error de uso.

### Cambiado

- **El dominio de `ropero-core` es puro**: recibe datos ya resueltos y no abre
  ficheros, procesos ni red, y el escaneo del catálogo y la captura de la máquina
  pasan a `ropero-runtime`, que es la capa de E/S. Separar la decisión de la E/S
  permite probar el comportamiento sin red ni GPU, localizar mejor los fallos y
  mantener la CLI como dispatcher.
- **El parser pasa de escrito a mano a `clap` con el builder**, y el contrato de
  los flags vive en el dominio (`ropero_core::surface`): añadir un flag es añadir
  una línea al dominio y cero a la CLI. El `derive` no servía: obligaba a repetir
  cada flag y no sabía expresar `ropero <modelo>` (un token que puede ser comando
  o modelo) sin `external_subcommand`, que no parsea flags.
- **La ayuda de la CLI enumera los comandos que existen** y está **en español**,
  incluidas las cabeceras que `clap` pone en inglés. Se evita anunciar una
  superficie todavía no implementada.
- **`Limits::min_ram_gb` es `Option<f64>`**: el campo es opcional y sólo se
  comprueba cuando está declarado; un valor declarado debe ser mayor que cero,
  como en la v1.
- **La resolución de placeholders de `[files]` usa un predicado único**: el
  mismo reconoce las claves de `[files]` declaradas y las variables de entorno
  disponibles.
- **`ropero validate` valida también los manifiestos de motor** y el catálogo de
  la v1 (`ropero-code/models`) **ya no se valida**: es sólo referencia, y la v2 se
  desarrolla como si esa carpeta no existiera. Es la decisión de
  [ADR-003](docs/adr/ADR-003-ropero-code-es-referencia.md). Las tres fichas de la
  v1 con `[download]` **y** `[build]` (`llama/nemotron.toml`,
  `llama-official/nemotron-o.toml` y `llama/fired/granite.toml`) eran violaciones
  por la regla que **se eliminó**: tener las dos vías es un estado válido del
  TOML, y la elección de camino es contrato de `ropero install models`
  (R-MOD-29), no del validador.
- **La batería por defecto deja de depender de `ropero-code/`**: los TOML del
  golden master están congelados en el crate, y las dos comparaciones contra la
  v1 real quedan marcadas `#[ignore = …]` y se ejecutan de forma explícita
  (`cargo test --locked --workspace -- --ignored`). El catálogo de la v2
  (`crates/ropero-core/tests/catalogo_real.rs`) corre siempre, en la batería y
  en CI.
- **Se retira la API redundante y los tipos muertos** de `ropero-core`. Una única
  API hace más difícil conservar contratos duplicados durante la extracción.

### Corregido

- **El puerto se reintenta cuando el motor muere al hacer `bind`**: elegir el
  menor puerto libre y arrancar es una **carrera** —dos procesos que usan el
  mismo rango ven el mismo puerto y lo cogen—, y el que pierde el `bind` moría
  con «address already in use», dejando arranque fallado, sin estado en disco y
  puerto ocupado por el motor que ganó. Ahora se prueba con el siguiente puerto
  libre del rango, con el bucle **acotado** por el tamaño del rango, y sin
  reintento cuando el puerto lo pidió el usuario (`--port`); `--force` queda
  **antes** del bucle, así que lo que se paró sigue parado.
- **Un motor zombi ya no parece vivo**: `kill(pid, 0)` daba por vivo al motor que
  murió al hacer `bind` y que el padre no había recogido, y el arranque no podía
  distinguir «motor sano» de «motor muerto». `alive()` mira el estado que dice el
  kernel, y el motor muerto se recoge en cuanto se ve muerto, para que el
  reintento no deje zombis. La sonda pregunta al motor **primero** y sólo acepta
  la respuesta si el motor sigue vivo (antes podía estar respondiendo otro motor
  que cogió el puerto), y la parada usa la misma comprobación, así que un motor
  zombi se da por parado en vez de esperar al deadline.
- **El motor falso de los tests de la CLI ya no sobrevive como nieto**: su padre
  era el ropero, así que al soltar el test sólo se mataba al ropero y el motor
  se quedaba escuchando en el puerto del rango, donde el siguiente test —o la
  siguiente vuelta de la suite— lo encontraba ocupado. Ahora el `Drop` para
  también el motor, con la misma parada verificada (PID + `start_time`) que usa
  `ropero stop`, para no matar un PID reciclado.
- **Las rutas del catálogo están acotadas al directorio de modelos** (path
  traversal): con `Path::join` una ruta absoluta absorbe el base y un `..` sube,
  así que un `[download].file = "../../../secreto.txt"` hacía que el `mv` de
  R-MOD-16.2 se llevara un fichero de fuera del directorio, y un
  `[download.aux.*].name` fuera lo sobrescribía. Un único helper
  (`ropero_download::ruta_dentro`) comprueba la ruta **antes** de unirla:
  rechaza la raíz absoluta, rechaza los `..` y exige que la ruta normalizada
  empiece por el base; se aplica a `download.file`, `download.name`, `aux.file`,
  `aux.name` y `model.path`, y en la ejecución, antes de cada `join`, `rename` y
  `create_dir_all`. El error es defecto del catálogo (exit 1) y nombra el campo
  y el valor recibido.
- **El GGUF se descarga donde el runtime lo busca** (R-MOD-6): el destino se
  construía con el `basename` de `[files].gguf`, así que con
  `[files].gguf = "carpeta/mi.gguf"` el fichero caía en `…/<modelo>/mi.gguf`
  mientras `ropero path <modelo>` decía `…/<modelo>/carpeta/mi.gguf`. El modelo
  quedaba inutilizable y la segunda instalación volvía a descargar. Ahora se usa
  la **ruta relativa completa** y el subdirectorio se crea al descargar.
- **La selección en el menú es consentimiento de la descarga** (R-MOD-27): en el
  menú de `ropero install models` se preguntaba «¿Descargar X (~4 GB)? [s/N]:» y
  con Intro **no se descargaba nada** (exit 0): elegir un modelo no era consentir
  su descarga. Elegir un modelo **es** consentir su descarga, y la pregunta de
  tamaño no se repite: el comando va siempre en modo desatendido.
- **El resumen cuenta los TOML del catálogo, no las filas de alias**: los cuatro
  TOML con `[model].alias` materializan una fila extra cada uno, y `ropero` a
  secas decía «modelos disponibles: 49» con 45 TOML. Ahora `counts()` cuenta
  **modelos únicos por nombre real** —la fila de un alias y la de su TOML de
  origen son el mismo modelo—, y los dos recuentos (`ropero` y
  `ropero validate`) coinciden. El listado no cambia: las filas de alias siguen
  apareciendo, porque un alias responde igual que el nombre.
- **La cabecera GGUF se lee con la tabla canónica del formato**: el lector usaba
  los tipos equivocados (UINT32=6, STRING=12, ARRAY=13) y por eso no encontraba
  `general.file_type` en ningún GGUF real: la clase salía siempre del nombre del
  fichero. Las fixtures se rehacen **según el formato, no según el código roto**
  (v1, v2 y v3, el `file_type` de cada tipo de valor, las arrays anidadas, el
  fichero truncado y la versión inválida).
- **El hash de la imatrix remoto llega al sidecar**: se bajaba a la caché del
  work, y la limpieza (fase 5) borraba el work **antes** de que la procedencia
  (fase 6) se escribiera, así que el sha256 salía sin escribir y el sidecar
  afirmaba el fichero y el repo pero no **cuál** matriz se usó. Ahora el hash se
  calcula en la fase 3, con la imatrix delante, y se guarda en el checkpoint.
- **El sidecar nombra el estado real de cada salida**: decía «el GGUF ya estaba:
  no se cuantizó» para un GGUF que el conversor acababa de convertir en esa misma
  tirada. La segunda parte era cierta; la primera, mentira.
- **El checkpoint sabe qué petición es**: guardaba sólo la fase, y otra petición
  sobre la misma fuente reanudaba con el checkpoint ajeno y salía 0 anunciando un
  GGUF que no estaba hecho. Con huella distinta se descarta y la tirada se
  rehace.
- **El listado encuentra los GGUFs publicados**: miraba sólo los ficheros del
  primer nivel, así que los GGUFs del directorio del modelo no aparecían; ahora
  recorre el primer nivel de subdirectorios y el artefacto `<modelo>.gguf`, y el
  quant se toma del trozo tras el último `-` **sólo** si el fichero es de ese
  modelo: el listado no mezcla modelos.
- **El nombre de la fuente no pierde los dígitos del fichero.**
- **Los mensajes y los errores dicen qué hacer** (P-13): un `--quant` inválido
  sale con **2** (error de uso), como el resto de la superficie, y no con 1; el
  informe no puede enseñar un GGUF que no está en disco (nuevo
  `Error::SalidaFaltante`); la imatrix **local** se comprueba y su error dice el
  origen; y las pistas dicen «rehaz con `--force`» sin borrar los pesos,
  «comprueba que se puede escribir», «comprueba el disco y el entorno» y
  «reintenta: el checkpoint reanuda por donde iba». El mensaje falso de la v1
  («faltan ficheros en el clon: la descarga está incompleta») empujaba a borrar
  60 GB por un problema que no era de la descarga.
- **La ejecución de `hf` reintenta ante `ETXTBSY`** («Text file busy»): era una
  carrera de milisegundos cuando otro proceso tenía el fichero abierto para
  escribir, y fallaba el `exec`. Con el reintento, 60 tiradas seguidas salen
  bien; antes fallaba el 5 %.
- **El plan no repite el sufijo del quant**: pedir NVFP4 del repo
  `acme/modelo-NVFP4` colocaba `modelo-NVFP4-NVFP4.gguf`.
- **La batería de CLI es hermética**: usa un catálogo de pruebas propio y no
  depende del catálogo real de la v1. Así sus resultados no cambian cuando cambia
  un TOML de producción, y los logs del motor de los tests van a un temporal.
- **El contrato de CLI tiene una puerta propia** (`cli_contract`) y cubre los
  códigos de salida y la forma de la salida. `ropero <cmd> | head` termina con
  código 0: un pipe roto (SIGPIPE/EPIPE) no se confunde con un error real.
- **El valor de un `--prio` inválido aparece íntegro en el mensaje** (no se
  recorta), `--prio` en `install engines` es error de uso, un modelo **sin vías**
  sale con 2 y la línea de error de un modelo va a **stderr** (stdout es para los
  datos, R-MOD-11).
- **No hay panics en `ropero-core`**: el `unreachable!` de la resolución de un
  modelo es un camino de error real, y el sha256 se hashea **por partes**
  (bloques de 8 KiB), sin copiar el fichero entero en memoria.

### Deuda conocida

- El árbol de la versión contiene **653 funciones `#[test]`, dos marcadas
  `#[ignore = …]`**: las dos comparaciones del golden master contra la v1 real,
  que se ejecutan de forma explícita. La batería por defecto y la CI son
  herméticas y no dependen de `ropero-code/`.
- **Las cuatro comprobaciones de la v1 siguen sin implementarse**: que
  `[args.level.<n>]` esté en `[levels].list`, que `[args.flag.<f>]` y
  `[args.option.<o>]` estén declarados y que no falten `[download]` ni `[build]`.
  El doc B §4.1.1 las registra como deuda y es el contrato de la v2, no el
  contrato completo de la v1.
- **La regla de `context` múltiplo de 32 sólo aparece en el doc B §4.1.1.** La v1
  acepta `133072`, que no es múltiplo de 32, y la v2 es más estricta (el
  validador la aplica). Queda pendiente promover la regla a R-DATA en el doc A o
  retirarla.
- **La v2 sustituye `replace` en el lugar que ocupaba en el base; la v1 lo
  aplicaba al final.** La diferencia está cubierta por tests sintéticos, pero no
  por un caso real del catálogo, y requiere un PR propio para reconciliar el
  comportamiento. Además, si el token replaced es el último del base, la v2 quita
  un token y la v1 quita dos o uno (`_drop_flag_pair`).
- Los PDF de `docs-especificacion/` están desalignados respecto de los documentos
  Markdown normativos.
- **Quedan las fases F7 a F9**: F7 es la puerta OpenAI-compatible (endpoint,
  auth, streaming, enrutado), F8 es la instalación y la integración con opencode,
  y F9 es la convivencia v1/v2, la migración y la retirada de la v1.

## Sin publicar

<!-- Lo que se añada después de la 0.1.0 va aquí. -->

## Cómo leer esto junto al plan

Las entradas de la versión describen cambios comprobables, no el detalle del
trabajo pendiente. Para saber qué tarea o fase cierra cada entrada, usa las
referencias `R-*`, `T-*` y `F-*` del documento de construcción. Las fases **F0 a
F6** están terminadas; **F7** (la puerta) es la siguiente.
