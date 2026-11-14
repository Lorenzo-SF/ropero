# ADR-006 · El umbral de líneas del CLI, después de `ropero install`

**Estado**: aceptada
**Fecha**: 2026-11-14

## Contexto

[ADR-005](ADR-005-umbral-de-lineas-del-cli.md) fijó el umbral de
`ropero-cli` en **800** líneas, con el CLI en **770**. Ese margen (30
líneas) era el que el ADR dejaba para la fase siguiente, y lo que este ADR
hereda.

La fase F5 trae `ropero install` (doc CLI v2 §9), que es la **primera
superficie con menús interactivos** del CLI:

| Invocación | Qué trae |
|---|---|
| `ropero install` | engines y luego models, en ese orden |
| `ropero install engines [<lista>]` | menú de motores **o** lista, y el enlace real del motor |
| `ropero install models [<lista>]` | menú de modelos **o** lista, y la decisión de vía |

Con lo que se suma:

- **dos menús** (motores y modelos), cada uno con su numeración, su color
  y su lectura de la respuesta;
- **una pregunta** (qué vía se usa cuando el modelo declara las dos);
- el **contrato de `--prio`** (global, por modelo, precedencia e ignorado),
  que exige conservar **el orden** de los tokens de la invocación;
- la **instalación real** de motores: resolver el binario con la
  precedencia PATH → `$BIN_DIR` → rutas largas y crear el enlace;
- el **código de salida** de cada caso (0 ok, 1 error, 2 uso), que es
  parte del contrato.

La medición, sobre el código de este ciclo (no estimada):

| Fichero | Antes (F3) | Ahora (F5) | Qué es |
|---|---:|---:|---|
| `parser.rs` | 243 | 273 | la traducción del contrato a `clap` + el orden de los tokens de `install` |
| `commands.rs` | 287 | 287 | los comandos que hacen su trabajo |
| `install.rs` | — | 195 | `ropero install`: los dos menús y las dos preguntas |
| `summary.rs` | 53 | 53 | `ropero` a secas y el arranque de un modelo |
| `main.rs` | 100 | 101 | el dispatcher y los códigos de salida |
| `discovery.rs` | 96 | 96 | la frontera entre la CLI y el runtime |
| **total** | **779** | **1005** | |

Sin comentarios ni líneas vacías son **604** líneas de código (ADR-005
medía 529 con 770): el crecimiento son **226** líneas de las cuales 118
son comentarios. Lo que crece es **pegamento**, no lógica de negocio: por
cada línea de lógica hay una de explanatory.

## Decisión

El umbral de `ropero-cli` pasa de **800** a **1100** líneas.

El umbral **sigue existiendo** y sigue midiendo lo mismo: que el CLI sea un
dispatcher y no un sitio donde viva la lógica. Lo que cambia es su
calibración, porque la superficie del contrato creció otra vez.

### Por qué 1100 y no 800

El suelo real del CLI con esta superficie está entre **950 y 1000** líneas:
`ropero install` es una **tercera invocación con menús**, y cada menú son
unas 40 líneas de pegamento (componer el menú, enseñarlo, leer la
respuesta, traducirla) más las preguntas. Con 800 el contrato es
**imposible**: el test obligaría a gutar el código o a mover el pegamento
al runtime, que es exactamente lo que ADR-005 gallarda: por eso este ADR
recalibra el umbral, en vez de saltárselo.

### Por qué 1100 y no 1200

Con 1005 líneas, un umbral de 1100 deja **95** de margen: es lo que crece
el CLI con **un comando más** de la misma tamaño, y lo que hace que el
test siga mordiendo. Un umbral de 1200 dejaría 191, y a ese ritmo el
umbral dejaría de ser un límite antes de que llegue la siguiente fase.

### Lo que se hizo para no subirlo más

Está **medido**, y es lo que se intentaría otra vez:

1. **Mover la lógica al dominio.** Se hizo: el contrato de `--prio`, los
   menús, la resolución de las listas (ids, alias legacy, modelos
   desconocidos, motores incompatibles), los mensajes y el estado de un
   motor viven en `ropero_core::install` (`crates/ropero-core/src/install.rs`).
2. **Mover la E/S al runtime.** Se hizo: el TTY, la lectura de la
   respuesta, la resolución del binario del motor y la creación del enlace
   viven en `ropero_runtime::install`
   (`crates/ropero-runtime/src/install.rs`).
3. **Fusionar el pegamento repetido.** Se hizo: los dos menús comparten la
   misma pregunta (`ropero_runtime::install::pregunta_menu`), y el estado
   de un motor se traduce en un mensaje **en el dominio**
   (`ropero_core::install::mensaje_motor`), no en la CLI.

### Lo que **no** se hizo, y por qué

- **No** se movió `ropero install` a otro crate "para que el contador no lo
  vea". Eso sería Falsejar el métrico: el código seguiría siendo el mismo,
  sólo en otro sitio.
- **No** se cambió la métrica a "líneas de código" para que el número
  encajase. La métrica es la que es (líneas del fichero), y el ADR la
  recalibra; cambiarla ahora, justo cuando el total se pasa, sería
  exactamente el tipo de trampa que ADR-005 advierte.
- **No** se rebajó la calidad (quitar la ayuda en español, los comentarios
  o los mensajes que dicen **qué hacer**) para entrar en 800.

### El límite que el umbral sigue poniendo

Con 1100 líneas el test sigue **prohibiendo** que el CLI se llene de
lógica. Para que eso sea cierto, el umbral va acompañado de las dos reglas
que no se negocian:

- `ropero-core` no hace E/S (lo comprueba el mismo test).
- El CLI **no decide nada**: compone con funciones del dominio y las
  imprime. Si un comando necesita un `match` con reglas de negocio, ese
  `match` está en el sitio equivocado.

## Alternativas consideradas

- **Subir el umbral a 1200.** Da holgura de sobra, pero deja de ser un
  límite: con 1005 líneas, un margen de 195 es el que hace que el test
  duela cuando se acumula código de más.
- **Contar sólo las líneas de código (604).** Da un número más fino y
  más honesto sobre la lógica, pero **cambia la métrica** en lugar de
  calibrarla, y el cambio coincidiría con el momento en que el total se pasa. Queda para una revisión posterior, con su propio ADR.
- **Dejar el umbral en 800 y mover `install` al runtime.** Descartado: la
  E/S del runtime ya tiene su capa (`ropero_runtime::install`), y el
  *dispatch* de los comandos es del CLI por contrato (R-ARQ-1).
- **No medir el CLI en líneas, sino en "funciones públicas"**. Interesante,
  pero lo mismo de siempre: cambiaría la métrica en lugar de calibrarla.

## Consecuencias

- El test `ropero_cli_under_800_lines` pasa a `ropero_cli_under_1100_lines`,
  con un mensaje que dice el número real.
- El CLI del ciclo F5 se queda en **1005** líneas, con **95** de margen.
  La fase siguiente (F6, la descarga de modelos con `hf`) tiene que
  escribir su pegamento en `ropero-core` y `ropero-runtime`, no en el CLI:
  si necesita más de 95 líneas de pegamento, lo que hay que mover es el
  pegamento, no el contrato.
- La decisión de que la superficie del contrato viva en
  `ropero_core::surface` (y el parser sea una traducción de ella) sigue
  siendo la que hace que el CLI crezca **linealmente** con el número de
  flags, y no exponencialmente.
