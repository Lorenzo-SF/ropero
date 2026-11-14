# Changelog

Este fichero registra los cambios de **ropero v2**, la reimplementación en Rust
que se está construyendo en `crates/`. Los cambios de la v1 funcional que vive
en `ropero-code/` siguen en su propio
[`CHANGELOG.md`](ropero-code/CHANGELOG.md).

El formato sigue [Keep a Changelog](https://keepachangelog.com/es/1.1.0/) y el
versionado sigue [SemVer](https://semver.org/lang/es/).

## Sin publicar

### Añadido

- **Workspace Rust de seis crates** (`ropero-core`, `ropero-runtime`,
  `ropero-download`, `ropero-forge`, `ropero-server` y `ropero-cli`), con Rust
  1.100.0, edition 2024, `Cargo.lock` y perfil release. Se rescata el trabajo
  que había quedado fuera del control de versiones para poder evolucionarlo y
  verificarlo de forma continua.
- **Toolchain, configuración y CI reproducibles**: `rust-toolchain.toml`,
  `rustfmt.toml`, `clippy.toml`, `deny.toml` y un workflow para Linux y macOS.
  Fijan las versiones y hacen comparables los cambios que siguen la
  especificación de construcción.
- **Dos ADR aceptados**: [ADR-001](docs/adr/ADR-001-por-que-rust.md) decide
  usar Rust; [ADR-002](docs/adr/ADR-002-extraccion-vs-reescritura.md) decide
  extraer por capas y mantener la convivencia v1/v2 hasta F9. La segunda evita
  un salto único que rompería contratos y estado en disco.
- **Modelo de datos tipado y validador del catálogo** con 16 reglas y errores
  `ruta:línea`. La validación pura comprueba nombres, formato, ficheros,
  descarga, placeholders, niveles, alias, opciones y claves desconocidas.
  Los errores de parseo de clave desconocida y motor desconocido se
  clasifican como `UnknownKey` y `EngineInvalid`, respectivamente. Cierra
  `R-DATA-13` a `R-DATA-23` en el alcance del modelo y evita que un TOML
  inválido pase sin explicar dónde falla.
- **Catálogo con unicidad y alias**, descubrimiento de modelos activos y
  retirados, y resolución de nombres y alias. Un directorio de motor
  desconocido se informa como error del catálogo, pero no aborta el escaneo
  del resto de modelos.
- **Catálogo propio de la v2 en `models/`** (doc B §2.4): 46 TOML versionados
  en la raíz del workspace (17 activos + 28 retirados en `fired/` +
  `_template.toml`), generados desde los TOML de la v1 como **fuente de
  referencia, en modo lectura**. Dos correcciones: `context = 133072` →
  `131072` en `mlx/metalnemotron.toml` y `mlx/metalqwen.toml` (el valor
  estándar 2¹⁷ = 128 K, múltiplo de 32; 133072 era un typo), y el renombrado
  de `tensorrt/groot.toml` a `tensorrt/groot-trt.toml` para que el fichero
  coincida con `name = "groot-trt"` (el nombre **no** se toca: los nombres
  son únicos globales y `llama/groot.toml` ya ocupa `groot`). Todo lo demás
  es idéntico a la fuente. `ropero validate` sobre el catálogo nuevo da
  `OK: 45 modelo(s) validados` (exit 0).
- **El descubrimiento ya no mira `ropero-code/`**: la raíz de la v2 es la que
  tiene `models/`. `ROPERO_MODELS_DIR` y `ROPERO_ROOT` se siguen honrando, y
  la búsqueda por ancestrales funciona igual. La v1 queda como **referencia**
  (aprender, consultar, contrastar): la v2 se desarrolla como si esa carpeta
  no existiera.
- **Montaje del argv** en el orden `base → level → flags → options`, con
  placeholders, comprobación de `requires`, alias de nivel y `replace`.
  `replace` sustituye la pareja del base en su posición para no duplicar una
  opción y dejar que el motor gane por casualidad. Cubre `R-DATA-16` y
  `R-DATA-17`; `R-DATA-13` queda además ligado al contrato de motor en T-11.
- **Seis comandos de la v2**: `validate`, `list`, `template`, `args`, `levels` y
  `path`. Exponen el catálogo y el argv calculado sin arrancar todavía ningún
  motor.
- **Golden master del argv** congelado con la salida de la v1 para cuatro
  modelos reales. Los TOML de referencia están congelados en
  `crates/ropero-core/tests/golden/models/`, de modo que la prueba no depende
  del catálogo real. Antes de reimplementar una función se fija su
  comportamiento observable, para detectar regresiones durante la extracción.
- **Identificadores de motor y alias legacy**: los ids canónicos y los alias
  heredados se canonicalizan con aviso, nunca en silencio. Cierra `R-ENG-1` en
  el alcance del enum `EngineId` de T-11.
- **Test de arquitectura real**: `ropero-core` no depende de E/S prohibidas ni
  de crates superiores, el grafo no admite ciclos y el CLI permanece por debajo
  de 400 líneas (actualmente, 389). Cierra `R-ARQ-1` y la puerta T-02.

### Cambiado

- **La ayuda de la CLI enumera los comandos que existen** en la v2. Se evita
  anunciar una superficie todavía no implementada.
- **El dominio de `ropero-core` es puro**: recibe datos ya resueltos y no
  abre ficheros, procesos ni red. Separar la decisión de la E/S permite probar
  el comportamiento sin red ni GPU y localizar mejor los fallos.
- **`Limits::min_ram_gb` es `Option<f64>`**: el campo es opcional y sólo se
  comprueba cuando está declarado; un valor declarado debe ser mayor que cero,
  como en la v1.
- **La resolución de placeholders de `[files]` usa un predicado único**: el
  mismo recognise las claves de `[files]` declaradas y las variables de
  entorno disponibles.

### Corregido

- **La batería de CLI es hermética**: usa un catálogo de pruebas propio y no
  depende del catálogo real de la v1. Así sus resultados no cambian cuando
  cambia un TOML de producción.
- **El contrato de CLI tiene una puerta propia** (`cli_contract`) y cubre los
  códigos de salida y la forma de la salida. `ropero <cmd> | head` termina con
  código 0: un pipe roto (SIGPIPE/EPIPE) no se confunde con un error real.
- **Se retira la API redundante y los tipos muertos** de `ropero-core`. Una
  única API hace más difícil conservar contratos duplicados durante la
  extracción.

### Deuda conocida

- La batería del workspace contiene **156 tests pasados y 2 ignorados**. Los
  ignorados son los del golden master, que comparan con la v1 real
  (`ropero-code/`, sólo referencia) y se ejecutan de forma explícita;
  `environment.rs` está cubierto por 17 tests. El catálogo de la v2
  (`models/`) ya no está ignorado: `crates/ropero-core/tests/catalogo_real.rs`
  corre siempre y afirma 45 modelos (17 activos + 28 retirados) que validan
  sin violaciones.
- `ropero validate` sobre el **catálogo nuevo de la v2** (`models/`) informa
  `OK: 45 modelo(s) validados` y termina con código 0. El catálogo de la v1
  (`ropero-code/models`) ya no se valida: es sólo referencia, y la v2 se
  desarrolla como si esa carpeta no existiera.
- **Un modelo puede declarar las dos vías** (`[download]` **y** `[build]`) a la
  vez: **no es un error**. El doc B §4.1.1 la daba como error de uso (código 2)
  y R-DATA-6 situaba la elección entre `--download` y `--build` en el momento
  de instalar, no como defecto del TOML. Resuelto (decisión del dueño,
  2026-11-14): la fila del doc B §4.1.1 se borró y el validador acepta ambas
  tablas. La elección del camino es de `ropero install models` (F5/T-25,
  R-MOD-29): con las dos vías se pregunta al usuario, y `--prio=download|build`
  la elige sin preguntar (global o por modelo; si el modelo no tiene ese
  camino, se ignora). El exit 2 —error de uso— lo reservan los comandos, no el
  validador.
- **La regla de `context` múltiplo de 32 sólo aparece en el doc B §4.1.1.** La
  v1 acepta `133072`, que no es múltiplo de 32, y la v2 es más estricta. En el
  catálogo nuevo de la v2 se corrigió el typo: `mlx/metalnemotron.toml` y
  `mlx/metalqwen.toml` pasan de `133072` a `131072` (el valor estándar 2¹⁷ =
  128 K). Queda pendiente promover la regla a R-DATA en el doc A o retirarla.
- **La tabla del doc B §4.1.1 es el contrato de la v2, no el contrato completo
  de la v1.** La v1 comprueba además que `[args.level.<n>]` esté declarado en
  `[levels].list`, que `[args.flag.<f>]` y `[args.option.<o>]` estén
  declarados, y que no falten `[download]` ni `[build]`. Estas cuatro
  comprobaciones aún no están implementadas en la v2 y quedan registradas como
  deuda.
- **El arranque directo `ropero <modelo>` (R-CLI-6) y el código de salida 130
  para SIGINT quedan para F3.** El contrato de CLI documenta 130, pero la CLI
  v2 actual no ejecuta todavía el arranque directo en primer plano.
- **La v2 sustituye `replace` en el lugar que ocupaba en el base; la v1 lo
  aplicaba al final.** La diferencia está cubierta por tests sintéticos, pero no
  por un caso real del catálogo, y requiere un PR propio para reconciliar el
  comportamiento. Además, si el token replaced es el último del base, la v2
  quita un token y la v1 quita dos o uno (`_drop_flag_pair`).
- Los PDF de `docs-especificacion/` están desalignados respecto de los
  documentos Markdown normativos.
- F2 continúa con T-12: faltan los manifiestos de motor y la evaluación de
  compatibilidad. Las fases F3 a F9 siguen pendientes.

## Cómo leer esto junto al plan

Las entradas de esta sección describen cambios comprobables, no el detalle
del trabajo pendiente. Para saber qué tarea o fase cierra cada entrada, usa
las referencias `R-*`, `T-*` y `F-*` del documento de construcción. Las fases
F0 y F1 están terminadas; F2 es la siguiente.
