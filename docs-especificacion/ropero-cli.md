# ropero — Especificación del CLI v2

**Estado**: normativa. Dada por el dueño el **2026-11-14**. Prevalece sobre la
familia `R-CLI` del [doc A](ropero-especificacion.md#44-la-línea-de-comandos) en
lo que la contradiga.

Este documento es un **contrato**, no una guía de estilo: transcribe la
especificación del CLI tal como la dio el dueño. Donde la especificación fue
ambigua, la ambigüedad queda registrada en [§17](#17-pendientes-de-decisión) y
**no** se ha resuelto aquí por decisión propia.

---

## 1. Alcance

Define la superficie pública del binario `ropero` v2:

- Los once comandos (o invocaciones) que el usuario ve.
- Los códigos de salida.
- La leyenda de motores con sus colores.
- Las decisiones de diseño que el dueño ha tomado y que este documento fija.

No define: el formato interno del TOML (doc A §4.2), el contrato de motor
(doc A §4.3), el ciclo de vida del runtime (doc A §4.5) ni el protocolo del
gateway (doc A §4.7). Cuando algo de esos términos aparece aquí, es sólo
como **nombre de una opción**.

## 2. Estética (normativa)

El CLI debe ser **nuevo, fresco, claro y sencillo de usar**.

**No se copia nada del ropero antiguo.** Ni la estructura de la ayuda, ni los
textos, ni el orden de las secciones, ni los nombres de los flags, ni el
formato de los listados. Lo único que se hereda de la v1 es el **concepto** de
las cosas (un motor, un modelo, un TOML), nunca su forma.

Consecuencia práctica: un `ropero --help` de la v2 no debe ser reconocible
como el `ropero --help` de la v1.

### 2.1 El sistema de formato (normativa)

**Todas** las salidas del CLI se componen con el **mismo** sistema de formato:
el resumen, `list`, `engines`, `status`, `doctor`, `install` y los errores.
No es una guía de estilo: es un contrato. Lo que no encaja en él, no sale.

El sistema vive en el dominio (`ropero_core::estilo`) y sus reglas son:

| Regla | Qué obliga |
|---|---|
| **Ancho visible** | El ancho de una columna es el que ocupa el texto **en la pantalla**: los códigos ANSI no cuentan, los caracteres anchos (CJK, emojis) valen **dos** y los acentos combinantes, ninguno. |
| **Alineación** | Los datos **numéricos** (tamaños, puertos, memoria) se alinean a la **derecha**; los de **texto**, a la **izquierda**. El título de una columna se alinea como sus datos. |
| **Recorte** | El texto que no cabe en su columna se **recorta** con `…`: nunca se parte en dos líneas. El ancho máximo de una columna es de **36** columnas visibles. |
| **Cabecera** | Una tabla lleva **una** cabecera (los títulos de sus columnas) y una **regla** de `─` debajo. La cabecera sale **una sola vez** por tabla: los grupos se separan con una regla, no repiten la cabecera. |
| **Grupos** | Un grupo (un motor) es una **fila de grupo**: una fila que ocupa toda la tabla, con su nombre en el color de la leyenda ([§4](#4-leyenda-de-motores)). Lo que va detrás del nombre (el estado del motor) va **fuera** del color. |
| **Notas** | Lo que va debajo de una fila (el motivo de una incompatibilidad, el nombre de una GPU) es una **nota**: una línea más indentada, que no rompe la rejilla. |
| **Rótulos** | Un listado de pares (`clave: valor`) alinea las **claves**: el valor de cada par empieza siempre en la misma columna. |
| **Títulos** | El título de una sección va en el **margen** y en **mayúsculas**; su cuerpo, un nivel más dentro. La sangría es de **dos espacios** por nivel. |
| **Color** | El color es el de la leyenda de motores ([§4](#4-leyenda-de-motores)) y **sólo** se usa en el **nombre del motor**. Ningún título, ninguna cabecera y ningún dato llevan color. |
| **Tamaños** | El tamaño de un artefacto es el de su descarga: **aproximado** (`~`) y con **dos decimales** (`~18.90 GB`). Lo que no se sabe **no se inventa**: sale `—`. |
| **Artefactos** | La columna del artefacto dice **qué** se baja, y lo dice el **formato** (R-MOD-6): el fichero (`gguf`), el directorio del checkpoint (`safetensors`), el **tag** del almacén (`q4nx`) o el paquete del catálogo (`strata`). Un modelo cuyo artefacto es un tag dice **el tag** (venga de `[model].path` o de `--decision`), nunca «no disponible»; si el tag no se sabe, dice la **clase** (`tag`). |
| **Final de línea** | Ninguna línea lleva **espacios al final**: rompen el diff y descolocan al copiar. |

Hay **dos** excepciones, y sólo dos:

- `ropero path` es una **exportación** (`CLAVE=valor`), no una tabla: se
  pasa a un `eval` o se lee con un `grep`.
- `ropero args` y `ropero levels` imprimen **un valor por línea**, sin
  cabecera: son datos para leer, no tablas.

### 2.2 La forma de cada salida

| Salida | Cómo se compone |
|---|---|
| `ropero` (a secas) | Secciones `MOTORES` y `MODELOS`, con los rótulos alineados. |
| `ropero list` | Sección `MODELOS` con una tabla: `modelo`, `tamaño`, `artefacto` (y `estado` con `--all`). El motor es una fila de grupo, en su color. |
| `ropero engines` | Sección `MOTORES` con una tabla: `motor`, `color`, `madurez`, `puertos`, `formatos`. El motivo de la incompatibilidad es una nota. |
| `ropero status` | Sección `ESTADO DE LA MÁQUINA` con dos tablas: la de los procesadores y la de la memoria. |
| `ropero doctor` | Secciones `DEPENDENCIAS DE ROPERO` y `DEPENDENCIAS DE LOS MOTORES`, con una tabla de `dependencia`, `estado` y `qué hacer`. El motor es una fila de grupo y una regla separa un motor del siguiente: la cabecera **no** se repite por motor. |
| `ropero install` | Los menús son tablas: la del motor, con su `nº`; la del modelo, con su `nº`, `tamaño` y `artefacto`. |

Un ejemplo, el listado:

```text
MODELOS
  modelo         tamaño  artefacto
  ───────────────────────────────────────────────────────────
  fastflowlm
  d3mini       ~3.50 GB  d3:mini
  ───────────────────────────────────────────────────────────
  llama-server
  groot-o     ~13.60 GB  Groot-19B-A3B-261023-NVFP4.gguf
  nemotron-o  ~18.90 GB  NVIDIA-Nemotron-3.5-Lightning-30B-A…
```

Y el doctor, con **una** cabecera por sección:

```text
DEPENDENCIAS DE LOS MOTORES
  dependencia  estado  qué hacer
  ────────────────────────────────────────────────────────
  llama-server (instalado)
  git          ok
  cmake        ok
  ────────────────────────────────────────────────────────
  vllm (no instalado)
  uv           falta   instálalo con tu gestor de paquetes
```

---

## 3. Códigos de salida

| Código | Significado |
|---:|---|
| `0` | Ok |
| `1` | Error |
| `2` | Uso (error de uso: argumento o comando inválido) |
| `130` | Ctrl-C |

Estos cuatro son los únicos.

## 4. Leyenda de motores

Esta tabla es la referencia canónica de los motores del CLI v2: colores,
descripción y formatos soportados. La nomenclatura de esta tabla es la que
gobierna, y queda fijada en
[ADR-004](../docs/adr/ADR-004-nomenclatura-de-motores.md).

| Engine | Color | Qué es | Formatos |
|---|---|---|---|
| `llama-server` | naranja | llama.cpp (repo oficial) | GGUF (nativo) |
| `turbo-llama` | amarillo | llama.cpp (fork de la cuenta del dueño, actualmente privado) | GGUF (nativo) |
| `flm` | rojo | FastFlowLM (sólo Linux, sólo CPU AMD compatible) | Q4NX (nativo NPU AMD XDNA2), GGUF (entrada compatible con determinados backends y conversiones) |
| `mlx` | gris | MLX-LM + mlx-server (sólo macOS ARM) | Safetensors estructura MLX (nativo), cuantizados MLX (4-bit, 8-bit…) |
| `airllm` | blanco | único engine sin gateway; ropero integra uno | Safetensors, PyTorch (.bin); compresión propia 4-bit y 8-bit |
| `tensorrt-llm` | verde pistacho | TensorRT-LLM (sólo Linux, sólo dGPU RTX) | Safetensors, .bin/.pth, checkpoints TRT y .engine; FP16/BF16/FP8/INT4 AWQ… |
| `vllm` | morado | vLLM (Linux y macOS) | Safetensors (principal), .bin, GGUF (experimental), AWQ/GPTQ/BitsAndBytes |
| `strata` | verde pálido | Strata (Linux y macOS) | GGUF (nativo); descargar/preparar/tunear/usar modelos externos con `groot.sh`, `qwen-abliterated.sh`, `qwen36.sh` de referencia |

Notas de la tabla:

- `airllm` es el **único** motor sin gateway: es ropero quien integra uno.
- `strata` no descarga modelos por sí mismo: la descarga, la preparación y el
  afinado se hacen con los scripts de referencia
  ([`groot.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/groot.sh),
  [`qwen-abliterated.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen-abliterated.sh),
  [`qwen36.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen36.sh)),
  que viven en la raíz del repo.

## 5. Decisiones de diseño fijadas

Estas decisiones las toma el dueño y este documento las fija para que no se
reabran.

### 5.1 Rangos de puertos por motor

Por defecto, **cada motor tiene un rango de puertos asignado**, para poder
tener varios motores sirviendo a la vez sin que se pisen.

| Engine | Rango de puertos |
|---|---|
| `llama-server` | `8100-8199` |
| `turbo-llama` | `8200-8299` |
| `mlx` | `8300-8399` |
| `flm` | `8400-8499` |
| `tensorrt-llm` | `8500-8599` |
| `vllm` | `8600-8699` |
| `airllm` | `8700-8799` |
| `strata` | `9900-9999` |

El rango de `strata` (`9900-9999`) **incluye el `9999`** que usan por defecto
los scripts de referencia (`groot.sh`, `qwen-abliterated.sh`, `qwen36.sh`), de
modo que un motor lanzado por ropero y uno lanzado a mano con esos scripts no
se pisan por defecto.

### 5.2 Códigos de salida de la invocación mínima

- `ropero` sin argumentos **sale con `0`**: es una ayuda breve, no un error.
- Un **comando desconocido sale con `2`**.

### 5.3 No copiar la v1

Ver [§2](#2-estética-normativa). El CLI v2 se escribe desde cero.

---

## 6. `ropero` (sin argumentos)

Muestra una **ayuda breve**, no la ayuda completa.

Contenido de esa ayuda breve:

1. **Nombre** del programa.
2. **Build**: versión, fecha de compilación.
3. **Autor**.
4. **Engines disponibles** (los que se pueden instalar en esta máquina) e
   **engines instalados**.
5. **Modelos disponibles** (los TOML del catálogo) e **modelos instalados**
   (los descargados).
6. **Sugerencia** de `ropero --help`.

Sale con `0` ([§5.2](#52-códigos-de-salida-de-la-invocación-mínima)).

## 7. `ropero --help`

Muestra una **ayuda breve** más la **lista de comandos**, con una frase por
comando.

Al final, **sugiere** `ropero <comando> --help`.

## 8. `ropero <modelo>`

Arranca el modelo.

### 8.1 Precondiciones

- El **engine** del modelo está **instalado**.
- El **modelo** está **instalado** (descargado).
- Hay **recursos disponibles**:
  - Sin flags: **VRAM suficiente** para el modelo.
  - Con `--cpu`: **RAM disponible** (el modelo se sirve desde RAM).

Si una precondición no se cumple, es **error de ejecución** (código `1`) y el
mensaje dice cuál de las tres falla.

### 8.2 Flags

| Flag | Qué hace |
|---|---|
| `--force` | Si **otros procesos lanzados por ropero** ocupan el device, **matarlos** y cargar lo pedido. |
| `--cpu` | Fuerza el **offload 100 % a CPU/RAM**, si el engine lo permite. **Sólo los llama-servers** lo soportan. |
| `--port` | Fuerza el **puerto**. Por defecto el puerto se elige dentro del rango del engine ([§5.1](#51-rangos-de-puertos-por-motor)). |
| `--api-key` | Fija la clave de API del gateway. |
| `--no-api-key` | Arranca **sin** clave de API. |
| `--ctx` | Fuerza el **contexto**. |

`--cpu` en un engine que no lo soporta es **error de uso** (código `2`).

## 9. `ropero install [<nada>|engines|models]`

| Invocación | Qué hace |
|---|---|
| `ropero install` | Instala **engines y luego models**, en ese orden. |
| `ropero install engines` | Instala engines. |
| `ropero install models` | Instala models. |

### 9.1 `ropero install engines [<lista>]`

- **Sin lista**: muestra el **listado** de engines disponibles y **espera
  selección**.
- **Con lista**: instala **directamente**, sin preguntar. Es el modo
  **automatizable**.

La lista acepta **ids o alias**, separados por **espacios o comas**.

### 9.2 `ropero install models [<lista>]`

- **Sin lista**: muestra el listado de modelos y **espera selección**.
- **Con lista**: instala **directamente**, sin preguntar. Es el modo
  **automatizable**.

La lista acepta **ids o alias**, separados por **espacios o comas**.

### 9.3 El flag `--prio`

El contrato de `--prio` es el del
[doc B §4.10.7](ropero-construccion.md#4107-el-contrato-de---prio), que es la
fuente **normativa** (el [doc A §4.10.6](ropero-especificacion.md#4106-la-decisión-de-descargar),
R-MOD-29, remite a él):

- `--prio download|build` es la forma de **elegir sin preguntar** cuando un
  modelo declara **las dos vías** (`[download]` y `[build]`).
- Puede ser **global** (para todos los modelos de la invocación) o **por
  modelo**.
- El **global** va **antes** de la lista de modelos; el **por modelo** va
  **detrás** del modelo y **hasta el siguiente modelo**.
- Si hay un `--prio` **global** y otro **por modelo**, **manda el por modelo**
  para ese modelo.
- Si la prioridad pedida **no está disponible** para ese modelo, se **ignora**
  si el modelo no tiene ese camino, o si la prioridad ya **es su defecto**.
- Un valor que no sea `download` ni `build` es **error de uso (exit 2)**, con
  el valor recibido en el mensaje.

## 10. `ropero stop [<nada>|<modelo>]`

| Invocación | Qué hace |
|---|---|
| `ropero stop` | Mata **todos** los engines lanzados por ropero. |
| `ropero stop <modelo>` | Mata **sólo** el engine de ese modelo. Si hay **más modelos en el mismo engine**, **se respetan**. |

## 11. `ropero gguffer`

Descarga un repo de HuggingFace **o** usa **pesos locales** (safetensors /
gguf) y **crea un GGUF nuevo**, **cuantizado o no**.

### 11.1 La cuantización no es obligatoria

**No todos los GGUFs requieren cuantizar.** A veces los safetensors ya vienen
cuantizados; a veces el repo fuente ya es el **F16/BF16** que hay que
cuantizar. El comando debe detectarlo y **saltarse** la fase de cuantización
cuando el resultado pedido ya es el de fábrica.

### 11.2 Flags

| Flag | Qué hace |
|---|---|
| `--repo OWNER/NAME` | Repo de HuggingFace de origen. |
| `--path DIR` | Directorio de pesos **locales** de origen. |
| `--quant TIPO[,TIPO...]` | Tipo(s) de cuantización. **Repetible**. |
| `--imatrix FICHERO` | Matriz de importancia local. |
| `--imatrix-repo OWNER/NAME` + `--imatrix-file NOMBRE` | Matriz de importancia remota. **Van siempre en pareja.** |
| `--out NOMBRE\|RUTA` | Con **1 quant**: nombre o ruta del **1** GGUF. Con **lista** de quants: **directorio** de salida. |
| `--intermedio f16\|bf16` | Formato intermedio. **Defecto: `f16`**. |
| `--keep-source` | No borra los pesos de origen. |
| `--keep-intermedio` | No borra el intermedio. |
| `--no-mtp` | Genera el GGUF **sin cabeceras MTP**. |
| `--no-dflash` | Genera el GGUF **sin cabeceras DFlash**. |
| `--force` | Sobrescribe / fuerza. |
| `--list-quants` | Lista los tipos de cuantización disponibles. |
| `--diagnostico` | Modo diagnóstico: enseña qué haría, sin escribir nada. |

`--imatrix-repo` y `--imatrix-file` **van en pareja**: usar sólo una de las
dos es **error de uso** (código `2`).

### 11.3 Precondición

El engine **`llama-server`** (el oficial) debe estar **instalado y compilado**.

### 11.4 Referencia para Strata

Los GGUFs que usa Strata se preparan con los scripts de la raíz:
[`groot.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/groot.sh),
[`qwen-abliterated.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen-abliterated.sh)
y [`qwen36.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen36.sh).

## 12. `ropero status`

Vista tipo `nvidia-smi` del estado de la máquina.

| Recurso | Qué muestra |
|---|---|
| **CPU** | GHz y temperatura. |
| **dGPU** | GHz y temperatura. En **macOS** la única GPU se muestra aquí. En **Linux** sólo si la hay. |
| **iGPU** | GHz y temperatura. **Sólo Linux**, y sólo si la hay. |
| **RAM/SWAP** | MB **en uso**. Hay que **tener en cuenta zram** en CachyOS. |

## 13. `ropero test`

De los **modelos disponibles** (engine instalado, modelo descargado), lanza
**pruebas de rendimiento y respuesta**.

La batería de pruebas vive en [`docs/tests/`](../docs/tests/) y su índice es
[`docs/tests/indice.md`](../docs/tests/indice.md). Ese índice define, **test a
test**:

- qué mide,
- cómo se usa,
- cómo se interpreta,
- y cómo se usan **por grupo**.

El comando debe ofrecer **lanzar por tipo, por grupo, uno solo, o todos**.

## 14. `ropero doctor`

Chequea:

1. Las **dependencias del propio ropero**, incluido que **ropero esté
   instalado y en el PATH**.
2. Por **cada engine** — disponible o no — **sus dependencias**, para ver qué
   está **OK** y qué se **puede instalar**.

## 15. `ropero list`

Lista **sólo** los **modelos disponibles** (engine instalado, modelo
descargado), **agrupados por engine**, con la **leyenda de colores**
([§4](#4-leyenda-de-motores)).

Con `--all` añade los modelos **no disponibles** (con la columna `estado`) y
despliega los grupos plegados. La salida es la tabla de
[§2.2](#22-la-forma-de-cada-salida): **una** cabecera, el motor como **fila de
grupo** y una **regla** entre grupos. El artefacto de un modelo que es un tag
del almacén (`q4nx`) es **el tag**, nunca «no disponible».

## 16. `ropero template`

Imprime el **TOML template completo** con **comentarios de uso**.

---

## 17. Pendientes de decisión

Puntos que la especificación del dueño dejó abiertos y que **no** se han
resuelto aquí:
| # | Cuestión | Dónde |
|---:|---|---|
| 1 | Nombre exacto de la bandera para `--ctx` y su forma (`--ctx N` vs `--ctx=N`). | [§8.2](#82-flags) |
| 2 | Si `--api-key` y `--no-api-key` pueden coexistir y, si no, cuál gana. | [§8.2](#82-flags) |
| 3 | Forma exacta de `--out` cuando hay **1 quant**: ¿`--out nombre` o también ruta? Y con lista, ¿`--out` es siempre un directorio? | [§11.2](#112-flags) |
| 4 | Si `--imatrix FICHERO` y `--imatrix-repo/--imatrix-file` son excluyentes entre sí o combinables. | [§11.2](#112-flags) |
| 5 | Si `ropero stop <modelo>` mata el engine entero cuando es el **único** modelo que sirve, o sólo la instancia de ese modelo. | [§10](#10-ropero-stop-nadamodelo) |
| 6 | Nombres exactos de los **tipos** de cuantización que acepta `--quant` y de los que lista `--list-quants`. | [§11.2](#112-flags) |
| 7 | Si `ropero test` mide también **rendimiento** (tokens/s, latencia) con su propia rúbrica, o sólo qualitative con la del índice. | [§13](#13-ropero-test) |
| 8 | Formato de salida de `ropero status` (tabla, árbol, JSON) y frecuencia de refresco si lo lleva. | [§12](#12-ropero-status) |
| 9 | **Resuelto**: `ropero list` admite `--all`; añade los no disponibles con su `estado` y despliega los grupos. | [§15](#15-ropero-list) |
| 10 | Comportamiento de `ropero install engines`/`models` **sin TTY** y sin lista (la v1 lo trataba como error de uso). | [§9](#9-ropero-install-nadaenginesmodels) |
| 11 | Si existe un comando `ropero engines` / `ropero models` como alias de `install engines` / `install models`. | [§9](#9-ropero-install-nadaenginesmodels) |
| 12 | Si los flags `--download` / `--build` de la v1 sobreviven en la v2 como camino forzado, o los sustituye `--prio`. El doc A (R-DATA-6) aún los cita; este §9.3 y el doc B §4.10.7 dan el contrato de `--prio`. | [§9.3](#93-el-flag---prio) |

---

## 18. Trazabilidad

| Sección | Origen |
|---|---|
| §2 Estética | Decisión del dueño, 2026-11-14 |
| §2.1 Sistema de formato | Implementación del sistema de formato de la salida (`ropero_core::estilo`), 2026-11-15 |
| §3 Códigos de salida | Decisión del dueño, 2026-11-14 |
| §4 Leyenda de motores | Decisión del dueño, 2026-11-14 |
| §5 Decisiones fijadas | Decisiones del dueño, 2026-11-14 |
| §6-§16 Comandos | Decisión del dueño, 2026-11-14 |
| §9.3 `--prio` | [Doc B §4.10.7](ropero-construccion.md#4107-el-contrato-de---prio) (contrato normativo; el doc A, R-MOD-29, remite a él) |
| §13 Batería de tests | [`docs/tests/indice.md`](../docs/tests/indice.md) |
| §17 Pendientes | Ambigüedades de la especificación del dueño, sin resolver |
| §11.4 Scripts de Strata | [`groot.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/groot.sh), [`qwen-abliterated.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen-abliterated.sh), [`qwen36.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen36.sh) |
| §4, §5.1 Nomenclatura y puertos | [ADR-004](../docs/adr/ADR-004-nomenclatura-de-motores.md) |
