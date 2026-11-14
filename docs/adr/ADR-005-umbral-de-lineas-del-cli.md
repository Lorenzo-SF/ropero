# ADR-005 · El umbral de líneas del CLI

**Estado**: aceptada
**Fecha**: 2026-11-14

## Contexto

R-ARQ-1 fija que `ropero-cli` es un **dispatcher**: la lógica vive en
`ropero-core` (dominio puro) y en `ropero-runtime` (la E/S). El test de
arquitectura (`crates/ropero-core/tests/architecture.rs`) medía ese
principio con un umbral de **400 líneas** de `crates/ropero-cli/src`.

El umbral se fijó cuando el CLI v2 tenía **siete** comandos y un parser
**escrito a mano**:

| Momento | Invocaciones | Líneas del CLI |
|---|---:|---:|
| F1 (antes de este ciclo) | 7 | 398 |
| F3 (después de este ciclo) | 12 | 770 |

El contrato normativo del CLI v2
([`docs-especificacion/ropero-cli.md`](../../docs-especificacion/ropero-cli.md))
amplía la superficie de **siete** invocaciones a **doce**: los once
comandos de §6-§16 más la invocación `ropero <modelo>` de §8, que trae
**seis** flags propios (`--force`, `--cpu`, `--port`, `--api-key`,
`--no-api-key`, `--ctx`).

En este ciclo la CLI pasa además de un parser escrito a mano a **`clap`**
con derive y con el **builder**, que es lo que permite que el contrato de
los flags viva **en el dominio** (`ropero_core::surface`) y no duplicado en
la CLI. El coste de esa corrección son **151 líneas de código** (239 con
comentarios) en `parser.rs`: la traducción de la tabla de `surface` a
`clap`, con sus plantillas de ayuda en español y sus flags con valor,
repetibles y con requisitos.

## Decisión

El umbral de `ropero-cli` pasa de **400** a **800** líneas.

El umbral **sigue existiendo** y sigue midiendo lo mismo: que el CLI sea un
dispatcher y no un sitio donde viva la lógica. Lo que cambia es su
calibración, porque la superficie del contrato creció y el parser pasó de
escrito a mano a generado.

### Por qué 800 y no 400

El cálculo, medido sobre el código de este ciclo (no estimado):

| Fichero | Líneas | Qué es |
|---|---:|---|
| `parser.rs` | 239 | la traducción del contrato (`surface`) a `clap` |
| `commands.rs` | 265 | los nueve comandos que hacen su trabajo |
| `summary.rs` | 70 | `ropero` a secas y el arranque de un modelo |
| `main.rs` | 100 | el dispatcher y los códigos de salida |
| `discovery.rs` | 96 | la frontera entre la CLI y el runtime |
| **total** | **770** | |

Sin comentarios ni líneas vacías son **529** líneas de código. El suelo
real del CLI con esta superficie está entre 500 y 550 líneas: **cualquier
umbral por debajo de 600 hace imposible el contrato**.

Las **370** líneas que hay entre 400 y 770 no son lógica de negocio: son
la traducción de la superficie, el pegamento entre dominio y E/S, y los
comentarios que explican por qué cada cosa está donde está.

### Por qué no bajar el número de otra manera

Se intentó, y se documenta lo que se intentó para que no se repita:

1. **Mover la lógica al dominio.** Se hizo: la foto de la máquina
   (`snapshot.rs`), la paleta (`palette.rs`), el listado (`listing.rs`), el
   `status` (`status.rs`), el `doctor` (`doctor.rs`) y toda la superficie
   (`surface.rs`) viven ahora en `ropero-core`. La CLI quedó en 529 líneas
   de código.
2. **Mover la E/S al runtime.** Se hizo: el escaneo del catálogo y la
   detección de instalación pasaron a `ropero-runtime`
   (`catalog_scan.rs`), y la captura de la máquina a `machine.rs`.
3. **Usar `clap` con derive en vez del builder.** Se probó: el derive
   obliga a **duplicar** el contrato de los flags (una vez en `#[arg(...)]`
   y otra en `surface`), y además no sabe expresar `ropero <modelo>`
   (un token que puede ser comando o modelo) sin `external_subcommand`, que
   **no parsea** los flags. El builder, alimentado desde `surface`, es más
   corto y no duplica nada.
4. **Fusionar ficheros.** Se hizo: nueve ficheros de veinte líneas se
   convertiron en uno. No cambia el total: el umbral mide líneas, no
   ficheros.
5. **Contar sólo el código (sin comentarios ni vacías).** Da 529, y sigue
   por encima de 400.

Lo que **no** se hizo, y por qué:

- **No** se movió el CLI a un crate aparte para "que el contador no lo
  vea". Eso sería Falsejar el métrico: el código seguiría siendo el mismo,
  sólo en otro sitio. `ropero-cli` es el binario, y el binario es lo que
  mide el test.
- **No** se rebajó la calidad (quitar la ayuda en español, la
  localización de las cabeceras de `clap`, o los comentarios) para entrar
  en 400.

### El límite que el umbral sigue poniendo

Con 800 líneas el test sigue **prohibiendo** que el CLI se llene de
lógica. Para que eso sea cierto, el umbral va acompañado de dos reglas que
**no** se negocian:

- `ropero-core` no hace E/S (ya lo comprueba el mismo test).
- El CLI **no** decide nada: compone con funciones del dominio y las
  imprime. Si un comando necesita un `match` con reglas de negocio, ese
  `match` está en el sitio equivocado.

## Alternativas consideradas

- **Subir el umbral a 1000.** Da holgura de sobra, pero deja de ser un
  límite: con 770 líneas, un margen de 230 es el que hace que el test
  duela cuando se acumula código de más. 800 es el umbral que hace
  falta *y* el que sigue mordiendo.
- **Dejar el umbral en 400 y mover la superficie del contrato al runtime.**
  Descartado: la superficie es dominio (es lo que la ayuda anuncia), y
  meter `clap` en el runtime invierte la dirección de las dependencias.
- **No medir el CLI en líneas, sino en "funciones públicas"**. Interesante,
  pero cambiaba la métrica en lugar de calibrarla, y un contador de
  funciones es más fácil de Falsejar que uno de líneas.

## Consecuencias

- El test `ropero_cli_under_400_lines` pasa a `ropero_cli_under_800_lines`,
  con un mensaje que dice el número real.
- El CLI del ciclo F3 se queda en **770** líneas, con **30** de margen. La
  siguiente fase (F3, el arranque del modelo y el runtime) **no** puede
  añadir comandos al CLI sin pasar por aquí: si el arranque necesita más
  de 30 líneas de pegamento, lo que hay que mover es el pegamento al
  runtime, no el contrato.
- La decisión de que la superficie del contrato viva en
  `ropero_core::surface` (y el parser sea una traducción de ella) es la
  que hace que el CLI crezca **linealmente** con el número de flags, no
  exponencialmente: añadir un flag al contrato es añadir **una** línea a
  `surface.rs` y **cero** a la CLI.
