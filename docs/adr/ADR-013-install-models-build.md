# ADR-013 · Instalar modelos por la vía `[build]`

**Estado**: aceptada para Ropero v2  
**Fecha**: 2026-11-16

## Contexto

`ropero install models` ya decide entre `[download]` y `[build]` con
`install::elegir_via`, pero `Via::Build` todavía informa que la vía está
pendiente (`crates/ropero-cli/src/install.rs`). La fabricación de GGUF ya vive
en `ropero-forge`; la descarga normal y de auxiliares vive en
`ropero-download`.

La especificación exige que `[build]` sea la receta del gguffer (R-DATA-5 y
R-GGUF-34) y que las rutas de los artefactos coincidan con las que usa el
arranque (`resolve_paths`, R-MOD-6). La implementación debe conectar estas
piezas sin fabricar un artefacto que Ropero luego no reconozca ni arrancar un
segundo flujo de descarga del GGUF.

## Decisión

### 1. Selección y responsabilidades

- La selección de vía actual sigue siendo la única decisión de producto:
  modelos con ambas tablas requieren `--prio build|download` o la pregunta
  existente; modelos con sólo `[build]` entran directamente por esta vía.
- `ropero-cli` adapta el modelo seleccionado y presenta el resultado.
  `ropero-forge` conserva la validación, planificación, conversión,
  cuantización, verificación y procedencia. `ropero-download` conserva la
  responsabilidad de instalar artefactos auxiliares descargables.
- La interfaz de Forge para una receta debe ser un tipo de dominio reusable
  (`BuildSpec` o equivalente), no una invocación del subcomando CLI ni una
  dependencia de Forge sobre el dispatcher. El API manual de `ropero gguffer`
  conserva su contrato actual.

### 2. Traducción normativa de `[build]`

La capa adaptadora transforma los campos de `ropero_core::Build` a una
petición tipada para Forge conforme a R-DATA-5/R-GGUF-34:

| Campo TOML | Semántica al instalar por build |
|---|---|
| `repo` | Repositorio fuente de Hugging Face. Es obligatorio salvo que la API futura admita explícitamente una fuente local. |
| `gguf = false` | El repo contiene pesos que deben convertirse a GGUF. |
| `gguf = true` | El repo contiene un GGUF publicado; se coloca o cuantiza según el quant pedido, sin conversor de pesos. |
| `gguf_file` | Nombre del GGUF fuente dentro del repo cuando `gguf = true`; si falta, se aplica el patrón/nombre que ya define Forge. No se interpreta como nombre del artefacto final. |
| `quant` | Quant(s) de destino. Se admiten las formas normativas: lista y cadena separada por comas. Sin quant, se fabrica/coloca el formato intermedio previsto por Forge. |
| `imatrix` | Ruta local del importance matrix. Es excluyente con la pareja remota. |
| `imatrix_repo` + `imatrix_file` | Fuente remota de la importance matrix; ambos se declaran juntos. |
| `out` | Restricción declarativa de la ruta final esperada; debe ser coherente con el destino runtime canónico descrito en §3. |
| `out_principal` | Nombre declarado del GGUF principal entre varias salidas; debe coincidir con el fichero runtime canónico. |
| `from_subdir` | Subdirectorio relativo del repo descargado donde buscar pesos. No cambia el destino de salida. |
| `include` | Patrones de ficheros que se solicitan al descargar el repo fuente. No pueden excluir metadata o ficheros requeridos para la conversión. |
| `intermedio` | `f16` o `bf16`, conforme a Forge. |
| `keep_source` | Pide a Forge conservar los pesos fuente. |
| `keep_intermedio` | Pide a Forge conservar el intermedio. |
| `no_mtp` | Pide a Forge fabricar sin el artefacto/cabeceras MTP que controla el conversor. |
| `force` | Pide rehacer la tirada aunque haya salida/checkpoint reutilizable. No equivale a un force de instalación de engines. |
| `size` | Informativo para mensajes y planificación; no se usa como tamaño medido ni modifica la forja. |

`Build.gguf` es un booleano mientras que el `Flags.gguf` manual de Forge es
`Option<String>` y representa una fuente escrita por el usuario (`Fuente::Gguf`).
No se asigna el booleano a ese campo ni se codifica `repo`/`gguf_file` en una
cadena ambigua. La interfaz reusable de Forge debe expresar explícitamente el
tipo de fuente, por ejemplo:

- pesos de HF: `{ repo }` → conversión;
- GGUF publicado: `{ repo, file }` → descarga del fichero seleccionado y
  colocación/cuantización;
- path local, si la API lo ofrece: directorio de pesos o fichero GGUF, con
  tipo explícito.

La adaptación mantiene `Flags.gguf` para el modo manual existente y añade la
capacidad tipada necesaria a Forge (`gguf_file`, `from_subdir`, `include` y
fuente GGUF remota de fichero concreto). El schema actual también diverge en
dos tipos: `Build.quant` es `String`, aunque la plantilla y R-GGUF-34 admiten
lista o cadena separada por comas; y `Build.from_subdir` es `bool`, aunque la
plantilla describe un subdirectorio. La implementación debe aceptar ambas
formas de quant y convertir `from_subdir` en un subdirectorio relativo
opcional. Un `from_subdir = true` antiguo no contiene información suficiente
para localizar pesos y debe fallar con un diagnóstico que indique corregir el
TOML; no se debe adivinar un nombre de directorio.

### 3. Destino runtime canónico

- Para un modelo GGUF, `resolve_paths(spec, env).gguf` es el destino principal
  obligatorio. Incluye `GGUF_DIR` efectivo, el directorio propio del modelo,
  `[files].gguf` y el override runtime de cuantización si existe. Forge recibe
  ese destino como salida principal explícita; las salidas de quants
  adicionales quedan en el mismo directorio.
- Se exige `[files].gguf` y la ruta canónica no puede quedar vacía. El build
  sólo se considera instalado si `Informe.salidas` contiene el destino
  canónico y el fichero ha pasado la verificación de Forge.
- Si `[build].out` declara una ruta exacta distinta, la receta se rechaza con
  un error que muestra ambas rutas. No se deja el GGUF en `out` mientras el
  arranque busca otro fichero.
- Si `[build].out_principal` declara un nombre distinto de
  `basename(resolve_paths(...).gguf)`, se rechaza antes de descargar. Si
  coincide, la ruta absoluta que se pasa a Forge sigue siendo la canónica.
- El sidecar de procedencia de Forge queda al lado de la salida canónica. No
  se trata como un artefacto que sustituya `[files].gguf`.

La ruta local de `[files].gguf` no se debe reinterpretar como path de pesos
fuente: la fuente está en `[build]`; el campo `[files]` describe qué buscará
el runtime después de fabricar.

### 4. Preflight, ejecución y auxiliares

El orden observable de la vía es:

1. validar receta, quants, fuente, conflictos de imatrix y coherencia de
   destinos sin descargar pesos ni auxiliares;
2. verificar la toolchain llama.cpp/gguffer que necesita esa receta;
3. ejecutar Forge, cuyo propio preflight y checks de viabilidad deben ocurrir
   antes de descargar los pesos fuente o el imatrix;
4. comprobar que la salida canónica existe y está verificada;
5. instalar sólo los auxiliares runtime (`download.aux.*` cuyo `use` efectivo
   es `run`), en sus rutas canónicas de `resolve_paths`;
6. declarar éxito sólo cuando los artefactos runtime exigidos por el modelo
   estén presentes.

La instalación auxiliar reutiliza una API de `ropero-download` que planifica y
descarga **auxiliares solamente**. No llama a `plan_de_descarga`/`descarga` del
principal en la vía build: hacerlo bajaría también el GGUF de `[download]`.
La descarga auxiliar es idempotente; si falla después de fabricar el GGUF, el
comando falla sin borrar el GGUF y una repetición sólo instala lo que falta.

`use = "build"` nunca se instala como auxiliar runtime. Para la imatrix se
elige una sola fuente de Forge:

1. `build.imatrix` local, si se declaró;
2. la pareja `build.imatrix_repo`/`build.imatrix_file`, si se declaró;
3. si las anteriores faltan, `[download.aux.imatrix]` con `use = "build"`,
   pasado a Forge como una fuente remota, no descargado por separado por
   `ropero-download`.

Si hay una fuente explícita en `[build]`, no se descarga además
`download.aux.imatrix`. `ropero-download` sólo maneja esa aux si se clasifica
como `run`, nunca como parte de la fuente de cuantización.

Los auxiliares runtime se guardan junto al directorio del GGUF. Cuando se
declaren `[files].mtp`, `[files].mmproj` o `[files].chat_template`, el destino
de la aux debe coincidir con su ruta de `resolve_paths`; si `[download.aux.*].name`
la contradice, el TOML es inválido para instalación hasta que se corrija. Una
aux sin `[files]` correspondiente puede seguir guardándose por su nombre si
el contrato existente de runtime la resuelve, como ocurre con el chat template.

## Decisiones por ausencia de detalle

- `Build.gguf = true` y `gguf_file` se interpretan como descarga de un fichero
  GGUF concreto del repo fuente. El nombre de salida continúa gobernado por
  `[files].gguf` y `resolve_paths`.
- `Build.out` no puede redirigir silenciosamente la instalación fuera de la
  ruta runtime. La norma de la plantilla (“ruta exacta”) se conserva como una
  aserción de coherencia; el destino efectivo lo fija el catálogo runtime.
- Si `Build.gguf = true` y no hay `gguf_file`, Forge puede usar su patrón de
  búsqueda sólo si el repo contiene una única fuente GGUF no ambigua. Si hay
  varias candidatas, debe fallar y pedir `gguf_file`; no escoger una al azar.
- Un modelo build-only no tiene `[download]`, por lo que tampoco puede declarar
  `[download.aux]` en TOML. Esta decisión no añade una tabla de auxiliares
  nueva; esa necesidad requiere una especificación posterior.

## Dependencias de implementación

1. Extender la API reusable de `ropero-forge` con un input de build tipado que
   acepte fuente HF/GGUF, fichero GGUF fuente, subdirectorio, includes,
   quant(s), imatrix y salidas. Mantener el API de Flags del comando manual.
2. Corregir/expandir el schema `ropero_core::Build` para `quant` lista o
   cadena y `from_subdir` relativo opcional, y validar las combinaciones de
   campos antes de I/O.
3. Exponer el plan/operación de auxiliares runtime en `ropero-download` sin
   incluir ni descargar el artefacto principal.
4. Conectar `Via::Build` en `ropero-cli/src/install.rs`: resolver destino,
   invocar Forge, comprobar la salida y después instalar auxiliares runtime.
5. Mantener errores de configuración (ruta incoherente, campos incompletos,
   imatrix duplicado/incompatible) antes de cualquier descarga.

## Criterios de aceptación

- Un modelo sólo-build puede fabricarse y quedar disponible para arrancar en
  la ruta exacta de `resolve_paths(...).gguf`.
- Un modelo con ambas vías respeta la elección existente `--prio` o el menú;
  elegir build no descarga el GGUF declarado en `[download]`.
- La fuente de pesos safetensors y la fuente GGUF publicada son caminos
  distintos y comprobables; `gguf_file` selecciona el fichero GGUF fuente.
- `from_subdir`, `include`, uno o varios quants, `intermedio`, `keep_*`,
  `no_mtp` y `force` llegan a Forge con la semántica de R-GGUF-34.
- Un destino explícito incoherente falla antes de bajar datos. Falta de
  toolchain o de imatrix requerida también falla antes de bajar pesos.
- Una receta con GGUF final ya existente puede informar estado válido conforme
  a la procedencia/checkpoint de Forge; con `force = true` la rehace.
- Los auxiliares `use=run` se descargan después de fabricar el principal,
  usando sus repos/ficheros/nombres, sin repetir la descarga del GGUF ni
  duplicar el imatrix `use=build`.
- Si la descarga de un auxiliar falla, el GGUF verificado se conserva, el
  install sale con error y la siguiente ejecución reanuda desde el auxiliar
  faltante.
- La vía manual `ropero gguffer` conserva su comportamiento y Forge sigue
  siendo invocable sin pasar por la CLI de Ropero.

## Riesgos y límites

- `gguf_file` y los filtros HF `include` exigen soporte explícito en el
  downloader de fuentes de Forge; concatenarlos a `Flags.gguf` no es suficiente.
- `from_subdir` debe aplicarse tanto al análisis de metadata de viabilidad
  como a la conversión; si sólo se aplica al segundo paso, el preflight puede
  evaluar el directorio equivocado.
- El override `ROPERO_<SLUG>_GGUF` puede cambiar el nombre runtime respecto a
  `[files].gguf`. Se usa la ruta resuelta como destino; la incompatibilidad con
  `out`/`out_principal` es error, no una segunda ubicación.
- El sidecar/checkpoint de Forge forma parte de su reanudación y procedencia.
  No debe borrarse como limpieza general del modelo mientras haya una tirada
  reanudable o el sidecar sea la única prueba de la salida.
- Esta ADR cubre únicamente `format = "gguf"`. La fabricación de Safetensors,
  Q4NX, Strata u otros formatos no se infiere a partir de `[build]`.
