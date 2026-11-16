# ropero — Especificación del CLI v2

**Estado**: normativa. Dada por el dueño el **2026-11-14**. Prevalece sobre la
familia `R-CLI` del [doc A](ropero-especificacion.md#44-la-línea-de-comandos) en
lo que la contradiga.

Este documento es un **contrato**, no una guía de estilo: transcribe la
especificación del CLI tal como la dio el dueño. Donde la especificación fue
ambigua, la ambigüedad queda registrada en [§19](#19-pendientes-de-decisión) y
**no** se ha resuelto aquí por decisión propia.

---

## 1. Alcance

Define la superficie pública del binario `ropero` v2:

- Los comandos (o invocaciones) que el usuario ve.
- Los códigos de salida.
- La leyenda de motores con sus colores.
- Las decisiones de diseño que el dueño ha tomado y que este documento fija.

> **Actualización (ADR-008)**: se añade `ropero config` para mostrar rutas y
> origen; el catálogo normal es externo en `<config>/models/<engine>` con
> manifiestos en `<config>/engines/`; no se auto-descubre el repositorio en
> uso normal, pero se conservan los overrides `ROPERO_MODELS_DIR`/`ROPERO_ROOT`
> (incluido el caso en que apuntan al directorio `models/` de una raíz). Los
> logs por defecto viven en `<config>/logs`. El directorio y `config.toml` se
> crean en el primer uso sin sobrescribir. `ropero template` sugiere destino en
> `models/` sin escribir.

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
| **Ancho visible** | El ancho de una columna es el que ocupa el texto **en la pantalla**: los códigos ANSI no cuentan, los caracteres anchos (CJK, emojis) valen **dos** y los acentos combinantes y los modificadores de tono de piel, ninguno. |
| **Alineación** | Los datos **numéricos** (tamaños, puertos, memoria, relojes, temperaturas) se alinean a la **derecha**; los de **texto**, a la **izquierda**. El título de una columna se alinea como sus datos. |
| **Recorte** | El texto que no cabe en su columna se **recorta** con `…`: nunca se parte en dos líneas. El ancho máximo de una columna es de **36** columnas visibles. El **título** de la columna se recorta como sus datos: no desborda la regla. Un tabulador o un salto de línea dentro de una celda se sustituyen por un espacio. |
| **Cabecera** | Una tabla lleva **una** cabecera (los títulos de sus columnas) y una **regla** de `─` debajo. La cabecera sale **una sola vez** por tabla: los grupos se separan con una regla, no repiten la cabecera. |
| **Grupos** | Un grupo (un motor) es una **fila de grupo**: una fila que ocupa toda la tabla, con su nombre en el color de la leyenda ([§4](#4-leyenda-de-motores)). Lo que va detrás del nombre (el estado del motor) va **fuera** del color. |
| **Notas** | Lo que va debajo de una fila (el motivo de una incompatibilidad, el nombre de una GPU) es una **nota**: una línea más indentada, que no rompe la rejilla. |
| **Rótulos** | Un listado de pares (`clave: valor`) alinea las **claves**: el valor de cada par empieza siempre en la misma columna. |
| **Títulos** | El título de una sección va en el **margen** y en **mayúsculas**; su cuerpo, un nivel más dentro. La sangría es de **dos espacios** por nivel. |
| **Color** | El color es el de la leyenda de motores ([§4](#4-leyenda-de-motores)) y **sólo** se usa en el **nombre del motor**. Ningún título, ninguna cabecera y ningún dato llevan color. |
| **Tamaños** | El tamaño de un artefacto es el de su descarga: **aproximado** (`~`) y con **dos decimales** (`~18.90 GB`). Lo que no se sabe **no se inventa**: sale `—`. |
| **Artefactos** | La columna del artefacto dice **qué** se baja, y lo dice el **formato** (R-MOD-6): el fichero (`gguf`), el directorio del checkpoint (`safetensors`), el **tag** del almacén (`q4nx`), el paquete del catálogo (`strata`) o el id de modelo (`ninfer`). |
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
| `ropero list` | Sección `MODELOS` con una tabla: `modelo`, `tamaño`, `artefacto` (y `estado` con `--all`). El motor es una fila de grupo, en su color. Un motor **plegado** dice su motivo debajo del nombre, como una nota. |
| `ropero engines` | Sección `MOTORES` con una tabla: `motor`, `color`, `madurez`, `puertos`, `formatos`. El motivo de la incompatibilidad es una nota. |
| `ropero status` | Sección `ESTADO DE LA MÁQUINA` con dos tablas: la de los procesadores y la de la memoria. |
| `ropero doctor` | Secciones `DEPENDENCIAS DE ROPERO` y `DEPENDENCIAS DE LOS MOTORES`, con una tabla de `dependencia`, `estado` y `qué hacer`. El motor es una fila de grupo y una regla separa un motor del siguiente: la cabecera **no** se repite por motor. |
| `ropero install` | Los menús son tablas con su **sección**: `MOTORES` (la del motor, con su `nº`) y `MODELOS` (la del modelo, con su `nº`, `tamaño` y `artefacto`). El título va en el margen y la tabla, un nivel más dentro. |

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
| `turbo-llama` | amarillo | llama.cpp (fork de la cuenta del dueño, repositorio público) | GGUF (nativo) |
| `flm` | rojo | FastFlowLM (sólo Linux, sólo CPU AMD compatible) | Q4NX (nativo NPU AMD XDNA2), GGUF (entrada compatible con determinados backends y conversiones) |
| `mlx` | gris | MLX-LM + mlx-server (sólo macOS ARM) | Safetensors estructura MLX (nativo), cuantizados MLX (4-bit, 8-bit…) |
| `airllm` | blanco | único engine sin gateway; ropero integra uno | Safetensors, PyTorch (.bin); compresión propia 4-bit y 8-bit |
| `tensorrt-llm` | verde pistacho | TensorRT-LLM (sólo Linux, sólo dGPU RTX) | Safetensors, .bin/.pth, checkpoints TRT y .engine; FP16/BF16/FP8/INT4 AWQ… |
| `vllm` | morado | vLLM (Linux y macOS) | Safetensors (principal), .bin, GGUF (experimental), AWQ/GPTQ/BitsAndBytes |
| `strata` | verde pálido | Strata (Linux y macOS) | GGUF (nativo); descargar/preparar/tunear/usar modelos externos con `groot.sh`, `qwen-abliterated.sh`, `qwen36.sh` de referencia |
| `ignis` | cian | Ignis (Linux x86_64, NVIDIA RTX) | `.ninfer` (artefactos de su catálogo, incluye companions) |

Notas de la tabla:

- `airllm` es el **único** motor sin gateway: es ropero quien integra uno.
- `ignis` se instala desde su release Linux verificado en
  `~/.local/share/ropero/engines/ignis`; `~/.local/bin/ignis-server` apunta al
  binario activo. El catálogo expone Qwen3.8-27B y Qwen3.8-Flash-Next.
  `ropero install models` delega descarga y companions a `ignis-server model
  download`; en 27B el id 16 GB se elige cuando `nvidia-smi` informa una GPU
  de hasta 20 GB. Los artefactos viven por defecto en `~/models/ignis`,
  configurable con `[engines.ignis].artifacts_dir`. El arranque gestionado
  fija `--download-enabled false` y usa lo ya descargado.
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
| `llama-server` (oficial) | `9980-9989` |
| `turbo-llama` (fork) | `9990-9999` |
| `mlx-serve` | `9960-9969` |
| `fastflowlm` | `9970-9979` |
| `tensorrt-llm` | `9950-9959` |
| `vllm` | `9940-9949` |
| `airllm` | `9930-9939` |
| `strata` | `9920-9929` |
| `ignis` | `9910-9919` |

El ropero viejo usaba un puerto base por motor y dejaba diez puertos
descendentes entre bases. Rust conserva esas bases como extremo superior de
rango. `strata` queda en `9920-9929`; `9999` pertenece a Turbo-Llama, igual que
al `llama-server` del ropero viejo.

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
| `--log-level` | Nivel común de log: `error`, `warn`, `info`, `debug` o `trace`; se traduce al control nativo del engine cuando existe. |
| `--background` | Arranca **desacoplado**, sin salida a la terminal (tampoco `ROPERO_PORT`), y guarda los logs bajo `logs_dir`. |

Por defecto, la salida del motor se muestra en la terminal (colorizada cuando procede) y el flujo normal sigue generando el fichero de log del modelo para fallos y estado. Se registra además un log de uso local append-only en `logs_dir` (timestamp, modelo, motor, puerto, resultado y modo), sin API keys ni flags/argumentos sensibles. La cobertura de colorizado es común/semántica; si no hay formato específico por motor se usa este colorizador seguro.

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

- Si el manifiesto declara `[install].source`, clona (solo si el directorio no
  contiene ya un árbol ajeno), configura y compila antes de enlazar el binario.
  Si ya existe un checkout, pregunta si se reutiliza o se borra y clona de
  nuevo. `--force` omite la pregunta, borra el código fuente de cada motor
  seleccionado con receta y realiza una instalación limpia. Sin terminal,
  un checkout existente requiere `--force`.
- Recetas actuales: `llama-server` compila upstream `ggml-org/llama.cpp`;
  `turbo-llama` compila `lorenzo-sf/turbo-llama`. Ambos usan los mismos
  argumentos de compilación por sistema operativo y ninguno aplica parches
  del instalador heredado. Ambos
  enlazan todos los ejecutables generados en `bin`; los del fork usan el
  prefijo `turbo-` (por ejemplo `turbo-llama-server`). Los enlaces se vuelven
  a crear al instalar. Los demás motores no declaran hoy una receta de clonado
  y compilación en el manifest.

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
| `--turbo-llama` | Usa las herramientas compiladas del fork Turbo-Llama; por defecto usa llama.cpp oficial. |

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

1. La **configuración** de ropero (archivo, estado y rutas relevantes).
2. Las **dependencias del propio ropero**, incluido que **ropero esté
   instalado y en el PATH**.
3. Por **cada engine** — disponible o no — **sus dependencias**, para ver qué
   está **OK** y qué se **puede instalar**.

**No modifica artefactos de modelos**. Puede ejecutar el bootstrap idempotente
de configuración (crear directorio y `config.toml` por defecto si faltan, sin
sobrescribir) y tolera configuración ilegible para poder diagnosticarla.

**No modifica artefactos de modelos**. Puede ejecutar el bootstrap idempotente
de configuración (crear directorio y `config.toml` por defecto si faltan, sin
sobrescribir) y tolera configuración ilegible para poder diagnosticarla.

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

Imprime la **plantilla de modelo** (TOML) y sugiere un destino en el
directorio `models/` del catálogo de usuario. **No escribe la plantilla ni
modifica artefactos de modelos**. Si el catálogo no contiene `_template.toml`,
se muestra una plantilla de referencia.

Como parte del arranque normal, puede ejecutarse el bootstrap idempotente de
configuración (crear `<config>/` y `config.toml` por defecto si faltan, sin
sobrescribir).

## 17. `ropero config`

Muestra la configuración resuelta de ropero: rutas efectivas y su origen
(variable de entorno, archivo o valor por defecto). **No modifica artefactos de
modelos**.

El `config.toml` admite las cinco rutas globales y overrides opcionales bajo
`[engines.<id>]`: `models_toml_dir`, `gguf_dir`, `safetensors_dir`,
`port_min`, `port_max` y `color`. La precedencia de rutas es variable global
de entorno, override del engine, campo global del archivo y valor por defecto.
Los overrides de puerto/color sustituyen los valores del manifest. `color`
acepta sólo nombres de la leyenda. Las recetas de instalación, dependencias y
hooks permanecen fijos en los manifests/código; no son configurables.

Puede ejecutar el bootstrap idempotente de configuración en el primer uso
(crear directorio y `config.toml` por defecto sin sobrescribir) para poder
informar del estado real; si el archivo existe pero es ilegible o inválido, lo
reporta sin ocultarlo.

## 18. `ropero <modelo> remove`

Elimina los artefactos **derivables** del TOML y de las rutas del modelo.

- **Sintaxis**: `ropero <alias|modelo> remove`.
- **Confirmación**: antes de borrar pide confirmación interactiva en un TTY.
  Si no hay TTY o la respuesta se cancela/no es afirmativa, **no se borra nada**.
- **Alcance**: sólo elimina ficheros atribuibles al modelo (rutas resueltas de
  `[files]` y auxiliares de runtime declarados). No elimina directorios
  compartidos ni datos no declarados; el directorio concreto del modelo en
  `GGUF_DIR` sólo se retira si queda vacío tras borrar ficheros concretos.
- **Artefactos compartidos**: si otro TOML activo o retirado referencia la
  misma ruta resuelta (o una ruta de checkpoint Safetensors padre/hija que
  solaparía un borrado recursivo, o el mismo tag FLM), se informa de ese modelo
  y se conserva el artefacto solapado. Se pueden eliminar tras confirmación los
  artefactos exclusivos del modelo seleccionado. Compartir el repo de origen
  no cuenta si los destinos instalados son distintos.
- **Safetensors**: `[model].path` identifica el checkpoint que se descargó. El
  plan muestra esa ruta exacta y, tras confirmación, elimina su contenido y el
  directorio del checkpoint; nunca borra la raíz compartida de
  `SAFETENSORS_DIR`, rechaza rutas que salgan de ella y no sigue enlaces
  simbólicos intermedios.
- **FLM**: con motor FLM delegado, elimina cada tag declarado que se instaló
  (modelo principal y auxiliares) con una llamada `flm remove <tag>` por tag.
- **Mensajes**: en español, con salida clara del plan y del resultado.

---

## 19. Pendientes de decisión

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

## 20. Trazabilidad

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
| §18 `ropero <modelo> remove` | Contrato de implementación (eliminación segura de artefactos derivables), 2026-11-15 |
| §19 Pendientes | Ambigüedades de la especificación del dueño, sin resolver |
| §11.4 Scripts de Strata | [`groot.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/groot.sh), [`qwen-abliterated.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen-abliterated.sh), [`qwen36.sh`](https://github.com/Lorenzo-SF/ropero/blob/main/qwen36.sh) |
| §4, §5.1 Nomenclatura y puertos | [ADR-004](../docs/adr/ADR-004-nomenclatura-de-motores.md) |
# Revisión normativa del CLI (2026-11-16)

La especificación posterior y prioritaria está registrada en
`ropero-especificacion.md` §4.14 (R-CLI-18…27, R-CFG-1…2 y R-MOD-32…34).
Este documento debe leerse subordinado a esos requisitos cuando exista
divergencia. En particular, el resumen sin argumentos, `--api-key`/
`--no-api-key`, `--ctx`, instalación interactiva y por lista, la selección de
pruebas por grupo, la elegibilidad de `list`, y la ayuda breve por comando son
parte del contrato objetivo.
