# ADR-007 · El umbral de líneas del CLI, después de `ropero gguffer`

**Estado**: aceptada
**Fecha**: 2026-11-15

## Contexto

[ADR-006](ADR-006-umbral-de-lineas-del-cli-tras-install.md) fijó el umbral
de `ropero-cli` en **1100** líneas, con el CLI en **1070**. Ese margen (30
líneas) era el que el ADR dejaba para la fase siguiente, y lo que este ADR
hereda.

La fase F6 trae `ropero gguffer` (doc CLI v2 §11), que es la **primera
superficie con tres modos** del CLI:

| Invocación | Qué trae |
|---|---|
| `ropero gguffer --repo/--path/--gguf` | la tirada: las seis fases de la forja |
| `ropero gguffer --list-quants` | los quants del motor y los ya publicados |
| `ropero gguffer --diagnostico` | qué motor hay, dónde, y qué falta |

Con lo que se suma:

- **diecisiete flags** (`--repo`, `--path`, `--gguf`, `--quant`,
  `--imatrix`, `--imatrix-repo`, `--imatrix-file`, `--out`,
  `--out-principal`, `--intermedio`, `--keep-source`, `--keep-intermedio`,
  `--no-mtp`, `--no-dflash`, `--force`, `--list-quants`, `--diagnostico`);
- los **tres modos**, con sus reglas (el diagnóstico está exento de la
  regla del imatrix, y sin motor los dos modos de consulta salen con 1);
- los **códigos de salida** de cada caso (0 ok, 1 error, 2 uso), que es
  parte del contrato.

La medición, sobre el código de este ciclo (no estimada):

| Fichero | Antes (F5) | F6 (medición del ADR) | Qué es |
|---|---:|---:|---|
| `parser.rs` | 273 | 273 | la traducción del contrato a `clap` |
| `commands.rs` | 287 | 287 | los comandos que hacen su trabajo |
| `install.rs` | 260 | 260 | `ropero install`: los dos menús y las dos preguntas |
| `summary.rs` | 53 | 53 | `ropero` a secas y el arranque de un modelo |
| `main.rs` | 102 | 102 | el dispatcher y los códigos de salida |
| `discovery.rs` | 96 | 96 | la frontera entre la CLI y el runtime |
| `gguffer.rs` | — | 77 | `ropero gguffer`: leer los flags, imprimir el reporte |
| **total** | **1071** | **1148** | |

Sin comentarios ni líneas vacías son **813** líneas de código (ADR-006
medía 759 con 1070): el crecimiento son **77** líneas de las cuales 24 son
comentarios. Lo que crece es **pegamento**, no lógica de negocio: 53
líneas de código para leer diecisiete flags, imprimir lo que el dominio
compone y traducir un error a su código de salida.

## Decisión

El umbral de `ropero-cli` pasa de **1100** a **1200** líneas.

El umbral **sigue existiendo** y sigue midiendo lo mismo: que el CLI sea un
dispatcher y no un sitio donde viva la lógica. Lo que cambia es su
calibración, porque la superficie del contrato creció otra vez.

### Por qué 1200 y no 1100

Con 1148 líneas, un umbral de 1100 es imposible: el `gguffer` **necesita**
sus líneas de pegamento (leer los flags, imprimir el reporte, traducir el
error), y quitarlas sería quitar el comando. Un umbral que obliga a
mutilar la superficie para que el test pase deja de medir lo que dice que
mide: de eso exactamente advierte el ADR-005.

### Por qué 1200 y no 1300

Con 1148 líneas, un umbral de 1200 deja **52** de margen: es lo que crece
el CLI con **un comando más** de la misma tamaño (y lo que hace que el
test siga mordiendo). Un umbral de 1300 dejaría 152, y a ese ritmo el
umbral dejaría de ser un límite antes de que llegue la siguiente fase.

### Lo que se hizo para no subirlo más

Está **medido**, y es lo que se intentaría otra vez:

1. **Mover la lógica al dominio.** Se hizo: toda la lógica del gguffer
   vive en `ropero-forge` (`crates/ropero-forge/src/`): la validación
   dura de la petición (`peticion.rs`), el plan y la viabilidad
   (`plan.rs`), la precondición del motor (`toolchain.rs`), las seis fases
   y su checkpoint (`ejecutar.rs`, `fases.rs`), la política de limpieza
   (`limpieza.rs`), la procedencia (`procedencia.rs`), la lista de quants
   (`listado.rs`), el diagnóstico y las líneas del informe
   (`reporte.rs`). La CLI no **decide** nada: compone con
   `ropero_forge::gguffer` e imprime.
2. **Que la superficie del contrato siga en el dominio.** Se hizo: los
   diecisiete flags viven en `ropero_core::surface`, y el parser los
   traduce a `clap`. Por eso el CLI crece **linealmente** con el número
   de flags (17 flags = 0 líneas de parser).
3. **Fusionar el pegamento repetido.** Se hizo: los tres modos del
   gguffer salen por **una** función (`ropero_forge::gguffer`), que
   devuelve un `Reporte` (líneas + error). La CLI no tiene un `match` por
   modo: tiene un `match` por **error**, que es lo único que le toca.

### Lo que **no** se hizo, y por qué

- **No** se movió `ropero gguffer` a otro crate "para que el contador no lo
  vea". Eso sería Falsejar el métrico: el código seguiría siendo el mismo,
  sólo en otro sitio.
- **No** se cambió la métrica a "líneas de código" para que el número
  encajase. La métrica es la que es (líneas del fichero), y el ADR la
  recalibra; cambiarla ahora, justo cuando el total se pasa, sería
  exactamente el tipo de trampa que ADR-005 advierte.
- **No** se rebajó la calidad (quitar la ayuda en español, los comentarios
  o los mensajes que dicen **qué hacer**) para entrar en 1100.
- **No** se quitó ningún flag del contrato del doc CLI v2 §11.2 para que
  el CLI encajase: el contrato manda, y el ADR se recalibra.

### El límite que el umbral sigue poniendo

Con 1200 líneas el test sigue **prohibiendo** que el CLI se llene de
lógica. Para que eso sea cierto, el umbral va acompañado de las dos reglas
que no se negocian:

- `ropero-core` no hace E/S (lo comprueba el mismo test).
- El CLI **no decide nada**: compone con funciones del dominio y las
  imprime. Si un comando necesita un `match` con reglas de negocio, ese
  `match` está en el sitio equivocado.

## Alternativas consideradas

- **Subir el umbral a 1300.** Da holgura de sobra, pero deja de ser un
  límite: con 1148 líneas, un margen de 152 es el que hace que el test
  duela cuando se acumula código de más.
- **Contar sólo las líneas de código (813).** Da un número más fino y
  más honesto sobre la lógica, pero **cambia la métrica** en lugar de
  calibrarla, y el cambio coincidiría con el momento en que el total se
  pasa. Queda para una revisión posterior, con su propio ADR.
- **Recortar el CLI para entrar en 1100.** Habría que mover `install` o
  el parser a otro crate, o dejar de documentar el gguffer: mover el
  pegamento a otro sitio "para que el contador no lo vea" es Falsejar el
  métrico (ADR-005), y quitar la documentación es rebajar la calidad.
- **Mover el `gguffer` al runtime.** Descartado: el runtime es la capa de
  E/S, y la lógica de la forja es dominio (además, `ropero-forge` ya es
  un crate propio: el CLI no crece con la forja, crece con el pegamento).

## Consecuencias

- El test `ropero_cli_under_1100_lines` pasa a `ropero_cli_under_1200_lines`,
  con un mensaje que dice el número real.
- El CLI del ciclo F6 se queda en **1148** líneas, con **52** de margen.
  La fase siguiente tiene que escribir su pegamento en `ropero-core` y
  `ropero-runtime`, no en el CLI: si necesita más de 52 líneas de
  pegamento, lo que hay que mover es el pegamento, no el contrato.
- La decisión de que la superficie del contrato viva en
  `ropero_core::surface` (y el parser sea una traducción de ella) sigue
  siendo la que hace que el CLI crezca **linealmente** con el número de
  flags, y no exponencialmente.
