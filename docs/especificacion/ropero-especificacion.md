---
title: "ropero — Especificación y requerimientos"
subtitle: "El contrato que implementa el código, escrito como si se construyera desde cero"
author: "Extraído de la implementación de Lorenzo-SF/ropero @ f21b442"
date: "2026-11-12"
lang: es
---

# ropero — Especificación y requerimientos

[English](ropero-especificacion.en.md) · [Español](ropero-especificacion.md)

> **Contrato funcional objetivo de ropero Rust.** Este documento incorpora la
> revisión del propietario recibida el 2026-11-16. Cuando un requisito previo
> contradiga esa revisión, prevalece esta sección hasta que se integre
> formalmente en las secciones existentes.

---

## 0.4 Especificación posterior del CLI y configuración (2026-11-16)

Esta sección registra la especificación posterior entregada por el propietario.
Es normativa para la implementación Rust. Los puntos aquí marcados como
pendientes no se consideran implementados por estar presentes en documentos,
manifiestos o tests solamente.

### CLI

**R-CLI-18 · Resumen sin argumentos.** `ropero` presenta nombre, versión,
fecha de compilación y autor; disponibilidad/instalación de engines y modelos;
y una invitación a consultar `ropero --help`.

**R-CLI-19 · Ayuda navegable.** `ropero --help` incluye el resumen, comandos
disponibles con descripción breve, e indica `ropero <comando> --help` para el
detalle. Cada subcomando ofrece ayuda propia. La identidad visual debe ser
nueva, clara y coherente; no se copia literalmente la presentación de la v1.

**R-CLI-20 · Arranque del modelo.** `ropero <modelo>` exige engine instalado,
modelo instalado y recursos suficientes en el dispositivo elegido. `--cpu`
solicita CPU/RAM sólo para engines que lo soportan; actualmente el contrato
esperado es llama-server. `--force` puede liberar recursos terminando procesos
de ropero que ocupen el dispositivo, nunca procesos ajenos. `--port`,
`--api-key`/`--no-api-key` y `--ctx` fuerzan puerto, autenticación y contexto.

**R-CLI-21 · Instalación interactiva y desatendida.** `ropero install` ejecuta
engines y después models. `install engines [ids...]` e `install models
[ids...]` ofrecen selección interactiva sin lista y aceptan listas separadas
por espacios/comas para automatización. Los ids y alias se validan antes de
modificar el sistema.

**R-CLI-22 · Parada.** `ropero stop` detiene todos los engines lanzados por
ropero; `ropero stop <modelo>` detiene sólo la instancia de ese modelo y
preserva otros modelos, incluso si usan el mismo engine.

**R-CLI-23 · GGUF.** `ropero gguffer` admite origen HF (`--repo`) o local
(`--path`), lista de quants, imatrix local o de HF en pareja repo/fichero,
salida, precisión intermedia, retención de fuentes/intermedio, exclusión de
cabeceras MTP/DFlash, `--force`, listado de quants y diagnóstico. Debe aceptar
entradas safetensors o GGUF, incluidos pesos ya cuantizados o F16/BF16 que no
requieran cuantización. El motor oficial llama-server compilado es requisito.
La preparación específica de Strata sigue scripts/modelos de referencia
declarados para ese motor y no se presume cubierta por el pipeline genérico.

**R-CLI-24 · Status.** Presenta CPU, memoria RAM/swap y telemetría de GPU
disponible: dGPU y, en Linux, iGPU cuando existan; en macOS la GPU unificada
se muestra como GPU. Valores no disponibles se identifican como tales.

**R-CLI-25 · Test.** Ejecuta baterías sobre modelos instalados y ofrece
selección por prueba individual, grupo/tipo o todas, según
`docs/tests/indice.md`; explica qué mide cada selección y conserva resultados.

**R-CLI-26 · Doctor.** Comprueba la instalación y dependencias de ropero,
incluida su presencia en PATH, y revisa dependencias por cada engine, esté o
no instalado, mostrando cuáles faltan y cómo resolverlas.

**R-CLI-27 · List/template.** `list` muestra sólo modelos descargados cuyo
engine está instalado, agrupados por engine y con la leyenda visual.
`template` imprime el TOML completo con comentarios de uso.

### Configuración y propiedad de artefactos

**R-CFG-1 · Configuración por engine.** `~/.config/ropero/config.toml` (o la
ruta XDG equivalente) permite configurar por engine: directorio de modelos,
directorio de TOML de modelos y rango de puertos. La precedencia, valores por
defecto, validación de rangos, solapamientos y migración desde configuración
actual deben quedar explícitos. El esquema global existente no satisface por
sí solo este requisito.

**R-CFG-2 · Instalación declarativa.** Los manifiestos pueden describir cómo
instalar/compilar cada engine, sus dependencias y gestores soportados. Los
comandos o scripts pre/post instalación sólo se incorporarán con contrato de
seguridad, consentimiento, salida reproducible y cobertura por plataforma.
La lista de engines y sus recetas de instalación es fija y mantenida por el
proyecto; **no es configurable por el usuario**. La configuración del usuario
sí puede cambiar presentación (color), rutas por engine y rangos de puertos.
El color no determina compatibilidad ni estado.

**R-MOD-32 · Rutas compartidas.** Varios engines pueden compartir raíz de
almacenamiento. Un mismo directorio no implica propiedad exclusiva: modelos,
auxiliares (MTP, plantillas, mmproj, DFlash, imatrix) y derivados deben
identificarse para no interferir al instalar o borrar.

**R-MOD-33 · Reutilización de artefacto idéntico.** Si dos modelos requieren
el mismo GGUF, identificado por origen/repo, revisión o identidad de contenido,
fichero final y transformación/build, ropero reutiliza los mismos bytes y no
los descarga/fabrica dos veces. La identidad no puede basarse sólo en basename.
Si sólo comparten raíz pero no artefacto, deben quedar aislados, por ejemplo
en subdirectorios por modelo.

**R-MOD-34 · Borrado referencial.** Eliminar un modelo quita sus referencias y
artefactos exclusivamente propios; no elimina bytes compartidos mientras otra
referencia válida los necesite. Debe recuperarse de referencias rotas o
huérfanas sin seguir symlinks fuera de la raíz permitida.

**R-MOD-35 · Confirmación de pesos compartidos.** `remove` informa qué otros
modelos registrados usan los pesos/artefactos compartidos. Si hay otros
usuarios, preserva los bytes y elimina sólo la asociación del modelo pedido.
Si ese modelo es el último usuario, pide confirmación antes de borrar los
pesos; el rechazo o la falta de TTY no produce efectos.

**R-ENG-1 · Integraciones mantenidas en el proyecto.** Los engines soportados
forman una lista fija controlada por código y manifiestos del proyecto. Cada
integración debe documentar plataformas, requisitos, instalación,
descarga/almacén, argv de servicio, readiness, parada y formatos/API. El
usuario puede ajustar color, rutas y puertos, pero no repositorios, comandos,
dependencias ni hooks de instalación.

**R-ENG-2 · Diagnóstico de instalación.** `install engines` informa errores
accionables por defecto y ofrece un modo detallado que incluye contexto o
backtrace Rust y el fallo del subproceso sin filtrar secretos. Debe poder
activarse con `RUST_BACKTRACE=1` o un flag explícito.

**R-ENG-3 · Colibri.** Añadir Colibri como engine soportado. Implementar su
instalación y ciclo de vida según capacidades documentadas upstream, su
selección de modelos y su API OpenAI compatible. No duplicar su almacén ni su
descarga sin necesidad. Las capacidades CUDA, Vulkan y Metal varían por
modelo/backend y deben expresarse con precisión.

**R-ENG-4 · AirLLM (decisión aplazable).** Antes de declarar terminado el
proyecto, decidir y documentar el puente mínimo para AirLLM: proceso Python
aislado con API HTTP compatible u otra integración. Evaluar empaquetado,
dependencias CUDA, gestión del modelo y ciclo de vida. Esta decisión no bloquea
el cierre del resto de engines y funcionalidades.

**R-ENG-5 · Turbo-llama.** `turbo-llama` es un engine soportado y se integra
como fork de `llama-server`, con las mismas capacidades, plataformas y
argumentos de compilación en Linux y macOS. Su disponibilidad para el usuario
no depende de que el repositorio sea públicamente descargable. El código debe
referenciar el origen/repositorio configurado para el fork, sin reemplazarlo
por un fallback distinto. Si no se puede clonar por permisos o acceso, el
error debe identificar esa causa; la receta y el soporte del engine siguen
siendo parte del producto.

### Verificación inicial del árbol Rust (2026-11-16)

| Requisito | Evidencia observada | Estado inicial |
|---|---|---|
| R-CLI-18/19 | dispatcher/parser implementados; summary en transición | Parcial |
| R-CLI-20/22 | módulos de runtime presentes y con cambios activos | Parcial; confirmar gates de recursos y propiedad |
| R-CLI-21 | menú/lista presentes; instalación por manifiesto en progreso | Parcial; se encontró un fallo CMake, corregido en runtime |
| R-CLI-23 | `ropero-forge` y CLI gguffer existen | Parcial; casos GGUF cuantizado/Strata requieren matriz de aceptación |
| R-CLI-24/25/26/27 | crates de batería, system info, doctor, list y template presentes | Parcial; validar semántica frente al contrato nuevo |
| R-CFG-1 | config global existente, pero no por engine ni puertos | Pendiente |
| R-CFG-2/R-ENG-1 | manifiestos e instalación en progreso; recetas de todos los engines sin auditar | Parcial |
| R-MOD-32/33/34 | rutas por modelo y eliminación segura en trabajo activo; deduplicación/refs no encontradas | Pendiente |
| R-MOD-35 | remove está en trabajo activo; falta validar aviso de modelos que comparten pesos y confirmación para último usuario | Pendiente |
| R-ENG-2 | instalación no muestra modo detallado/backtrace de forma explícita | Pendiente |
| R-ENG-3 | no existe `EngineId` ni manifiesto de Colibri | Pendiente |
| R-ENG-4 | AirLLM usa API de librería Python; integración diferida, análisis requerido para cierre | Pendiente diferible |
| R-ENG-5 | `EngineId` y manifiesto existen; revisar que el origen configurado sea el fork correcto y compilarlo cuando haya acceso | Parcial (repo inaccesible desde este entorno) |

Esta tabla es una fotografía de inspección estática, no una declaración de
pruebas de aceptación. Debe actualizarse al cerrar cada requisito con evidencia
de código y pruebas.

---

## 0. Cómo se ha construido este documento

> **Nota de estado (2026-11-16):** el texto histórico de esta sección describe
> la extracción original desde la v1. La especificación posterior del CLI y
> los criterios de aceptación se registran en §4.14; los requisitos marcados
> allí son objetivo, no afirmaciones de que el código ya los cumpla.

Este documento **no es un plan**: es la especificación que la implementación ya
cumple. Se ha extraído leyendo el código y sus comentarios, y cada requisito
lleva la evidencia (`fichero:línea`) donde se demuestra.

### 0.1 El hueco que este documento llena

El código de ropero cita una especificación por todas partes —`spec §2.1`,
`§5.5`, `§7.3`, `§8.13`, `ADR-1`— y el `README.md` apunta a
`docs/specs/` como «contrato del refactor 1.0».

**Ese documento no existe en el repositorio.** `docs/` contiene la batería de
pruebas (`prueba*.md`), el juez y dos guías (`flm-memoria.md`, `strata.md`),
pero no `docs/specs/`. El README también cita `tests/suite/run.sh`, y la
batería real vive en `test/test-*.sh`.

Es decir: la especificación es real —el código la cumple y la cita— pero se
quedó fuera del repo. Este documento la reconstruye.

### 0.2 Método

1. Lectura completa del núcleo: `bin/ropero`, `bin/lib/*.sh`,
   `bin/lib/engines/*.sh`, `bin/python/modelconf.py`, `install.sh`,
   `models/_template.toml`, `README.md`, `CHANGELOG.md`, `docs/strata.md`.
2. Extracción paralela del perímetro: la fábrica de GGUFs (`cmd-gguffer.sh`,
   `bin/quant/`), el endpoint y el router (`bin/server/`), la instalación y la
   gestión de modelos (`cmd-install.sh`, `cmd-models.sh`, `install.sh`) y la
   batería de pruebas (`test/*.sh`).
3. Lectura del script adjunto `groot-strata.sh`, que se integra como
   requisito del camino «GGUF externo servido por Strata» (§ 4.13). Este
   script es **externo y no está versionado en este repositorio**; por eso
   sus 13 citas no pueden verificarse localmente contra un fichero del repo.
4. **Entrevista al dueño** (2026-11-12): el origen del proyecto (llama.cpp
   oficial → fork propio con TurboQuant/NextN/NVFP4 → MLX, FLM, TensorRT,
   AirLLM, vLLM, Strata), las dos plataformas (macOS y CachyOS), el sentido
   de cada comando (`install`, `stop`, `gguffer`, `stats`) y el vocabulario
   del TOML. Los requisitos marcados 🔶 en este documento salen de ahí: son
   el **estado objetivo**, no el código de hoy.

### 0.3 Convenciones

- **DEBE / NO DEBE**: requisito normativo. Cada uno con su evidencia.
- **R-XXX-n**: identificador estable del requisito, por área.
- Los requisitos sin evidencia local se marcan **[derivado]**: se deducen del
  comportamiento observado y de las razones escritas en los comentarios, pero
  no tienen un test que los cubra.
- **🔶** = requisito del **estado objetivo**, tomado de la declaración del
  dueño (2026-11-12) y **no implementado todavía en el código**. La
  divergencia con el código va a § 6.
- Los requisitos que el repositorio **no** garantiza están en el § 6
  (huecos), no aquí.

### 0.4 Estado del repositorio analizado

| Dato | Valor |
|---|---|
| Commit | `f21b442` («chore: hop muerto del farm eliminado de endpoint.py y test-strata (F8)») |
| Motores (engines) | 8 |
| Modelos activos | 17 (en 7 carpetas de motor) |
| Modelos retirados (`fired/`) | 28 |
| Ficheros de test | 16 |
| Tamaño | 11.022 líneas de bash + 4.188 de Python |

---

## 1. Qué es ropero

### 1.1 Tesis

**Ropero es una CLI que sirve modelos de lenguaje locales con el motor que
mejor rinda en cada máquina**, y **un modelo es un fichero de datos, no código**.

De ahí se derivan las dos mitades del producto:

- La **configuración de un modelo** (qué binario, qué flags, qué contexto,
  qué niveles de razonamiento, qué ficheros necesita) vive en un TOML.
- El **código** no cambia cuando el motor o el modelo cambian: se añade un
  fichero en `models/<engine>/` y, si hace falta, un motor nuevo en
  `bin/lib/engines/`.

> «Si el motor cambia, cambia el dato; el código no se toca.»
> — `ropero-code/README.md:9-10`

**De dónde viene.** Ropero nació para el **llama.cpp oficial** (el de
`ggml-org`), y de ahí le fueron Growing los motores: el fork propio con
**TurboQuant**, los cambios de **NextN** (una variante de MTP) y los
**fixes de NVFP4** para las NVIDIA; **MLX** (el motor de Metal de Apple, que
se usa con la aplicación `mlx-serve`); **FastFlowLM**, para la **NPU AMD
(XDNA)**; **TensorRT-LLM** (NVIDIA); **AirLLM**; **vLLM**; y **Strata**, que
está a medio integrar.

La plataforma de referencia son **macOS** y **CachyOS** (Arch-based). La
distinción no es decorativa: **cada motor declara su compatibilidad** y
`install engines` no ofrece en Linux lo que es de Metal, ni en macOS lo que
es de NVIDIA o de la NPU de AMD.

> «al final todo esté bien implementado, todo esté correctamente integrado y
> funcionando» — el objetivo declarado del proyecto

### 1.2 Alcance

Ropero resuelve cinco cosas:

1. **Catálogo**: saber qué modelos hay, de dónde salen y cómo se sirven.
2. **Ciclo de vida**: arrancar un modelo, saber cuándo está listo, pararlo,
   volver a pararlo, y no matar nada ajeno.
3. **Puertos y estado**: un modelo sirve en un puerto; ese puerto es la
   identidad operativa del sistema.
4. **Puerta única**: todos los motores acaban detrás de la misma API
   OpenAI-compatible y la misma clave.
5. **Fabricación**: convertir pesos en GGUF listo para servir, con imatrix y
   verificación.
6. **Diagnóstico**: ver el estado del sistema y de los motores, y reunirlos
   en un fichero para poder mirar (o pegar) el estado completo.

### 1.3 No-alcance (explícito)

| Ropero NO… | Por qué | Evidencia |
|---|---|---|
| …baja modelos de Strata | No hay modo «descargar sin arrancar» en Strata: el modelo se baja al arrancar el motor. Ropero enseña el **plan** y lo dice. | `ropero-code/bin/lib/engines/strata.sh:214-233` |
| …toca los ficheros de Strata | Pesos, tabla n-gram, motor y carpeta son de Strata. | `docs/strata.md` § «Lo que NO hace ropero» |
| …gestiona los presets de Strata del usuario | Los que crea ropero se llaman `ropero-<modelo>` y se para sólo ese preset. | `docs/strata.md` § «Las tres piezas» |
| …ajusta el sistema operativo | Swappiness, THP, THP, DPM, cmdline del kernel y ananicy son de `Lorenzo-SF/setup`. | `CHANGELOG.md` § «Fuera de ropero» |
| …es sourceable como librería | Es una CLI pura. Sourceado no hace nada y no altera opciones de shell. | `ropero-code/bin/ropero:41-55` |
| …mata procesos que no son suyos | `stop` mata **por puerto**, con verificación de que el PID es el engine. | `ropero-code/bin/lib/04-launch.sh:450-536` |

---

## 2. Principios de diseño

Estos principios se repiten en el código y explican casi todas las decisiones
concretas. Son la parte de la especificación que no se puede deducir de un
listado de funciones.

### P-1 · La evidencia manda sobre la suposición

El código no supone el estado del sistema: lo **mide** o lo **pregunta**.
Un `command -v` no basta: se prueba que el intérprete arranca de verdad.

> «un shim de asdf roto pasa `command -v` y luego falla: se prueba, no se
> supone» — `ropero-code/bin/lib/02-modelconf.sh:60-63`

### P-2 · Un fallo silencioso es peor que un error visible

Cada vez que una comprobación podría degradarse en silencio, el código la
duplica con un método independiente. El ejemplo canónico: `lsof` puede no
existir, y entonces `lsof -iTCP` falla y siempre dice «libre» → ropero
arranca un server encima del que ya está, y «stop» no mata nada.

> «Es un fallo SILENCIOSO que se lee como ‘los modelos no arrancan’»
> — `ropero-code/bin/lib/04-launch.sh:52-64`

Por eso hay **tres** formas de preguntar quién escucha en un puerto: `lsof` →
`ss` → `fuser` → `/proc/net/tcp` + `/proc/*/fd`.

### P-3 · Nunca se mata nada ajeno

`stop` mata por puerto, nunca con `pkill -f` global (mataría los slots
paralelos). Y antes de matar, comprueba que el PID del estado **es** el engine:
el SO recicla los PIDs, y un estado viejo apuntaría a un navegador.

> «Matarlo sería matar a un inocente» — `ropero-code/bin/lib/04-launch.sh:474-483`

### P-4 · El estado en disco es la fuente de verdad

Lo que escucha en un puerto no dice a quién lo lanzó. Ropero **sabe** a qué
modelo lanzó en cada puerto, y lo escribe en `$STATE_DIR/ports/<puerto>`. Sin
ese fichero, `status`, `stop` y la idempotencia de `use` no funcionarían desde
otro proceso.

> «lo que escucha en el puerto no dice a quién lo lanzó (y a veces ni siquiera
> publica un id mapeable al catálogo)» — `ropero-code/bin/lib/04-launch.sh:16-20`

### P-5 · Un arranque que falla, se limpia

Si el modelo no llega a tiempo, ropero **mata al proceso que arrancó** y
limpia el estado. Antes avisaba y lo dejaba vivo: el server terminaba de
cargar y se ponía a servir peticiones, dejando un zombi en el puerto.

> «ropero se comía los 300s del timeout y luego mataba a un server
> perfectamente sano» — `ropero-code/bin/lib/04-launch.sh:725-736`

### P-6 · Idempotencia por defecto

`use` es idempotente: si el modelo ya está sirviendo con **la misma
configuración**, no toca nada y sale en milisegundos. Si la configuración
cambió (el *sello* es el `cksum` de los args), reinicia: seguir con la config
vieja sería mentir en silencio.

### P-7 · Lo que no se puede comprobar, no se afirma

El validador distingue **problemas** (rompen) de **avisos** (informan). Un
modelo con `[args].base` vacío en un engine con servidor propio es un
olvido, no un error: se avisa y se deja pasar.

> «Los problemas hacen fallar el validate (salida 1). Los avisos se imprime
> pero no rompen» — `ropero-code/bin/python/modelconf.py:1417-1421`

### P-8 · La clave la pone ropero, una sola

Todos los motores comparten `LLAMA_API_KEY`. Los que no la comprueban
(Strata, FLM) arrancan sin ella y son el proxy de ropero quien la exige: una
sola clave, sin contradicciones.

### P-9 · El usuario decide, el script ejecuta

Elegir un modelo en el menú **es** consentir su descarga; la pregunta no se
repite. Los avisos de VRAM ocupada no matan nada: informan y el usuario
decide.

### P-10 · Portabilidad como restricción de diseño

Bash 3.2 (el `/bin/bash` de macOS): sin `nameref`, sin `declare -A`, sin
`mapfile`, sin `readlink -f`, sin `${var^^}`. Sin namerefs, el catálogo usa
**arrays paralelas** con el índice = posición.

### P-11 · Nada de ejecución en el sourceado

Todos los módulos son seguros de sourcear: sólo definiciones y defaults,
cero ejecución. `rp_env_setup` (que crea directorios) sólo lo llama el
comando que los necesita: `help` y `--version` no tocan disco.

### P-12 · El comentario explica el porqué

Los comentarios no describen la línea (el código ya lo dice): explican por
qué está así y qué fallo concreto evita. Es la diferencia entre un sistema
que se puede defender en voz alta y uno que no.

---

## 3. Contexto y restricciones

### 3.1 Plataformas

| | macOS (Apple Silicon) | CachyOS / Arch (Linux) |
|---|---|---|
| Bash | **3.2** (el del sistema) | ≥ 3 |
| GPU | Metal (mlx-serve) | CUDA (llama-server, tensorrt-llm) |
| NPU | — | AMD XDNA (FastFlowLM) |
| Slot CPU | **no existe**: `--cpu` es error de uso | `:9998` |
| Instalador de deps | `brew` | `pacman` |

Detección: `rp_detect_os()` → `macos | cachyos | arch | linux | otro`
(`ropero-code/bin/lib/01-env.sh:80-99`).

### 3.2 Motores soportados

| id | Qué es | SO / hardware | Puerto base | Auth nativa |
|---|---|---|---:|---|
| `llama-server` | El fork turboquant de llama.cpp (el motor principal) | macOS (Metal) / Linux (CUDA) | `:9999` | sí |
| `llama-server-official` | llama.cpp de upstream (`ggml-org`) | macOS / Linux (CUDA) | `:9989` | sí |
| `fastflowlm` | FastFlowLM en la NPU AMD (XDNA) | **sólo Linux**, NPU AMD | `:9979` | no (la pone el proxy) |
| `mlx-serve` | mlx-lm en Apple Silicon (vía la app `mlx-serve`) | **sólo macOS** (arm64) | `:9969` | sí |
| `tensorrt-llm` | TensorRT-LLM | **sólo Linux** + CUDA | `:9959` | no |
| `vllm` | vLLM | **sólo Linux** + `nvidia-smi` | `:9949` | no |
| `airllm` | AirLLM (librería pura, sin server) | macOS / Linux | `:9939` | no |
| `strata` | Strata (MoE entre VRAM/RAM/SSD) | Linux / macOS | `:9929` | no |

*(tabla de `ropero-code/bin/lib/04-launch.sh:167-180` y `ropero-code/bin/lib/03-engines.sh:38`;
el orden de ids canónicos y sus alias legacy en `ropero-code/bin/lib/03-engines.sh:54-64`;
la columna de plataforma es la **compatibility declarada** por cada engine
en `bin/lib/engines/<id>.sh`, hook `compatible`)*

**R-CTX-1** · `install engines` **sólo ofrece los motores compatibles con el
SO y el hardware de la máquina**: en Linux no aparece Metal; en macOS no
aparecen FLM, TensorRT-LLM ni vLLM. La compatibilidad la declara el propio
engine (hook `compatible`), no una tabla central. Evidencia:
`ropero-code/bin/lib/03-engines.sh:5-17`, `ropero-code/bin/lib/cmd-install.sh:495-499`.

**R-CTX-2** · `install models` **sólo ofrece los modelos de los engines que
están instalados**: si el catálogo tiene 80 modelos pero sólo hay un engine
instalado, se ofrecen los 15 de ese engine. Evidencia:
`ropero-code/bin/lib/cmd-models.sh:136-145`, `:602-767`.

**R-CTX-3** · 🔶 **Los motores y los modelos no están todos en el mismo
grado de madurez**, y la especificación no debe fingir lo contrario. En el
momento de escribir esto:

| Estado | Qué cubre |
|---|---|
| **probado de extremo a extremo** | `llama-server` (el fork con TurboQuant), `mlx-serve`, `fastflowlm` (verificado en el HX 370) |
| **integrado pero sin probar por el dueño** | `airllm`, `tensorrt-llm`, `vllm`, `llama-server-official` |
| **a medio integrar** | `strata` (objetivo: motor de primera clase, R-STRATA-36) |
| **con el mecanismo de garantía roto** | el endpoint (`endpoint.py`): sin probar y con su test de auth dando verde falso (R-TST-18) |

El principio que sostiene esta tabla es **P-1** (la evidencia manda sobre la
suposición): un motor instalado no es un motor probado, y la doc tiene que
decir cuál de las dos cosas es.

### 3.3 Hardware de referencia

Las cifras del repo están medidas en el **MSI Stealth AI+ A16**: RTX 5080
Laptop (15,93 GB de VRAM), 30,45 GiB de RAM, CachyOS. De ahí salen el
`--vram-reserve-mib 1344` de Strata, los 228 t/s de `strata-qwen36-q4` y los
topes de contexto.

La **NPU** tiene su propia referencia: **AMD Strix Point HX 370**, donde
FastFlowLM está probado y «funciona perfectamente». Es la única
combinación hardware↔motor verificada de extremo a extremo en la NPU.

### 3.4 Restricciones no funcionales

| id | Restricción | Evidencia |
|---|---|---|
| R-NF-1 | **Bash 3.2**: sin `nameref`, `declare -A`, `mapfile`, `readlink -f`, `${var^^}` | `ropero-code/README.md:199-200`, `ropero-code/bin/ropero:20-22` |
| R-NF-2 | **Comentarios y mensajes en español** | `ropero-code/bin/ropero:23-24` |
| R-NF-3 | El dispatcher no altera opciones de shell: ni `errexit` ni `nounset` | `ropero-code/bin/ropero:39-55` |
| R-NF-4 | Todo módulo es seguro de sourcear (sólo definiciones) | `ropero-code/bin/lib/01-env.sh:5-8`, `ropero-code/bin/lib/02-modelconf.sh:26` |
| R-NF-5 | `shellcheck -S warning` limpio sobre `bin/ropero bin/lib/*.sh bin/lib/engines/*.sh` | `ropero-code/README.md:195-197` |
| R-NF-6 | Los datos del modelo son **datos**: nada de lo que hay en un TOML se ejecuta | `ropero-code/models/_template.toml:9` |

---

## 4. Requisitos

### 4.1 Arquitectura y módulos

**R-ARQ-1** · El sistema se compone de un **dispatcher fino** (`bin/ropero`,
~200 líneas) y módulos con carga diferida. El dispatcher no contiene lógica de
negocio: resuelve la raíz, sourcea `00-utils.sh` y `01-env.sh`, y delega.

> «Este fichero SOLO enruta» — `ropero-code/bin/ropero:14-19`

**R-ARQ-2** · La tabla de ruteo es **completa desde el primer día** y no se
toca: cada fase posterior sólo añade `cmd-*.sh` y su función `rp_cmd_<comando>`.

> «NADIE vuelve a tocar este dispatcher» — `ropero-code/bin/ropero:25-28`

**R-ARQ-3** · Un comando no portado responde `aún no portado (fase N)` y sale
con **2**; un comando cuyo módulo no define la función sale con **1**.
Evidencia: `ropero-code/bin/ropero:252-263`.

**R-ARQ-4 (v2, working tree)** · La resolución del catálogo y la configuración
**no** usa por defecto una cadena de directorios `cwd`/`exe` como raíz de
autodescubrimiento del repositorio. En uso normal, la raíz del catálogo es la
derivada de la configuración externa: `<directorio_config>/models/<motor>/` con
manifiestos en `<directorio_config>/engines/`. Los overrides `ROPERO_MODELS_DIR`
y `ROPERO_ROOT` se preservan; si apuntan al directorio `models/` de una raíz de
catálogo/repositorio, se conserva la semántica de búsqueda de manifiestos en la
raíz hermana `<raíz>/engines/`. Los TOML del repositorio son para desarrollo y
pruebas y **no se auto-descubren** en uso normal. Ver
[ADR-008](../docs/adr/ADR-008-configuracion-externa-y-bootstrap.md).
(Evidencia histórica v1: `ropero-code/bin/ropero:66-99`.)

**R-ARQ-5** · Los módulos son: `00-utils` (utilidades y presentación),
`01-env` (entorno), `02-modelconf` (wrapper del catálogo), `03-engines`
(cargador de motores), `04-launch` (runtime), `05-download` (descargas),
`06-styles` (color de la salida en vivo), y un `cmd-<comando>.sh` por comando.

**R-ARQ-6 (v2, working tree)** · El catálogo y los manifiestos de motor son
datos TOML parseados y validados por el dominio/runtime de la v2 (Rust). La
fuente de verdad operativa en uso normal es el catálogo externo de usuario
(`<config>/models/<motor>/` y `<config>/engines/`), sujeto a overrides de
entorno. (Evidencia histórica v1: parser Python en
`ropero-code/bin/python/modelconf.py`.)

**R-ARQ-7** · El parser es **autocontenido**: trae su propio lector de TOML
(`parse_toml`, `ropero-code/bin/python/modelconf.py:478`) y sólo usa `tomllib` si está
disponible (`_tomllib()`, línea 519). Esto es lo que permite correr en el
Python del sistema de macOS.

**R-ARQ-8** · El **interprete de Python se prueba, no se supone**: se
recorren los candidatos y se ejecuta `python -c 'import sys'`; un shim de asdf
roto pasa `command -v` y luego falla. `$ROPERO_PYTHON` gana si se fija.
Evidencia: `ropero-code/bin/lib/02-modelconf.sh:60-90`.

**R-ARQ-9** · El estado runtime vive en `$STATE_DIR/ports/<puerto>`, un
fichero de líneas `CLAVE=valor` con `MODEL=`, `ENGINE=`, `PID=`, `PORT=`,
`LEVEL=`, `TS=`, `LOG=`, `STAMP=`. La escritura es **atómica** (tmp + `mv`) y
**conserva** las claves que no vienen en la llamada. Evidencia:
`ropero-code/bin/lib/04-launch.sh:249-302`.

**R-ARQ-10** · El directorio de modelos tiene dos niveles: `models/<engine>/`
(activos) y `models/<engine>/fired/` (retirados). **La diferencia es la
carpeta, no el esquema**: un modelo retirado usa exactamente el mismo formato.
Evidencia: `ropero-code/models/_template.toml:5-7`.

**R-ARQ-11** · El estado de la herramienta se puede reconstruir desde el
disco: los puertos gestionados se derivan de `$PORT`, `$CPU_PORT`, los
`[model].port` de los TOML activos y los ficheros que hay en
`$STATE_DIR/ports/`. Evidencia: `ropero-code/bin/lib/04-launch.sh:304-334`.

---

### 4.2 El modelo de datos (el TOML)

> La fuente de verdad del esquema es el **validador**:
> `python3 bin/python/modelconf.py validate --root models`
> — `ropero-code/models/_template.toml:10-11`

#### 4.2.1 Tablas y reglas

**R-DATA-1** · `[model]` es obligatoria. `name` (kebab-case, único global
entre activos y retirados), `engine` (uno de los 8), `context` (entero) son
obligatorios; `alias`, `port` y `path` son opcionales; `format` vale `gguf`
(por defecto), `safetensors`, `q4nx` o `strata`.

- `[model].path` es **obligatorio** con `format = safetensors` y **prohibido**
  con `format = gguf`. Evidencia: `ropero-code/bin/python/modelconf.py:1474-1477`.
- El id legacy `mlx_lm` se acepta y se canóniza a `mlx-serve` con aviso.
  Evidencia: `ropero-code/models/_template.toml:22-24`.

**R-DATA-2** · `[files]` es obligatoria. Los valores son **nombres de
fichero**, nunca rutas absolutas (error explícito del validador). `gguf` es
obligatoria con `format = gguf`. Claves: `gguf`, `mtp`, `mmproj`,
`chat_template`. Evidencia: `ropero-code/bin/python/modelconf.py:1479-1494`.

**R-DATA-3** · `[download]` es obligatoria salvo que el modelo se fabrique
con `[build]`. `repo` es obligatorio; `file` es obligatoria **sólo** con
`format = gguf` (con safetensors el checkpoint entero baja `hf download` y el
nombre no significa nada; si aparece, se avisa pero no rompe). `name`,
`size`, `excludes` y `manual` son opcionales. `manual = true` saca el modelo
de la oferta de `ropero install models` (checkpoints enormes que el instalador
no baja por sorpresa). Evidencia: `ropero-code/bin/python/modelconf.py:1496-1529`,
`ropero-code/models/_template.toml:47-58`.

**R-DATA-4** · `[download.aux.<tipo>]` describe auxiliares; tipo ∈ `mtp`,
`mmproj`, `chat_template`, `imatrix`, `tokenizer`. `file` es obligatorio;
`repo` puede ser otro repo; `use` ∈ `run` | `build` (por defecto, `run` para
los de runtime y `build` para los de fabricación). Evidencia:
`ropero-code/models/_template.toml:60-68`.

**R-DATA-5** · `[build]` declara **cómo se fabrica** el GGUF: `repo`, `gguf`,
`gguf_file`, `quant`, `imatrix`, `imatrix_repo`, `imatrix_file`, `out`,
`out_principal`, `from_subdir`, `include`, `intermedio` (`f16`|`bf16`),
`keep_source`, `keep_intermedio`, `no_mtp`, `force`, `size`. Una clave
**desconocida** genera aviso (no error). Evidencia:
`ropero-code/models/_template.toml:71-93`.

**R-DATA-6** · Si un TOML tiene `[download]` **y** `[build]`,
`ropero install models <modelo>` exige `--download` o `--build` para elegir
camino; en el menú interactivo se pregunta. Evidencia: `ropero-code/README.md:74-75`,
`ropero-code/models/_template.toml:72-73`.

**R-DATA-7** · `[levels]`, `[flags]` y `[options]` declaran **lo que el modelo
acepta por línea de comandos**. Un nivel, flag u opción **no declarados son
error de uso (exit 2)**: no se ignoran en silencio. Evidencia:
`ropero-code/models/_template.toml:96-98`, `ropero-code/bin/lib/cmd-runtime.sh:143`, `ropero-code/bin/lib/cmd-catalog.sh:272`.

**R-DATA-8** · `[levels].default` tiene que estar en `[levels].list`;
`[options.default].<k>` tiene que estar en `[options].list`; un
`[aliases].<x>` tiene que apuntar a un nivel de `[levels].list`. Evidencia:
`ropero-code/bin/python/modelconf.py:1531-1555`.

**R-DATA-9** · `[limits].min_ram_gb` es un número > 0. Es **orientativo**: al
arrancar, si la RAM no da, ropero **avisa y sigue** (macOS pagina en vez de
fallar: mejor que el usuario pueda intentarlo). Evidencia:
`ropero-code/bin/python/modelconf.py:1557-1565`, `ropero-code/bin/lib/04-launch.sh:598-617`.

**R-DATA-10** · `[args]` es **obligatoria para `engine = llama-server`** y
debe traer `base` como lista. Para los demás engines, un `[args].base` vacío
es un **aviso**, no un error (un modelo con servidor propio puede no llevar
flags). Evidencia: `ropero-code/bin/python/modelconf.py:1567-1582`.

**R-DATA-11** · Un `${fichero}` usado en `[args]` pero no declarado en
`[files]` es error. Evidencia: `ropero-code/bin/python/modelconf.py:1625-1644`.

**R-DATA-12** · La carpeta del TOML tiene que ser un `engine_dir` conocido
(`llama`, `llama-official`, `mlx`, `airllm`, `flm`, `tensorrt`, `vllm`,
`strata`). Evidencia: `ropero-code/bin/python/modelconf.py:1646-1649`.

#### 4.2.2 Montaje del argv

**R-DATA-13** · Un modelo **no se ejecuta**: sus `[args]` son una lista de
*tokens*, y el argv final se monta en este orden fijo:

```
[args].base → [args.level.<nivel>] → [args.flag.<f>] (en el orden pedido) → [args.option.<o>]
```

Evidencia: `ropero-code/models/_template.toml:144-146`.

**R-DATA-14** · Un elemento de la lista = **un** argumento. Un argumento con
espacios es un error de uso del TOML, no un argumento con comillas.

**R-DATA-15** · `[args.level.<n>]` admite tres formas equivalentes de
expresar el nivel: `base` (args literales), `reasoning_effort` (atajo que se
traduce a `--chat-template-kwargs <json>`) y `chat_template_kwargs` (el json
completo). Evidencia: `ropero-code/models/_template.toml:118-122`.

**R-DATA-16** · `[args.flag.<f>]` puede declarar `requires = [<claves de
[files]>]`: si el fichero no está en disco, el flag es error. Y
`[args.option.<o>]` admite `replace = "--flag"`, que **sustituye** la pareja
del base en vez de añadirla detrás (para no duplicar `--temp`). Evidencia:
`ropero-code/models/_template.toml:124-129`, `ropero-code/bin/python/modelconf.py:1608-1623`.

> **Fix pendiente · orden del argv al usar `replace` (R-DATA-16).** La v1
> elimina la pareja del base y **añade la nueva al final**, aunque la
> alternativa correcta es **sustituirla en su lugar**, manteniendo el orden del
> argv. La v2 congela este último comportamiento, porque es el que fija
> R-DATA-16 y evita que el valor reemplazado dependa de su posición. Este
> cambio de comportamiento queda documentado como `fix` y **pendiente de su
> propio PR**; no forma parte de la extracción de la v1.

> **Fix pendiente · anchura de la pareja reemplazada (R-DATA-16).** Si el token
> a reemplazar es el último del base, la v2 quita un solo token, mientras que
> la v1 quita dos o uno según `_drop_flag_pair`. La diferencia queda cubierta
> por tests sintéticos y pendiente de reconciliación en un PR propio.

#### 4.2.3 Placeholders

**R-DATA-17** · Los placeholders válidos en cualquier `base[]` son:
`${gguf}`, `${mtp}`, `${mmproj}`, `${chat_template}`, `${ctx}`, `${value}`
(sólo en `[args.option]`), `${gguf_dir}`, `${safetensors_dir}`, `${model_path}`,
`${env:VAR}` y `${env:VAR:-def}`. Evidencia: `ropero-code/models/_template.toml:131-142`.

**R-DATA-18** · Se pueden **sobrescribir sin tocar el TOML** con variables de
entorno: `ROPERO_<NOMBRE>_CTX` (contexto efectivo), `ROPERO_<NOMBRE>_GGUF`
(apunta `${gguf}` a otra cuantización del mismo repo) y
`ROPERO_MODEL_OPT_<K>` (default de una opción). El nombre se normaliza a
mayúsculas con guiones y puntos convertidos en `_`. Evidencia:
`ropero-code/models/_template.toml:148-151`, `ropero-code/bin/lib/04-launch.sh:895`.

#### 4.2.4 Formatos

**R-DATA-19** · Los cuatro formatos de artefacto son:
`gguf` (fichero local), `safetensors` (checkpoint bajo `$SAFETENSORS_DIR`),
`q4nx` (el almacén de FLM: `[model].path` es un **TAG**) y `strata` (el
catálogo de Strata: `[model].path` es `<familia>/<paquete>` y los ficheros los
baja Strata, no ropero). Evidencia: `ropero-code/models/_template.toml:29-32`.

**R-DATA-20** · Un modelo `format = gguf` **sin** `[download]` y **sin**
`[build]` es un modelo **sólo-local**: su GGUF tiene que estar ya en disco.
Ropero lo avisa, no lo rompe (un catálogo entero no se cae por un TOML con el
`[download]` comentado). Evidencia: `ropero-code/bin/python/modelconf.py:1521-1529`.

**R-DATA-21** · 🔶 **`[model].name` DEBE ser igual al nombre del fichero
TOML, sin extensión**, y ese valor es el **alias** con el que se arranca y
se para el modelo.

> «Ese mismo nombre, el campo `name`, tiene que tener el mismo valor que el
> nombre del TOML. O sea, si tenemos un modelo llamado nemotron, pues el
> TOML se llamará nemotron.toml y dentro en el atributo `name` pondremos
> nemotron también» — el dueño, 2026-11-12

Consecuencia: **no puede haber dos TOMLs iguales**, porque el alias es la
identidad operativa del modelo (arrancar, parar, estado). Un modelo con
`[model].name = "nemotron"` en `otro.toml` es un conflicto, no un segundo
modelo. ⚠ **El validador NO comprueba esta regla** (H-23): valida que
`[model].name` exista, pero no que coincida con el fichero.

**R-DATA-22** · El vocabulario de lo que un modelo acepta del CLI tiene
**tres clases**, y cada una tiene su sección en el TOML:

| Clase | Qué es | Sección TOML | Ejemplo |
|---|---|---|---|
| **nivel** (modo) | un perfil de configuración con nombre: `think`, `instruct`, `low`, `medium`, `high` | `[levels]` + `[args.level.<n>]` | `ropero nemotron` ≡ `ropero nemotron --think` si `think` es el default |
| **flag** | un interruptor que añade argumentos | `[flags]` + `[args.flag.<f>]` | `--flash` → `--flash-attn` |
| **opción** | un `clave=valor` con valor por defecto | `[options]` + `[options.default]` + `[args.option.<o>]` | `--temp 0.7` → `--temp 0.7` |

El **nivel por defecto** hace que `ropero <modelo>` sin argumentos sea
equivalente a `ropero <modelo> --<nivel-por-defecto>`. Un nivel, un flag o
una opción **no declarados** son error de uso (R-CLI-9). Los alias de nivel
(`[aliases]`) son atajos: `ropero modelo --x` ≡ `ropero modelo --high`.
Evidencia: `ropero-code/bin/python/modelconf.py:1487-1529`, `:1549-1554`;
`models/_template.toml` (ejemplo comentado, líneas 230-289).

**R-DATA-23** · Un modelo puede tener **distintos ficheros para la vía
download y para la vía build**, y **cada uno puede estar en un repositorio
distinto**: el chat template, el MTP, el mmproj (la parte de visión) y el
imatrix se declaran como auxiliares `[download.aux.<tipo>]`, cada uno con su
`file`, su `repo` opcional y su `use` (`run` = lo carga el arranque;
`build` = sólo lo usa el gguffer). La plantilla lo soporta entero:
`[download.aux.imatrix]` con `repo = "otro-usuario/imatrices"`. Evidencia:
`ropero-code/models/_template.toml:205-224`; `ropero-code/bin/lib/cmd-models.sh:321-336`.

---

### 4.3 El contrato de motor (engine)

**R-ENG-1** · Un motor es **un fichero** `bin/lib/engines/<id>.sh` que define
**ocho hooks obligatorios**:

| hook | contrato |
|---|---|
| `rp_eng_<id>_label` | etiqueta humana por stdout |
| `rp_eng_<id>_compatible` | 0 si el SO/hw lo soporta (la razón, por stderr) |
| `rp_eng_<id>_deps` | comandos externos separados por espacio |
| `rp_eng_<id>_install [--force]` | instala **sus** dependencias |
| `rp_eng_<id>_resolve` | ruta absoluta del binario |
| `rp_eng_<id>_probe <puerto>` | `ok` \| `down` \| `missing` (0 sólo si `ok`) |
| `rp_eng_<id>_native_auth` | `yes` \| `no` |
| `rp_eng_<id>_launch <modelo> <puerto> <logfile> <argv...>` | **PID** por stdout |

Evidencia: `ropero-code/bin/lib/03-engines.sh:6-25`.

**R-ENG-2** · El cargador **verifica los ocho** y, si falta uno, error
nombrándolo: `engine 'x': falta el hook rp_eng_x_label`. Evidencia:
`ropero-code/bin/lib/03-engines.sh:93-105`.

**R-ENG-3** · Los ids de motor llevan guiones y **no se traducen**: son
identificadores bash legales (`rp_eng_llama-server_label`). Se aceptan ids
legacy y se canónizan (`llama`→`llama-server`, `mlx_lm`→`mlx-serve`,
`flm`→`fastflowlm`, `tensorrt`→`tensorrt-llm`, `strata_engine`→`strata`).
Evidencia: `ropero-code/bin/lib/03-engines.sh:26-33`, `54-64`.

**R-ENG-4** · **Añadir un motor = añadir un fichero** (y, si tiene modelos,
`models/<dir>/`). Nada más. Evidencia: `ropero-code/README.md:181-182`.

**R-ENG-5** · Un motor roto **no tumba el install**: `_rp_engine_deps_ok`
devuelve 1 con la lista de lo que falta y quien llama decide. Evidencia:
`ropero-code/bin/lib/03-engines.sh:186-195`.

**R-ENG-6** · El **probe mira el puerto de ropero**, no el interno del motor:
lo que tiene que contestar es el endpoint de ropero con su `/health`. Esto
importa en los motores que van detrás de `endpoint.py` (Strata, FLM, TRT).
Evidencia: `ropero-code/bin/lib/engines/strata.sh:143-156`.

**R-ENG-7** · Un motor que no tiene auth nativa (`native_auth = no`) arranca
**sin clave** y lo pone el proxy de ropero delante. Evidencia:
`ropero-code/bin/lib/engines/strata.sh:158-167`.

---

### 4.4 La línea de comandos

**R-CLI-1** · La superficie pública es:

```
ropero <modelo> [--port N] [--cpu] [--background] [--force]
ropero <comando> [opciones]
```

Evidencia: `ropero-code/bin/ropero:8-11`.

**R-CLI-2** · Los comandos son: `use`, `stop`, `status`, `list`, `template`,
`args`, `levels`, `path`, `models`, `install` (con `setup`/`engines`/`link`/
`opencode-plugin`), `engines`, `doctor`/`check`, `config`, `gguffer`, `test`.
`engines` y `models` son alias de `install engines` / `install models`.
Evidencia: `ropero-code/bin/ropero:150-168`, tabla de ruteo `160-179`.

**R-CLI-14** · 🔶 **La superficie pública de `install` son dos comandos**:
`ropero install engines` y `ropero install models`. `setup`, `link` y
`opencode-plugin` son **pasos internos** del proceso de instalación, no
comandos que el usuario deba conocer: existen como entradas del dispatcher
porque son etapas que hay que ejecutar, no porque sean parte del contrato
público.

> «Install … sólo debería tener dos, que son `ropero install engines` y
> `ropero install models`» — el dueño, 2026-11-12

El código actual ofrece las cinco. La divergencia está en § 6 (H-22).

**R-CLI-15** · `ropero install` **a secas ejecuta el proceso completo**:
preflight del entorno → engines → models, en ese orden. Sin TTY es **error de
uso** (exit 2): nada de menús que se traguen un EOF y elijan por su cuenta.
Evidencia: `ropero-code/bin/lib/cmd-install.sh:117-130` (bare: `setup` → `engines` →
`models`), `:463-467`.

**R-CLI-16** · `ropero install engines <id>...` y
`ropero install models <alias>...` aceptan una **lista explícita** y son el
**modo desatendido**: sin TTY no hay menú, se exige la lista. Es lo que
permite automatizar la instalación de un conjunto de engines y de modelos.
Evidencia: `ropero-code/bin/lib/cmd-install.sh:91-96`, `:464-467`;
`ropero-code/bin/lib/cmd-models.sh:641-645`.

**R-CLI-17** · 🔶 **`ropero stats [--watch]`** es un comando de **estado del
sistema**: CPU, RAM y GPU, con refresco automático en `--watch`. No existe en
el repo (H-23); hoy lo más cercano es `ropero doctor` (estado de engines,
puertos y modelos) y `bin/diagnostico.sh` (un recolector que junta cinco
fuentes en un fichero). Lo que se quiere es un comando de métricas en vivo,
**distinto** de `doctor`, que sigue siendo el estado de los motores.

**R-CLI-3** · **Códigos de salida**:
`0` éxito · `1` error de ejecución · `2` error de uso (incluido «comando
aún no portado») · `130` Ctrl-C en primer plano. Evidencia:
`ropero-code/bin/lib/00-utils.sh:170-182`, `ropero-code/bin/ropero:252-263`,
`ropero-code/bin/lib/04-launch.sh:1159-1161`.

**R-CLI-4** · Un error de uso **imprime el error y un puntero a la ayuda**
(`ropero <cmd> --help → uso de <cmd>`), no un volcado de veinte líneas.
Evidencia: `ropero-code/bin/lib/00-utils.sh:170-182`, `ropero-code/bin/ropero:236-241`.

**R-CLI-5** · La ayuda general está **en línea** en el dispatcher: tiene que
funcionar aunque sólo existan `00-utils.sh`/`01-env.sh`, e incluso sin `alaja`
y sin ANSI. Evidencia: `ropero-code/bin/ropero:20-22`, `199-213`.

**R-CLI-6** · Un comando no conocido que **no empieza por `-`** se interpreta
como **arranque directo de modelo** (no como error). Evidencia:
`ropero-code/bin/ropero:265-277`.

**R-CLI-7** · `ropero list` muestra los modelos activos; `--all` incluye los
retirados. Los retirados son **informativos**: no se instalan ni se arrancan
(`ropero <modelo-retirado>` falla con «está retirado»). Evidencia:
`ropero-code/README.md:39`, `ropero-code/bin/lib/04-launch.sh:848-852`.

**R-CLI-8** · `ropero args <modelo>` imprime el **argv final**, un argumento
por línea. `ropero levels <modelo>` imprime los niveles declarados.
`ropero path <modelo>` imprime las rutas resueltas. Los tres rechazan
argumentos que no son el modelo. Evidencia: `ropero-code/bin/lib/cmd-catalog.sh:221`,
`299`, `346`.

**R-CLI-9** · Los niveles, flags y opciones se validan **contra el TOML**: un
nivel no declarado, un flag no declarado o una opción no declarada son error
de uso, con el mensaje que enumera lo que sí hay. Evidencia:
`ropero-code/bin/lib/cmd-runtime.sh:143`, `ropero-code/bin/lib/cmd-catalog.sh:258`, `272`.

**R-CLI-10** · La gramática de los argumentos tras el nombre del modelo: el
primer argumento que no es `k=v` es el **nivel**; los `k=v` son **opciones**;
el resto son **flags** (con o sin `--`). Evidencia:
`ropero-code/bin/lib/02-modelconf.sh:269-307`.

**R-CLI-11** · `ropero status <modelo>` devuelve **0 sirviéndose · 1 no ·
2 desconocido**. Evidencia: `ropero-code/README.md:45`.

**R-CLI-12** · `--cpu` sólo existe en Linux (en macOS es error de uso) y sólo
funciona con `engine = llama-server` (es `--n-gpu-layers 0`, un flag suyo).
Evidencia: `ropero-code/bin/lib/cmd-runtime.sh:85-92`, `ropero-code/bin/lib/04-launch.sh:865-870`.

**R-CLI-13** · `--port` repetido: gana el último, con aviso. `--port=N`
también vale. Un puerto inválido (fuera de 1-65535) es error. Evidencia:
`ropero-code/bin/lib/cmd-runtime.sh:94-113`, `ropero-code/bin/lib/04-launch.sh:142-145`.

---

### 4.5 El ciclo de vida del runtime

#### 4.5.1 Preflight

**R-RUN-1** · El preflight ocurre **antes de tocar el puerto**: un lanzamiento
condenado (artefacto ausente, argv inválido, motor sin instalar) **no mata lo
que está corriendo**.

> «preflight ANTES de tocar el puerto — no se mata lo que corre por un
> lanzamiento condenado» — `ropero-code/bin/lib/04-launch.sh:833-835`

El orden del preflight es: (1) motor instalado, (2) `--cpu` compatible con el
motor, (3) argv del modelo válido, (4) artefacto en disco, (5) chat template
existente, (6) `[limits]` como aviso. Evidencia: `ropero-code/bin/lib/04-launch.sh:857-972`.

**R-RUN-2** · El **chat template se comprueba en disco** antes de arrancar:
llama-server, si no lo encuentra, aborta con el muro de sus plantillas
built-in y el error real queda enterrado arriba del log.

> «se corta aquí, con una línea que sí dice qué pasa y dónde»
> — `ropero-code/bin/lib/04-launch.sh:770-775`

**R-RUN-3** · El **contexto del modelo tiene que llegar al motor**: se
inyecta `-c <contexto>` (o `--ctx-size`) antes de los args del TOML, para que
un TOML con su propio `-c` gane. Sin esto, llama-server arranca con el
contexto nativo del modelo, la KV cache se come la VRAM, los expertos se
quedan en la CPU y el modelo va a la mitad de velocidad.

> «nemotron: 60 t/s en vez de 100-120» — `ropero-code/bin/lib/04-launch.sh:934-950`

**R-RUN-4** · Para `engine = llama-server`, ropero añade `--model <ruta-del-gguf>`
al argv: el TOML declara flags, no la ruta; sin esto el server arranca en
*router mode* y no carga nada. Y `--cpu` se añade **al final** (gana a los
`--n-gpu-layers`/`--threads` del TOML) como `--n-gpu-layers 0 --threads <nproc>`.
Evidencia: `ropero-code/bin/lib/04-launch.sh:928-933`, `957-963`.

**R-RUN-5** · El override `ROPERO_<NOMBRE>_GGUF` se resuelve **en el
preflight**, con el mismo fichero que acabará en `${gguf}`: si no, el
preflight daría por bueno un cuantizado distinto del que se sirve.
Evidencia: `ropero-code/bin/lib/04-launch.sh:888-900`.

#### 4.5.2 Readiness

**R-RUN-6** · La espera es **al modelo, no sólo al puerto**: `rp_wait_ready`
hace poll del *probe* del motor y, cuando responde, exige que `/v1/models`
publique el modelo pedido (por nombre, por GGUF o por path). El éxito es
también válido si el **estado** dice `MODEL=<esperado>` con el PID vivo
(algunos servers no publican un id mapeable al catálogo).

> «ropero se comía los 300s del timeout y luego mataba a un server
> perfectamente sano» — `ropero-code/bin/lib/04-launch.sh:725-736`

**R-RUN-7** · Si el proceso **muere** antes de escuchar, el fallo es
**inmediato**: no se esperan los N segundos. Evidencia:
`ropero-code/bin/lib/04-launch.sh:704-709`.

**R-RUN-8** · El timeout por defecto es `ROPERO_START_TIMEOUT=300 s`, con
poll cada 2 s. Para un modelo que se descarga al arrancar (Strata: hasta
72 GB) la receta documentada es subirlo a `7200`. Evidencia:
`ropero-code/bin/lib/01-env.sh:63`, `ropero-code/bin/lib/04-launch.sh:698`, `ropero-code/docs/strata.md:67-72`.

**R-RUN-9** · `/v1/models` se consulta **con la API key**: llama-server exige
la clave hasta para listar. Evidencia: `ropero-code/bin/lib/04-launch.sh:338-351`.

**R-RUN-10** · Un arranque fallido **mata el proceso que arrancó** y limpia
el estado; el error va **con las últimas 15 líneas del log** (que es donde
está la causa, no en el mensaje). Evidencia: `ropero-code/bin/lib/04-launch.sh:650-682`.

**R-RUN-11** · El **PID se captura por fichero temporal**, nunca por
sustitución de comandos: con `$( )` el proceso lo crea el *subshell*, el
engine queda huérfano y `wait` responde «no es un proceso hijo de este
shell» con rc 127 — que ropero leía como «el modelo terminó con código 127»
mientras el engine seguía vivo.

> «Con el PID en un fichero temporal, el proceso lo crea ESTE shell»
> — `ropero-code/bin/lib/04-launch.sh:1061-1070`

#### 4.5.3 El sello (STAMP)

**R-RUN-12** · El *sello* de la configuración de arranque es el **`cksum` de
los args** (no `sha256sum`, que macOS no trae). Sirve para detectar que el
TOML cambió mientras el modelo ya corría.

> «el server vivo queda con la config VIEJA y ‘ya está corriendo’ sería mentir
> en silencio» — `ropero-code/bin/lib/04-launch.sh:753-757`

**R-RUN-13** · Si el modelo ya corre en el puerto destino:
- con **sello idéntico** → no se toca nada («ya está corriendo»), salida 0;
- con **sello distinto** → se reinicia, con aviso;
- **sin sello en el estado** (lo escribió otra versión) → **se reinicia**, porque
  no se puede *probar* que la config sea la misma.

Evidencia: `ropero-code/bin/lib/04-launch.sh:986-1010`.

#### 4.5.4 Primer plano y Ctrl-C

**R-RUN-14** · En primer plano, el engine **siempre** arranca en background con
salida al logfile, y lo que se ve en la terminal es el **log en vivo
coloreado** desde el offset del arranque actual (para no repetir el
historial). Evidencia: `ropero-code/bin/lib/04-launch.sh:1040-1048`, `1113-1123`.

**R-RUN-15** · La espera en primer plano es un **bucle `kill -0`**, no un
`wait`: bash aplaza el trap INT hasta que el comando esperado acaba, y el
comando esperado es el engine, que ignora SIGINT (nohup + shell sin job
control → `SigIgn 0x7`). El trap corre en cuanto llega la señal y es quien
mata al engine con SIGTERM.

> «Ni el Ctrl-C de la terminal ni un ‘kill -INT’ posterior lo matan: un
> SIG_IGN no se puede ‘des-ignorar’ con kill» — `ropero-code/bin/lib/04-launch.sh:1134-1141`

**R-RUN-16** · Ctrl-C sale con **130**, y el 130 se reconoce **por el código
del proceso**, no por el trap (que puede no llegar a correr). El trap mata el
engine, el *tail* y el colorizador (si no, el tail se queda bloqueado en
inotify sin dueño). Evidencia: `ropero-code/bin/lib/04-launch.sh:1127-1128`, `1159-1161`,
`ropero-code/bin/lib/06-styles.sh:12-20`.

**R-RUN-17** · El arranque se **bloquea por puerto** mientras dura: un
directorio `.lock` con el PID dentro (`mkdir` es atómico). El lock **caduça**
si el ropero que lo tenía muere (un `kill -9` no debe dejar el puerto
bloqueado para siempre). Evidencia: `ropero-code/bin/lib/04-launch.sh:216-247`.

---

### 4.6 Puertos, estado y parada

**R-EST-1** · La precedencia del puerto destino es:
`--port N` > `[model].port` pineado > slot CPU (`:9998`, sólo Linux) > slot base
del engine. Evidencia: `ropero-code/bin/lib/04-launch.sh:147-182`.

**R-EST-2** · Los puertos gestionados son la lista canónica de `status`,
`stop all` y la idempotencia de `use`: `$PORT` + `$CPU_PORT` (sólo Linux) +
los `[model].port` de los TOML **activos** + los puertos con fichero de estado
(los ad-hoc de `--port`). Evidencia: `ropero-code/bin/lib/04-launch.sh:304-334`.

**R-EST-3** · Para preguntar «¿quién escucha en este puerto?» hay **cuatro
métodos en cascada**: `lsof` → `ss` → `fuser` → `/proc/net/tcp` + `/proc/*/fd`
(inodo del socket). Sin `lsof`, `lsof -iTCP` falla y siempre dice «libre».
Evidencia: `ropero-code/bin/lib/04-launch.sh:52-123`.

**R-EST-4** · La identidad del modelo en un puerto tiene una **jerarquía de
cuatro niveles**: (1) id de `/v1/models` mapeado al catálogo; (2) fichero de
estado con `MODEL=` y PID vivo; (3) id crudo publicado por el server; (4) `"?"`
si el puerto está arriba pero `/v1/models` no responde. Evidencia:
`ropero-code/bin/lib/04-launch.sh:391-426`.

**R-EST-5** · Un id publicado casa con el modelo si es el nombre, el GGUF
(con o sin extensión) o el path del checkpoint (basename o ruta completa).
Evidencia: `ropero-code/bin/lib/04-launch.sh:353-372`.

**R-EST-6** · `stop` mata **por puerto**, nunca con `pkill -f` global (que
mataría los slots paralelos). El árbol de objetivos es: PID del estado + PIDs
que escuchan + sus hijos directos. Evidencia: `ropero-code/bin/lib/04-launch.sh:450-500`.

**R-EST-7** · **El PID del estado se mata aunque el puerto no escuche
todavía**: un server que está cargando el modelo todavía no escucha, y antes
quedaba vivo y se ponía a servir peticiones al terminar. Evidencia:
`ropero-code/bin/lib/04-launch.sh:458-465`.

**R-EST-8** · **Antes de matar, se verifica que el PID no esté reciclado**:
el SO recicla PIDs y un estado viejo apuntaría a un navegador. Se comprueba
que `/proc/<pid>/cmdline` mencione el binario del motor; si no lo es, **no se
mata** y se avisa. Sin `/proc` (no es Linux) se asume que sí. Evidencia:
`ropero-code/bin/lib/04-launch.sh:430-448`, `474-484`.

**R-EST-9** · La parada es escalonada: `SIGTERM` a todo el árbol → hasta
**10 s** de margen (mirando el puerto **y** los PIDs) → `SIGKILL` de red de
seguridad → borrado del estado. Evidencia: `ropero-code/bin/lib/04-launch.sh:508-536`.

**R-EST-10** · `stop` de un puerto libre devuelve **1** (no había nada que
parar), no 0. Evidencia: `ropero-code/bin/lib/04-launch.sh:502-506`.

**R-EST-11** · La **API key** es única para todos los motores y se resuelve
por precedencia: variable de entorno `LLAMA_API_KEY` > `$STATE_DIR/api_key` >
generación `sk-ropero-<hex>` (openssl, fallback `od`+`/dev/urandom`), escrita
con `chmod 600`. Evidencia: `ropero-code/bin/lib/04-launch.sh:184-214`.

**R-EST-12** · El **estado es la fuente de verdad** y se escribe de forma
**atómica** (tmp + `mv`), conservando las claves no presentes en la llamada.
Evidencia: `ropero-code/bin/lib/04-launch.sh:268-296`.

**R-EST-13** · `ropero use <modelo>` es **idempotente por (modelo, nivel)** y
emite `ROPERO_PORT=<n>` como **última línea de stdout**. Ese es el contrato
estable que consume el plugin de opencode. Evidencia:
`ropero-code/bin/lib/cmd-runtime.sh:265-292`, `ropero-code/README.md:163-164`.

**R-EST-14** · `ropero stop` **sin argumentos** para **todos los puertos
gestionados** por ropero; `ropero stop <alias>` para uno solo.
«Gestionado» significa **arrancado por ropero**: un motor que el usuario
lanzó a mano, en su propia terminal, **no se toca nunca**.

> «porque a lo mejor has arrancado un llama-server por tu cuenta, pues esos
> no, sólo los de Ropero» — el dueño, 2026-11-12

Evidencia: `ropero-code/bin/lib/cmd-runtime.sh:314-324` (destino por defecto `all`),
`:325-360`; `ropero-code/bin/lib/04-launch.sh:304-334` (puertos gestionados).

**R-EST-15** · 🔶 **Detectar los motores ajenos es una pregunta abierta.**
La capacidad de **informar** («estos procesos están usando la GPU y no son
míos») es distinta de la de **pararlos**, y ropero hoy no tiene ninguna de
las dos: no los ve, no los cuenta y no los menciona. En Linux, el
`nvidia-smi --query-compute-apps` da la lista; en macOS no está resuelto.

Lo que **sí** está resuelto es la regla de seguridad: un motor ajeno **no se
mata nunca** (P-3, R-EST-6, R-EST-8). Informar y parar son dos cosas
distintas, y la primera es mucho más barata y mucho más útil: el síntoma
«ropero no arranca porque no hay VRAM» es indistinguible de «hay otro
llama-server usando la GPU» hasta que alguien mira.

---

### 4.7 La puerta única: el endpoint OpenAI-compatible

**R-API-1** · `bin/server/endpoint.py` es un servidor FastAPI/uvicorn que
expone una API OpenAI-compatible **con autenticación propia** y hace de proxy
inverso (o genera en proceso) para los motores que no traen servidor con auth.
Los motivos concretos: `airllm` es una librería pura (no tiene `serve`),
`trtllm-serve` no tiene `--api_key` (el `api_key` del SDK de OpenAI es un
campo del *cliente*, no una comprobación de servidor) y `flm serve` no
comprueba la clave. Con `HOST=0.0.0.0` esos tres quedarían **sin auth
expuestos a la LAN**. Evidencia: `ropero-code/bin/server/endpoint.py:13-20`.

**R-API-2** · Las rutas son exactamente cinco:

| Ruta | Método | Auth |
|---|---|---|
| `/health` | GET | **no** (deliberado) |
| `/v1/models` | GET | sí |
| `/ropero/engine` | GET | sí |
| `/v1/chat/completions` | POST | sí |
| `/v1/completions` | POST | sí |

`/v1/chat/completions` y `/v1/completions` comparten la misma función; la
distinción se hace por la ruta. No hay `/docs` ni `/openapi.json`.
Evidencia: `ropero-code/bin/server/endpoint.py:1326-1429`.

**R-API-3** · `/health` **siempre devuelve 200**, incluso con el modelo sin
cargar o el backend muerto: el estado real va en `backend.ready` y
`backend.error`. Es un chequeo de vida, no información.

> «ropero lo abre antes de que el modelo haya terminado de cargar»
> — `ropero-code/bin/server/endpoint.py:1352-1354`

**R-API-4** · `/v1/models` devuelve **exactamente un modelo**, con `id` = el
nombre de ropero (no el del backend). Es lo que hace que `ropero stop
<modelo>` y la columna MODELO del `status` funcionen. Evidencia:
`ropero-code/bin/server/endpoint.py:1362-1378`, `ropero-code/bin/server/README.md:17-18`.

**R-API-5** · El campo `model` del body se reescribe **siempre** al que el
backend conoce: a `model_id` para `tensorrt` y `strata`, y al **TAG** para
`flm` (FLM valida el campo: reescribirlo al id de ropero rompería la
petición). Evidencia: `ropero-code/bin/server/endpoint.py:886-903`, `1532-1540`.

**R-API-6** · `/v1/completions` se **traduce a `/v1/chat/completions`** hacia
el backend (FLM no implementa la primera), de modo que los cuatro backends
tienen la misma semántica. Evidencia: `ropero-code/bin/server/endpoint.py:1408-1423`.

**R-API-7** · Los errores usan **el formato OpenAI**:
`{"error":{"message":…,"type":…,"code":…}}`, con `type` ∈ `invalid_request_error`
| `server_error` | `upstream_unavailable` y `code` ∈ `invalid_api_key` |
`invalid_json` | `invalid_messages` | `model_loading` | `backend_down` |
`backend_error`. Evidencia: `ropero-code/bin/server/endpoint.py:198-204`, tabla en § 8.1
de la nota de análisis.

**R-API-8** · Un body que no es un objeto JSON, o un JSON inválido, es
**400 `invalid_json`** — nunca un 500. Un `messages` malformado es
**400 `invalid_messages`**. Evidencia: `ropero-code/bin/server/endpoint.py:228-243`,
`1419-1422`.

**R-API-9** · El `content` de un mensaje puede ser una **lista de partes
VLM**: `{"type":"text"}` pasa su texto y `{"type":"image_url"}` se sustituye
por el literal `"[imagen]"`. Motivo: un endpoint de texto que hace `str()`
produce `{"type": " text"...}` dentro del prompt. Evidencia:
`ropero-code/bin/server/endpoint.py:158-175`.

**R-API-10** · `airllm` **no implementa tool calling** y el endpoint lo dice
con un 400 explícito en vez de fingir soporte (el cliente colgaría).
Evidencia: `ropero-code/bin/server/endpoint.py:556-563`.

**R-API-11** · La autenticación acepta **dos cabeceras**:
`Authorization: Bearer <clave>` y `X-Api-Key: <clave>`. La comparación es en
**tiempo constante** (`hmac.compare_digest`) y sobre bytes UTF-8. El prefijo
`Bearer` es **case-sensitive** (`bearer` en minúscula → 401) y el valor se
recorta con `strip()`. Evidencia: `ropero-code/bin/server/endpoint.py:207-225`,
`ropero-code/test/test-endpoint-auth.sh:66-70`.

**R-API-12** · **Sin clave configurada el endpoint queda abierto**, y lo dice
en el log. Es el único caso fail-open del sistema, y es explícito.
Evidencia: `ropero-code/bin/server/endpoint.py:213-214`, `1542-1544`.

**R-API-13** · **La clave nunca se reenvía al backend**: `authorization` está
en el set `HOP_BY_HOP` y se elimina de las cabeceras hacia el upstream.
El backend no la valida (por eso existe el proxy), así que reenviar una
credencial a un componente que no la necesita sólo amplía la superficie.
Evidencia: `ropero-code/bin/server/endpoint.py:127-129`, `141`, `864-884`.

**R-API-14** · La clave **jamás aparece en un log**, y el endpoint loguea
**siempre a stderr** (stdout lo consume ropero). Evidencia:
`ropero-code/bin/server/endpoint.py:145-152`.

**R-API-15** · El **sub-proceso del backend siempre escucha en `0.0.0.0.1`**:
el único proceso expuesto a la red es el proxy. Evidencia:
`ropero-code/bin/server/endpoint.py:44-45`, `1226`, `1251`, `1313`.

**R-API-16** · El proxy **puede vivir sin backend**: el puerto externo abre
antes de que el modelo esté listo y responde `503 model_loading` en vez de
colgarse. Evidencia: `ropero-code/bin/server/endpoint.py:332-337`, `549-554`.

**R-API-17** · Un solo worker de uvicorn: con backend proxied el modelo vive
en el sub-proceso, y con airllm vive en el proceso del proxy y no se puede
compartir entre workers. Evidencia: `ropero-code/bin/server/endpoint.py:1576-1583`.

**R-API-18** · `airllm` **serializa las generaciones** con un lock global:
mueve el modelo entero capa a capa, y dos generaciones a la vez no es «más
rápido», es el mismo throughput repartido y el doble de swap. Evidencia:
`ropero-code/bin/server/endpoint.py:326-329`, `531`.

**R-API-19** · Si el binario del backend **rechaza un flag** (`No such
option`), el proxy lo quita del argv, lo **cachea** y **se relanza**. La
caché está atada a la identidad del binario (ruta + `mtime_ns` + tamaño): si
el binario cambia, la caché se ignora. Evidencia:
`ropero-code/bin/server/endpoint.py:678-717`, `773-813`.

**R-API-20** · El log del motor y el del proxy **comparten fichero**
(`$LOG_DIR/<modelo>.log`); el `stderr` del hijo se lee por un pipe y se
re-escribe, guardando las **últimas 40 líneas** para detectar el
`No such option`. Evidencia: `ropero-code/bin/server/endpoint.py:742-769`,
`ropero-code/bin/lib/04-launch.sh:1035`.

**R-API-21** · El error de un 4xx/5xx del backend se **reenvía tal cual**
(mismo status, mismos bytes); los errores de *contacto* se traducen a 502/503
con un motivo claro, **nunca a un cuelgue**. Evidencia:
`ropero-code/bin/server/endpoint.py:922-936`, `996-1006`.

**R-API-22** · En streaming SSE, el proxy emite `data: [DONE]` en el `finally`
aunque el upstream se corte a mitad (airllm no tiene servidor que lo haga).
Evidencia: `ropero-code/bin/server/endpoint.py:628-632`.

**R-API-23** · 🔶 **El proxy no se decide globalmente: lo decide el engine.**
Cada engine sabe si necesita el proxy, y su hook `launch` monta el comando
con el que arranca —con o sin proxy—. El engine **no necesita que ropero
«se entere»**: lo que tiene que estar en la config es **el modelo**, con su
`engine`.

> «cuando se añada un modelo y en ese modelo se especifique un engine que usa
> el endpoint de Ropero, pues ya se sabe cuando arranque Ropero que tienes que
> arrancar ese endpoint. Lo único que hay que configurar en Ropero son los
> modelos» — el dueño, 2026-11-12

Consecuencia: **integrar un engine nuevo es desarrollo** (escribir su
fichero en `bin/lib/engines/`); **usar un modelo nuevo no lo es** (escribir
su TOML). Evidencia: `ropero-code/bin/lib/engines/airllm.sh:121`, `ropero-code/bin/lib/engines/strata.sh:278`,
`ropero-code/bin/lib/engines/tensorrt-llm.sh:169` (los tres lanzan `endpoint.py`); `ropero-code/bin/lib/engines/mlx-serve.sh:5` y
`ropero-code/bin/lib/engines/fastflowlm.sh:292` (directos).

**R-API-24** · 🔶 **Qué engines pasan por el proxy hoy** (medido):

| Engine | ¿Proxy? | Por qué |
|---|---|---|
| `airllm` | **sí** | es una librería pura: no trae server |
| `tensorrt-llm` | **sí** | `trtllm-serve` no valida la API key |
| `strata` | **sí** | `strata launch` devuelve; el proxy es el proceso vivo, y pone la key |
| `fastflowlm` | **no** (el código) | `flm serve` ya es OpenAI-compatible |
| `mlx-serve` | **no** | server propio con `--api-key-env` |
| `llama-server` / `-official` | **no** | server propio con auth nativa |
| `vllm` | **no** | server propio |

⚠ **Contradicción abierta**: la cabecera de `ropero-code/bin/lib/engines/fastflowlm.sh:6-8` dice «va
detrás de endpoint.py» y el `ropero-code/CHANGELOG.md:23-25` lo agrupa con trtllm y
strata, pero el launch real (`ropero-code/bin/lib/engines/fastflowlm.sh:292`) es **`flm serve` directo**
y el test lo **exige** (`ropero-code/test/test-endpoint-auth.sh:109-122`). El código y el
test mandan; las cabeceras mienten (H-24).

**R-API-25** · 🔶 **El endpoint está sin probar.** El dueño lo dice
explícitamente: «no sé si es sólo AirLLM o TensorRT porque no los he
probado». Y hay un motivo medido para dudarlo: **el test de autenticación
pasa en verde con la auth rota** (R-TST-18). La consecuencia práctica es que
el contrato de § 4.7 **no puede darse por cumplido** hasta que ese test
propague su código de salida.

---

### 4.8 El enrutado

**R-ROUTE-1** · El enrutado real de ropero es **por puerto y por estado en
disco**, no por reglas de petición. Un cliente elige el modelo (y con él, el
puerto); el runtime decide quién sirve ese puerto.

**R-ROUTE-2** · `bin/router.sh` **no enruta peticiones HTTP**: es un selector
semántico de modelo. Consulta el endpoint de decisión de FastFlowLM
(`D3_URL`, por defecto `http://127.0.0.1:9991/v1/systemone`), pide un modelo
para la tarea y **imprime un nombre por stdout**. Si el clasificador no
responde, **nunca falla**: devuelve un id de fallback y sale 0. Evidencia:
`ropero-code/bin/router.sh:9-38`.

**R-ROUTE-3** · El fallback de `router.sh` es `minimax/m3.1-flash`, que **no
corresponde a ningún modelo del catálogo de ropero**. Es un residuo.
Evidencia: `ropero-code/bin/router.sh:31-38` frente a `models/`.

**R-ROUTE-4** · `router.sh` **no tiene consumidor en el repo** y su cabecera
promete consultar `MODELS.md`, un fichero que **no existe** en este
repositorio. Evidencia: `ropero-code/bin/router.sh:6`, `grep` sin resultados.

**R-ROUTE-5** · El cuerpo de la petición al clasificador se construye por
**interpolación sin escapar**: una descripción con comillas dobles rompe el
JSON y la petición cae al fallback sin mensaje. Evidencia:
`ropero-code/bin/router.sh:27-35`.

---

### 4.9 La instalación

#### 4.9.1 La tesis: symlink-only

**R-INST-1 (actualizado en v2)** · `install engines` compila el motor cuando
su manifiesto declara una receta `[install]`; luego crea el symlink. Sin receta
declarada requiere un binario ya instalado. No borra árboles fuente existentes.

> «R1 — sólo symlinks: no copia, no descarga, no compila, no farms de
> entradas, no borra el repo» — `ropero-code/install.sh:23-24`

**R-INST-2** · La instalación deja **exactamente dos symlinks**:

```
~/.local/bin/ropero      → <repo>/bin/ropero
~/.local/share/ropero    → <repo>/bin
```

La share **es** el `bin` del repo. Evidencia: `ropero-code/install.sh:5-20`, `:367-392`;
idéntico contrato en `ropero-code/bin/lib/cmd-install.sh:530-537`.

**R-INST-3** · El repo es la **fuente de verdad**: `git pull` actualiza el
código instalado de inmediato, sin reinstalar. Evidencia: `ropero-code/install.sh:25-28`
(regla R3).

**R-INST-4** · **El instalador nunca escribe dentro de `<repo>/`**. La razón
no es estética: como la share es un symlink al `bin` del repo, escribir en
la share **es** escribir en el repo. Por eso los binarios de los motores
viven en `$BIN_DIR` (`~/.local/bin`), igual que los de `uv` o `brew`.
Evidencia: `ropero-code/install.sh:45-47` (R11), `ropero-code/bin/lib/cmd-install.sh:538-549`.

**R-INST-5** · **Sin red**: no hay modo red, ni `--tarball`, ni bootstrap por
stdin. `--tarball` y `--version` se **rechazan con exit 2** y el mensaje
explica por qué. Evidencia: `ropero-code/install.sh:35-36` (R6), `:556-557`.

**R-INST-6** · El **PATH del usuario no se modifica nunca**: si
`~/.local/bin` no está en el PATH, ropero avisa e imprime el `export` que
falta, pero no edita ficheros de shell. Evidencia: `ropero-code/install.sh:408-415`
(R13).

**R-INST-7 (v2, working tree)** · El instalador v1 **sólo crea** `$BIN_DIR` y
`dirname "$DIR"`; el resto lo crea el runtime/configuración. En la v2, el
bootstrap de configuración crea de forma idempotente el directorio de
configuración y `config.toml` por defecto **sin sobrescribir** cuando faltan.
En uso normal, el catálogo externo vive en `<directorio_config>/models/<motor>/`
con manifiestos en `<directorio_config>/engines/`, y `logs_dir` por defecto es
`<directorio_config>/logs`.

**Variables y precedencia (configuración):** `variable de entorno > archivo
config.toml > valor por defecto` para `models_dir` (`ROPERO_MODELS_DIR`),
`gguf_dir` (`GGUF_DIR`), `safetensors_dir` (`SAFETENSORS_DIR`), `bin_dir`
(`ROPERO_BIN_DIR`) y `logs_dir` (`LOG_DIR`). `<directorio_config>` es
`$XDG_CONFIG_HOME/ropero` o `~/.config/ropero`.

**Manifiestos de motor (usuario):** no se embuten como fallback. Se leen del
directorio `engines/` asociado a la raíz del catálogo (p. ej.
`<directorio_config>/engines/` o `<raíz>/engines/` cuando el escaneo parte de
`models/` por override).

**Migración (TOML heredados):** copiar/mover los TOML de modelos a
`~/.config/ropero/models/<engine>/` (o al `models_dir` configurado) y los
manifiestos de motor a `~/.config/ropero/engines/` (o al `engines/` de la raíz
del catálogo). Los TOML del repositorio pueden usarse en desarrollo/pruebas
mediante `ROPERO_MODELS_DIR`/`ROPERO_ROOT`, sin autodiscovery en uso normal.
Ver [ADR-008](../docs/adr/ADR-008-configuracion-externa-y-bootstrap.md).

#### 4.9.2 Los flags y sus códigos

**R-INST-8** · Los flags de `install.sh` son un **conjunto cerrado**:

| Flag | Semántica |
|---|---|
| (ninguno), `--install`, `--local` | instalan (`--local` es alias silencioso) |
| `--check` | verifica **sin modificar nada**; exit 0/1 |
| `--dir <ruta>`, `--dir=<ruta>` | redirige la share |
| `--bin-dir <ruta>`, `--bin-dir=<ruta>` | redirige el entrypoint |
| `--help`, `-h` | uso, exit 0 |
| `--tarball`, `--version` | **exit 2** (symlink-only) |
| cualquier otro | **exit 2** |

Precedencia: *flag > variable de entorno > default* (`ROPERO_DIR`,
`ROPERO_BIN_DIR`). Evidencia: `ropero-code/install.sh:69-75`, `:535-562`.

**R-INST-9** · `--local` junto a `--check` → **gana `--check`**: la
comprobación se evalúa antes de migrar o enlazar. Evidencia:
`ropero-code/install.sh:563`, `:567-570`.

**R-INST-10** · El instalador usa **sólo `pipefail`**: sin `errexit` ni
`nounset`, para que un fallo implícito no aborte la instalación a medias sin
mensaje. Los abortes son **explícitos** (`|| die`). Evidencia:
`ropero-code/install.sh:61-66`.

**R-INST-11** · La salida va a **stderr** con marcadores `[✓] [!] [✗] [-]`, en
español, con recuento final «N ok · M avisos · K fallos»; el color sólo si
stderr es TTY y `NO_COLOR` no está. Códigos: 0 ok · 1 fallo · 2 uso.
Evidencia: `ropero-code/install.sh:41-42`, `:86-108`.

**R-INST-12** · El instalador **verifica su propio resultado**: tras enlar,
ejecuta `bash <repo>/bin/ropero --help` y si no sale 0, es un fallo
(instalación **exit 1**), no un aviso. Evidencia: `ropero-code/install.sh:423-434` (R9).

#### 4.9.3 La migración del layout viejo y el rescate

**R-INST-13** · `fase_migrar` decide por el estado de `$DIR`:

| Situación | Acción |
|---|---|
| no existe | nada (se crea al enlar) |
| symlink canónico a `$ORIGIN/bin` | nada (idempotente) |
| symlink nuestro con target antiguo | se re-crea canónico |
| **symlink colgante** | si el target crudo lleva `/ropero/` como componente → se re-crea; si no → aviso y **no se toca** |
| symlink no resoluble (permisos) | aviso, no se toca |
| symlink ajeno | aviso, no se toca, `SHARE_SKIP=1` |
| **fichero real** en `$DIR` | `[✗]` + **exit 1** |
| **directorio real reconocido** | rescate → `mv` a `$DIR.old.$$` → `rm -rf` |
| **directorio real desconocido** | `[✗]` + **exit 1** («no lo toco») |

Evidencia: `ropero-code/install.sh:275-356`.

**R-INST-14** · Los **hooks de rescate** se ejecutan **antes de borrar nada**:
`_rescatar_engines` (por cada binario de motor que sea symlink dentro del
farm viejo) y `_rescatar_venv` (el venv del gguffer a
`$HOME/.local/state/ropero/venv`). Evidencia: `ropero-code/install.sh:191-258`.

**R-INST-15** · El rescate de un motor **no lo hace si `command -v <nombre>`
ya resuelve**: no debe sombrear un binario de brew o del sistema que el
usuario ya tiene funcionando. Ésta es exactamente la razón por la que
`resolve` da prioridad al PATH. Evidencia: `ropero-code/install.sh:196-214` (F2).

**R-INST-16** · El rescate **no pisa** nada: si el destino ya existe (fichero
o symlink), o el enlace está colgante o no ejecutable, **no se rescata**.
Evidencia: `ropero-code/install.sh:215-223`.

**R-INST-17** · El enlace rescatado apunta al **binario real** (symlinks
resueltos), para que no se rompa si `$BIN_DIR` se vacía y se rellena.
Evidencia: `ropero-code/install.sh:219-228`.

**R-INST-18** · El venv rescatado que colisiona con uno existente **se aparta**
como `venv.farm.$$`: no se pisa nada. Evidencia: `ropero-code/install.sh:236-258`.

#### 4.9.4 Lo que el instalador nunca toca

**R-INST-19** · La lista de intocables: el repo; un fichero real en
`$BIN_DIR/ropero`; un fichero real en `$BIN_DIR/ropero.d`; un directorio real
en `$DIR` que no reconoce como suyo; un symlink ajeno en `$DIR`; la red; el
PATH del usuario; y el entorno de otros motores.
Evidencia: `ropero-code/install.sh:369-373`, `:404-406`, `:352`, `:324-326`,
`ropero-code/bin/lib/cmd-install.sh:601-604`.

**R-INST-20** · **No existe `--uninstall`**: la única desinstalación es borrar
los dos symlinks a mano. Lo que sí existe es la **migración** del layout
viejo. Evidencia: `grep` de «uninstall» sobre `install.sh` → 0 resultados.

#### 4.9.5 `--check`

**R-INST-21** · `--check` es **read-only** y hace cinco cosas: (1) el
entrypoint debe ser symlink y resolver a un ejecutable existente; (2) la
share debe ser symlink y su destino resuelto estar **dentro del repo** del
entrypoint instalado; (3) `bash <entry> --help` debe salir 0; (4) restos
legacy (`ropero.d`, `$DIR.old.*`) → avisos, no fallos. Con **guard F4**: si
el entrypoint resuelto apunta **fuera** de este repo, avisa y **no ejecuta
nada**. Evidencia: `ropero-code/install.sh:445-530`, `:501-512`.

#### 4.9.6 Los motores: tres estados que se confunden

**R-INST-22** · Hay **tres estados distintos** de un motor, y el doctor los
distingue:

| Estado | Definición |
|---|---|
| **instalado** | su `install` terminó ok **y** `resolve` devuelve una ruta utilizable |
| **enlazado** | existe un symlink en `$BIN_DIR` → binario real |
| **en el PATH** | `command -v <nombre>` resuelve |

Evidencia: `ropero-code/bin/lib/cmd-install.sh:506-515`, `:593-613`;
`ropero-code/bin/lib/cmd-doctor.sh:286-301`.

**R-INST-23** · Un motor cuyo instalador dice OK pero que **no queda
utilizable** es un **fallo, no un éxito maquillado** (`rc=1`). Evidencia:
`ropero-code/bin/lib/cmd-install.sh:506-515`.

**R-INST-24** · La precedencia de `resolve` es **PATH → `$BIN_DIR` → rutas
largas**, en los ocho motores. ⇒ *El motor que se usa es el que está en el
PATH*, no el más nuevo ni el recién compilado. Evidencia:
`ropero-code/bin/lib/engines/llama-server.sh:314-327`, `ropero-code/bin/lib/engines/llama-server-official.sh:228-241`,
`ropero-code/bin/lib/engines/vllm.sh:73-84`, `ropero-code/bin/lib/engines/mlx-serve.sh:76-138`, `ropero-code/bin/lib/engines/tensorrt-llm.sh:103-116`,
`ropero-code/bin/lib/engines/fastflowlm.sh:155-167`, `ropero-code/bin/lib/engines/strata.sh:124-139`; `ropero-code/bin/lib/engines/airllm.sh:70-75` (no busca
binario: busca un **intérprete**).

**R-INST-25** · Los nombres de los enlaces son el `basename` del binario
resuelto: `llama-server`, `llama-server-official`, `vllm`, `mlx-serve`,
`trtllm-serve`, `flm`, `strata`. **`airllm` no lleva enlace**: es una librería
que sirve `bin/server/endpoint.py` desde su intérprete. Evidencia:
`ropero-code/bin/lib/cmd-install.sh:564-565`, `:599-600`.

**R-INST-26** · `ropero install engines` **exige TTY**: sin TTY es error de
uso (exit 2), porque un menú que se traga un EOF acabaría eligiendo por su
cuenta. Un engine no compatible se **omite** con aviso; uno ya instalado no
se reinstala salvo `--force`. Evidencia:
`ropero-code/bin/lib/cmd-install.sh:375-448`, `:464-467`, `:495-505`.

**R-INST-27** · Un engine **roto no tumba** el install: `_rp_engine_deps_ok`
devuelve 1 con la lista de dependencias que faltan, **sin `exit`**.
Evidencia: `ropero-code/bin/lib/03-engines.sh:171-184`.

---

### 4.10 El catálogo y la descarga de modelos

#### 4.10.1 El TOML es el dato

**R-MOD-1** · El catálogo **son los TOML**: la raíz es `<repo>/models`; cada
`*.toml` que no empiece por `_` es un modelo **activo**; los de
`models/<engine>/fired/` son **retirados** (informativos: no se instalan ni
arrancan). Evidencia: `ropero-code/bin/python/modelconf.py:630-658`, `ropero-code/README.md:60-62`.

**R-MOD-2** · `bin/python/modelconf.py` es **la fuente de verdad del esquema**
(el shell no lo duplica). Evidencia: `ropero-code/bin/lib/02-modelconf.sh:4-6`.

**R-MOD-3** · El shell consume el catálogo mediante **un único dump** por
proceso (idempotente), con separador `0x1F` y **31 campos** por registro, y
un **guard de versión de formato**: si la cabecera no es
`ROPERO_MC_DUMP_VERSION=1`, error y no se lee a ciegas. Evidencia:
`ropero-code/bin/lib/02-modelconf.sh:6-7`, `:33-34`, `:159-185`.

**R-MOD-4** · **Un TOML roto no tumba el catálogo**: se avisa por stderr y se
salta; los demás cargan igual. Antes abortaba el dump entero y `ropero list`
imprimía una lista vacía **con exit 0** (el peor fallo posible: falsehood
silencioso). Evidencia: `ropero-code/bin/python/modelconf.py:1176-1186`.

**R-MOD-5** · Nombres duplicados entre activo y `fired` **no son error**: las
dos filas se quedan y **la primera gana** (los activos van antes). Evidencia:
`ropero-code/bin/python/modelconf.py:137-140`, `ropero-code/bin/lib/02-modelconf.sh:220-229`.

**R-MOD-6** · El **formato** declara **dónde vive el artefacto**:

| `format` | Ubicación |
|---|---|
| `gguf` | `$GGUF_DIR/<modelo>/<[files].gguf>` |
| `safetensors` | `$SAFETENSORS_DIR/<[model].path>` |
| `q4nx` (FLM) | `[model].path` es un **TAG**, no una ruta |
| `strata` | `[model].path` es `<familia>/<paquete>` |

Evidencia: `ropero-code/bin/lib/cmd-catalog.sh:389-410`.

#### 4.10.2 Los alias

**R-MOD-7** · El alias se declara en `[model].alias`; los alias de **nivel**
en `[aliases]` **deben apuntar a un nivel declarado** (si no, error de
validación). Evidencia: `models/_template.toml` (sección `[aliases]`),
`ropero-code/bin/python/modelconf.py:1549-1554`.

**R-MOD-8** · En memoria, un alias se materializa como **una fila extra** en
las arrays paralelas, detrás de las reales, copiando sus campos con
`MODEL_ISALIAS=1` y `MODEL_REAL=<real>`. Evidencia:
`ropero-code/bin/lib/02-modelconf.sh:36-38`, `:193-214`.

**R-MOD-9** · **El choque de alias es un error** que nombra el conflicto; un
alias igual al propio nombre se ignora. Evidencia:
`ropero-code/bin/lib/02-modelconf.sh:202-206`.

**R-MOD-10** · Un alias **responde igual que el nombre**: `rp_model_exists`
acepta ambos, `rp_model_argv` traduce alias → real, y `ropero path` usa
**siempre el nombre real** para el directorio. Evidencia:
`ropero-code/bin/lib/02-modelconf.sh:231-235`, `:284-288`;
`ropero-code/bin/lib/cmd-catalog.sh:364-372`.

#### 4.10.3 El contrato de stdout

**R-MOD-11** · **stdout = datos; stderr = todo lo humano**. En `ropero list`,
stdout es el formato crudo `  %-16s engine=%-13s format=%-12s ctx=%-7s …` y
el prefijo de **dos espacios** es un contrato explícito con el plugin.
Evidencia: `ropero-code/bin/lib/cmd-catalog.sh:11-15`, `:24-27`, `:91-92`;
`ropero-code/bin/lib/00-utils.sh:5-8`.

**R-MOD-12** · `ropero levels` **no imprime cabeceras**, porque cualquier
otra cosa por stdout rompe el autocompletado; los niveles se validan con
`[A-Za-z0-9._-]+` (tokens seguros para shell). Evidencia:
`ropero-code/bin/lib/cmd-catalog.sh:284-287`.

**R-MOD-13** · `ropero args` imprime **un argumento por línea** y **valida
contra el TOML**: un nivel, flag u opción no declarado es exit 2. Evidencia:
`ropero-code/bin/lib/cmd-catalog.sh:221-282`.

#### 4.10.4 La descarga: `hf` y sólo `hf`

**R-MOD-14** · El único downloader es **`hf` (HuggingFace CLI)**: no hay
`curl` de reserva, porque `ROPERO_DOWNLOADER` era «una vía de fuga de los
tests». Evidencia: `ropero-code/bin/lib/05-download.sh:9-13`.

**R-MOD-15** · La descarga de un GGUF usa **staging propio**:
`mktemp -d "$dest_dir/.rp-dl.XXXXXX"` — dentro del destino y en el mismo
filesystem, para que el `mv` sea atómico; además evita que dos descargas con
el mismo nombre de fichero y repos distintos se pisen. Evidencia:
`ropero-code/bin/lib/05-download.sh:95-99`, `:158-162`.

**R-MOD-16** · La secuencia de una descarga de GGUF es **exactamente**:
(1) si el nombre final existe y no está vacío → «ya descargado», return 0;
(2) si no existe el final pero **existe con el nombre del repo** (descarga a
medio renombrar) → `mv` y se da por buena; (3) **`hf` ANTES de preguntar**
—«prometer una descarga y luego confesar que no se puede es la peor
secuencia posible»—; (4) `rp_confirm` con el tamaño; (5) `hf download` al
staging; (6) si `hf` dice OK pero el fichero no aparece, se busca por
`basename` (hf a veces normaliza el nombre); (7) rename comparando rutas
absolutas; (8) `rm -rf` del staging. Evidencia: `ropero-code/bin/lib/05-download.sh:123-199`.

**R-MOD-17** · La descarga de un checkpoint **safetensors NO usa staging, a
propósito**: `hf` es reanudable con `--local-dir` fijo, y un staging mataría
precisamente esa reanudación (una descarga de 12 GB cortada a medias debe
**continuar**, no empezar de cero). Evidencia:
`ropero-code/bin/lib/05-download.sh:203-209`.

**R-MOD-18** · La verificación de una descarga es **por contenido real**,
no por marcadores:
- GGUF: fichero final presente **y no vacío** (`[[ -f && -s ]]`).
- Safetensors: pesos de cualquiera de los tres formatos (`*.safetensors`,
  `pytorch_model*.bin`, `*.npz`) **y**, si hay más de un shard, el índice
  (`model.safetensors.index.json` o `pytorch_model.bin.index.json`) —«un
  shard suelto NO cuenta»— **y** `config.json`.

Evidencia: `ropero-code/bin/lib/05-download.sh:20-22`, `:38-59`, `:126`, `:228-235`.

**R-MOD-19** · Aunque `hf` salga con 0, se **re-verifica**; si falta algo →
error «hf terminó pero <dir> está incompleto», se borra el marcador y
**return 1**. Un exit 0 de un downloader no es un modelo instalado.
Evidencia: `ropero-code/bin/lib/05-download.sh:285-296`.

**R-MOD-20** · **No hay verificación por hash ni por checksum** en ninguna
parte. El campo `[download].size` es **puramente informativo**: se usa sólo
para el mensaje de consentimiento y las tablas. Evidencia:
`ropero-code/bin/lib/05-download.sh:91`, `:151`; `ropero-code/bin/lib/cmd-models.sh:478`, `:487`.

**R-MOD-21** · El estado de una descarga **se reconstruye desde el disco**:
GGUF → presencia del fichero final; safetensors → pesos + `config.json` +
marcador `.download_complete` (que **sólo** se toca si los pesos están de
verdad); un `.download_complete` sin pesos es «un directorio roto, no un
modelo instalado» y se re-descarga. Evidencia:
`ropero-code/bin/lib/05-download.sh:21-22`, `:250-256`, `:286-298`.

**R-MOD-22** · La salida de `hf` va a **stderr**, para que un install con
selección no «parezca escupir basura en vez de un modelo». Evidencia:
`ropero-code/bin/lib/05-download.sh:277-279`.

**R-MOD-23** · Los auxiliares `[download.aux.*]` se bajan al mismo directorio
con filtro `use=run` (los que el arranque referencia: `${mtp}`,
`${mmproj}`, `${chat_template}`). Evidencia: `ropero-code/bin/lib/cmd-models.sh:321-336`,
`:515-517`.

#### 4.10.5 Reutilización entre modelos

**R-MOD-24** · Si dos TOMLs piden el **mismo repo + el mismo nombre de
fichero**, el segundo modelo **se enlaza** al directorio del primero en vez de
re-descargar. La comparación es **repo + nombre destino**, no sólo el nombre
(`mmproj-BF16.gguf` existe en repos distintos con contenidos distintos).
Evidencia: `ropero-code/bin/lib/cmd-models.sh:338-435`, `:346-350`.

**R-MOD-25** · Un symlink ya hecho a mano **se respeta** (no se toca); uno
**roto** se quita; y si el fichero está con el nombre del repo en vez del
esperado, **no** se enlaza un directorio inútil: se explica el `mv` que hay
que hacer. `--force` **no** enlaza nada («quiero el fichero AHORA AQUÍ»).
Evidencia: `ropero-code/bin/lib/cmd-models.sh:396-426`.

#### 4.10.6 La decisión de descargar

**R-MOD-26** · `ropero models` **exige TTY** sin selección explícita; un
modelo `fired` pedido explícitamente es **error de uso (exit 2)**; un modelo
sin `[download]` ni `[build]` es exit 2 («su artefacto se consigue a
mano»); y un flag que contradice al TOML (`--build` en uno que sólo tiene
`[download]`) es exit 2. Evidencia: `ropero-code/bin/lib/cmd-models.sh:641-662`,
`:688-690`, `:730-737`.

**R-MOD-27** · **Elegir un modelo ES consentir su descarga**: dentro del
proceso se fija `RP_YES=1`, y la pregunta de tamaño no se repite. Evidencia:
`ropero-code/bin/lib/cmd-models.sh:19-20`, `:500`.

**R-MOD-29** · Un modelo puede declarar **las dos vías** (`[download]` y
`[build]`) a la vez; entonces hay que elegir, y `--prio download|build` es
la forma de **elegir sin preguntar**, que es lo que hace posible la
instalación desatendida de una lista de modelos:

> «cuando llegas a un modelo que tiene las dos, no sabes cuál vas a elegir con
> `--prio`. Dices directamente lo que quieres seguir y así lo automatizas»
> — el dueño, 2026-11-12

Si la prioridad pedida **no está disponible** para ese modelo, **se ignora**:
no es error de uso y no aborta la ejecución. También se ignora si el modelo
sólo tiene un camino y se indica ese, porque ya es su comportamiento por
defecto. Un `--prio` ignorado **no** impide instalar los demás modelos de la
lista.

> «ignored if the model doesn't have that path or it's already its default»
> — el dueño, 2026-11-14 (decisión que prevalece sobre la v1)

**Cambio respecto de la v1**: este caso se documentaba como **error de uso**,
con el nombre del modelo y las dos vías; la v2 **lo ignora**. Evidencia de la
v1 (comportamiento que v2 cambia):
`ropero-code/bin/lib/cmd-models.sh:696-723`, `:730-737`.

El **contrato completo** de `--prio` —valores válidos, ámbito global y por
modelo, precedencia entre ambos y qué pasa sin TTY— está en el
[doc B §4.10.7](ropero-construccion.md#4107-el-contrato-de---prio), que es la
fuente normativa de esta regla.

**R-MOD-30** · En la vía **build**, si el repo de safetensors **ya viene
cuantizado**, la fase de cuantización **se salta**: se genera el GGUF
directamente. Es la misma regla que aplica el gguffer cuando el quant pedido
ya es el de fábrica (R-GGUF-9). Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:439-483` (plan de cuantización).

**R-MOD-31** · `ropero gguffer` es el **equivalente manual** de
`ropero install models --prio build`: baja el repo de safetensors, crea el
GGUF y lo cuantiza. Funciona directamente si el safetensors ya está
cuantizado, o con el imatrix para cuantizar. Es el comando para cuando el
ciclo completo de `install models` no encaja (un modelo puntual, un
quant puntual, o un modelo que todavía no está en el catálogo). Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1-15`, `:1271-1327`.

**R-MOD-28** · Los motores con almacén propio (FLM, Strata) **deciden ellos**:
si el engine define el hook opcional `install_model`, **manda él** y se salta
el camino download/build. FLM baja su build con `flm pull <tag>` (el tag
puede venir de `[model].path` **y** de `--decision` en `[args]`) y crea
symlinks en `~/models/q4nx/<modelo>`. **Strata no descarga nada**: en Strata
no hay modo «descargar sin arrancar»; su hook pide el *plan*
(`strata launch --plan --output json`), lo enseña y dice explícitamente «no
se ha bajado NADA». Evidencia: `ropero-code/bin/lib/cmd-models.sh:437-455`,
`ropero-code/bin/lib/engines/fastflowlm.sh:188-259`,
`ropero-code/bin/lib/engines/strata.sh:188-237`.

El **contrato completo** de `--prio` —valores válidos, ámbito global y por
modelo, precedencia entre ambos y qué pasa sin TTY— está en el
[doc B §4.10.7](ropero-construccion.md#4107-el-contrato-de---prio).

---

### 4.11 La batería de tests

#### 4.11.1 Qué es

**R-TST-1** · La batería es **una especificación ejecutable en dos niveles**:

1. **Comportamiento** — 138 aserciones explícitas (shell + python) que fijan
   el contrato de arranque, probe, symlink, auth, GGUF, cuantización, FLM y
   Strata.
2. **Evaluación** — 50 sesiones de opencode (10 modelos × 5 prompts) que fijan
   el contrato de *calidad*, con un juez externo que lee los `.md` a mano.

Evidencia: recuento de call-sites en `test/test-*.sh`; el desglose por
fichero está en § 5.2.

**R-TST-2** · El exit code de los wrappers **sólo** distingue 0 (las 5
baterías ok) de 1 (alguna falló) de 2 (no se puede ni empezar). El producto
real de la capa de evaluación no es un exit code: es un **informe de
puntuación 0–100 por bloque** con veredicto A/B/C. Evidencia:
`ropero-code/test/lib/test-common.sh:499-565`, `docs/juez.md`.

**R-TST-3** · **El contenido de las respuestas del modelo no se evalúa
automáticamente en ningún test**: no hay ni un `assert` sobre la calidad de
la respuesta, sólo sobre el artefacto (JSON no vacío). La regresión sobre
calidad es manual. Evidencia: `ropero-code/test/lib/test-common.sh:401-437`.

#### 4.11.2 El runner común

**R-TST-4** · El runner resuelve sus rutas **desde la ubicación del propio
script**, no desde el `cwd` ni desde `ROPERO_ROOT`: el test debe ser lanzable
desde cualquier sitio. Evidencia: `ropero-code/test/lib/test-common.sh:37-38`.

**R-TST-5** · El runner monta un **provider hermético propio**
(`ropero-local`, openai-compatible) apuntando a `127.0.0.1:<puerto>/v1` y
**no lee ni toca la config global de opencode** del usuario; además aísla
`HOME` con `XDG_CONFIG_HOME/DATA/STATE/CACHE` → directorio de la batería.
Evidencia: `ropero-code/test/lib/test-common.sh:20-27`, `:147-183`, `:288-297`.

**R-TST-6** · La API key se resuelve **después** del arranque del modelo, en
este orden: `LLAMA_API_KEY` > `$STATE_DIR/api_key` > `OPENAI_API_KEY` >
`sk-local-dev-key`. El orden importa: contra un engine recién arrancado,
resolverla antes daría 401. Y **nunca** se imprime ni se escribe en el
informe; el fichero de config que la lleva queda en `chmod 600`. Evidencia:
`ropero-code/test/lib/test-common.sh:85-100`, `:83`, `:181`, `:459`.

**R-TST-7** · El contexto y el `output` del modelo bajo prueba se derivan
del `context` del TOML: `output = context − 1024`, saturado a
`[4096, 65536]`, con contexto por defecto 32768. Evidencia:
`ropero-code/test/lib/test-common.sh:114-128` (medido: `groot` → `context 65536 /
output 64512`).

**R-TST-8** · El runner hace **probe** del engine con
`GET /v1/models` + `Authorization: Bearer <key>` antes de lanzar opencode, y
aborta con mensaje claro si no responde; una respuesta **vacía** cuenta como
probe fallido. Evidencia: `ropero-code/test/lib/test-common.sh:189-203`, `:394-398`.

**R-TST-9** · El runner invoca exactamente
`opencode run <prompt> --agent build --model ropero-local/<modelo> --format json --standalone`,
con stdout al JSON y stderr a su propio fichero, **nunca descartado**.
Evidencia: `ropero-code/test/lib/test-common.sh:401-403` (argv verificado en ejecución).

**R-TST-10** · Un fallo en una batería **no aborta las demás**: se anota y se
sigue; el agregado sale por 1 sólo si alguna falló. Y `ropero stop all` es
tolerante a fallo (`|| true`): un entorno ya limpio no puede abortar la
batería. Evidencia: `ropero-code/test/lib/test-common.sh:541-565`, `:213`, `:216`.

**R-TST-11** · El runner produce un `recopilacion-completa.md` con el
**CONTENIDO** de cada artefacto (log del engine, JSON, sesión exportada,
stderr), no sólo sus rutas. Evidencia: `ropero-code/test/lib/test-common.sh:312-332`,
`:445-492`.

**R-TST-12** · La negativa a empezar es **exit 2**, no 1: falta el modelo,
falta `jq`/`opencode`/`ropero`, no se puede crear el directorio de salida o
no se puede preparar el entorno aislado. Evidencia:
`ropero-code/test/lib/test-common.sh:501-528`, `:530-539`.

#### 4.11.3 Los dobles

**R-TST-13** · Los dobles de `test/fakes/` no son un adorno: son **la forma
de verificar el contrato sin GPU, sin red y sin tocar el estado del
usuario**. El doble `pkill` **no mata nada** y registra el argv; el
`curl` falso devuelve 401 con rc=22 sin cabecera y error rc=7 en cualquier
otra URL; el `ropero` falso emite `ROPERO_PORT=<n>` como última línea.
Evidencia: `test/fakes/{pkill,curl,ropero,opencode}`.

**R-TST-14** · El doble `llama-quantize` **impone la restricción real del
motor**: re-cuantizar falla con el mensaje literal
`requantizing from type X is disabled`. Por eso el gguffer debe detectar el
cuant incompatible *antes* de descargar los pesos. Evidencia:
`ropero-code/test/fakes/gguffer/llama-quantize:121-129`.

**R-TST-15** · El GGUF sintético (`gguf_fake.py`) permite que el resultado
del cuantizador falso sea verificable con el **`ggufcheck` real**, no con
otro doble: eso es lo que da credibilidad a la aserción del `file_type`.
Evidencia: `test/fakes/gguffer/gguf_fake.py`, `ropero-code/test/test-gguffer.sh:178`.

**R-TST-16** · La suite offline, ejecutada con `HOME` temporal y los fakes
en el PATH, corre **5/5 baterías ok en ~2 s, exit 0**, sin GPU, sin red y
sin matar nada. *Medido en ejecución.* Evidencia: `test/test-groot.sh` +
`test/fakes/` en el PATH.

**R-TST-17** · La baja de procesos es una **lista cerrada y explícita**:
`llama-server`, `llama-server-official`, `opencode`, `mcode`, `claude`.
**Nunca `python`** — un `pkill -9 python` mataría procesos ajenos al test.
Evidencia: `ropero-code/test/lib/test-common.sh:206-219`, `ropero-code/test/fakes/pkill:6-10`.

#### 4.11.4 Lo que la batería NO garantiza — tres heridas medidas

**R-TST-18** · 🔴 **El guard de auth del endpoint pasa en verde con la auth
rota.** Verificado empíricamente: al romper `authed()` en una copia para que
devolviera `True` siempre, el bloque python reportó 5 fallos… y el script
salió con **exit 0**.

Causa: el bloque python mantiene sus **propios** contadores y su propio
`sys.exit`; bash captura ese código en `rc=$?` y **nunca lo vuelve a mirar**
(el único uso de `$rc` está en código inalcanzable, porque antes hay un
`exit 0`). Los contadores de bash sólo cuentan los checks del cableado, y
el recuento impreso añade un `+12` **hardcodeado**.

Consecuencia: el requisito de seguridad más sensible del repo **no está
garantizado por la suite**. Arreglo: `[[ "$rc" == "0" ]] || fallos=$((fallos+1))`.
Evidencia: `ropero-code/test/test-endpoint-auth.sh:31-106`, `:204-208`.

> Éste es el peor tipo de defecto posible en una batería: un checker que da
> verde sin mirar. Es peor que no tener checker, porque **consume la confianza
> que debería consumir**.

**R-TST-19** · 🟠 **El test de gguffer no es hermético**: depende del
`transformers` del host, porque hay un pre-check real **antes** de invocar
el conversor falso. Medido: en un sandbox sin `transformers`, **15 de los 38
checks fallan** y el script sale 1. El mensaje culpa al entorno, no al
gguffer. Falta un doble de `python` (o un `ROPERO_PYTHON` que apunte a un
stub). Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1961-1980`,
`ropero-code/test/test-gguffer.sh:16-17`.

**R-TST-20** · 🟠 **No hay runner de CI ni comando documentado** que ejecute
la suite completa: hay que lanzar 16 scripts a mano. El path que el README y
el CHANGELOG dan (`tests/suite/run.sh`) **no existe** (fue movido a `test/`
en `4b133bf` y la doc nunca se actualizó). El propio `ropero-code/README.md:176` dice
`test/`: la doc se contradice. No hay `.github/`, ni `Makefile`, ni
`justfile`. Evidencia: `ropero-code/README.md:176`, `ropero-code/README.md:194`, `ropero-code/CHANGELOG.md:205`.

**R-TST-21** · 🟠 **Los logs de los dobles se escriben pero nunca se
assertan.** Los cinco dobles raíz registran su argv en `$HOME/.fake-*.log`
y **ningún test los lee** (grep de `FAKE_LOG`/`.fake-` → 0 coincidencias).
Por tanto «nunca `pkill -9 python`» y «el argv exacto de `opencode run`»
son invariantes *documentadas*, no *verificadas*. El andamiaje ya existe:
basta un `grep` sobre el log. Evidencia: `ropero-code/test/fakes/README.md:50-51`.

**R-TST-22** · 🟠 **La resolución de niveles (`[levels]`) no tiene un solo
test.** El catálogo define niveles por modelo y `rp_model_argv <modelo>
[nivel]` los resuelve, pero ningún test verifica que el nivel pedido llegue
al argv, que un nivel no declarado falle, ni que el nivel por defecto se
aplique. Es la mitad de la superficie de configuración del catálogo.
Evidencia: `models/llama/groot.toml` (`[levels]`), `ropero-code/bin/lib/02-modelconf.sh:269-312`.

**R-TST-23** · 🟠 **21 ficheros de código no son tocados por ningún test**,
entre ellos `cmd-catalog.sh`, `cmd-doctor.sh`, `cmd-install.sh`,
`cmd-runtime.sh`, `cmd-gguffer.sh`, `router.sh`, los seis motores restantes
(sólo `fastflowlm` y `strata` tienen test) y todo `bin/quant/`. Se prueban
**2 de los 8 motores**. Evidencia: `grep` sobre `test/*.sh` → 0 coincidencias.

**R-TST-24** · 🟡 **El CHANGELOG describe un contrato caducado** que el test
contradice: afirma que el engine FLM le pasa `--api-key` al endpoint y «se
niega a arrancar sin `LLAMA_API_KEY`»; el test afirma **exactamente lo
contrario** (arranca `flm serve` directo, sin `--api-key`, y arranca igual
sin `LLAMA_API_KEY`). Va además en la cifra (17 vs. 19 checks) y no registra
el refactor. **El test es la fuente de verdad; el CHANGELOG miente.**
Evidencia: `ropero-code/CHANGELOG.md:98-102` vs. `ropero-code/test/test-endpoint-auth.sh:116-122`,
`:199-202`.

**R-TST-25** · 🟡 **Dos scripts dependent del entorno más de lo que
parece**: `test-flm-decision.sh` y `test-models-symlink.sh` abortan con
`LLAMA_API_KEY: unbound variable` si no está exportada (por `set -u` +
`: "${OPENAI_API_KEY:=$LLAMA_API_KEY}"`), y el runner exige `jq` en el PATH.
Un fallo de entorno se presenta como fallo de test. Evidencia:
`ropero-code/bin/lib/01-env.sh:60`, `ropero-code/test/lib/test-common.sh:522-528`.

**R-TST-26** · 🟡 **La compatibilidad con bash 3.2 es una declaración, no
una verificación**: el README y las cabeceras de los 11 scripts la afirman,
pero la batería **no ejecuta `bash 3.2`** en ningún sitio (el sandbox es
Linux con bash 5). El grep confirma que no se usan las construcciones
prohibidas, pero la compatibilidad real en macOS **no está probada**.
Evidencia: `ropero-code/README.md:199-200`, `ropero-code/test/bench-motor.sh:411`.

**R-TST-27** · 🟡 **`shellcheck` no está en la batería**, pese a que el
README lo documenta como requisito de desarrollo. Evidencia:
`ropero-code/README.md:194`.

**R-TST-28** · 🟡 **Dos scripts devuelven códigos no estándar**:
`test-strata.sh` sale con el **número de fallos** (3 fallos → exit 3, no 1) y
`test-endpoint-auth.sh` sale 0/1 pero **sin propagar el fallo del bloque
python** (ver R-TST-18). Evidencia: `ropero-code/test/test-strata.sh:218-223`,
`ropero-code/test/test-endpoint-auth.sh:204-208`.

---

### 4.12 La integración con opencode

**R-OC-1** · El plugin es `opencode-plugin/index.ts`, un plugin de
TypeScript (`@opencode/plugin`) con `id: "ropero-modelo"`. Hace **dos
cosas** por el mismo motivo: que el modelo que pides sea el que te
responde, y que no se te vaya GPU/tiempo en peticiones que no son tuyas.
Evidencia: `ropero-code/opencode-plugin/index.ts:4-40`, `:146-147`.

**R-OC-2** · El plugin se instala como **copia**, no como symlink — al revés
que todo lo demás de ropero, y con razón explícita: **el watcher de opencode
(inotify) no sigue symlinks**, así que los cambios no se recargarían. Si el
destino difiere, se guarda un **backup numerado** (`.bak`, `.bak.1`,
`.bak.2`…) antes de sobrescribir; si el destino es idéntico (`cmp -s`), no
se toca nada. Evidencia: `ropero-code/bin/lib/cmd-install.sh:688-691`, `:710-712`,
`:722-732`.

**R-OC-3** · El instalador **no registra el plugin**: al final **comprueba**
si la config de opencode declara `"ropero-modelo"` y, si no, avisa e imprime
el snippet. Los plugins de `~/.config/opencode/plugins/` se cargan solos.
Evidencia: `ropero-code/bin/lib/cmd-install.sh:741-753`, `ropero-code/opencode-plugin/README.md:14`.

**R-OC-4** · Las **tres opciones** del plugin:

| Opción | Default | Efecto |
|---|---|---|
| `provider` | `"marichocho"` | el provider de opencode que atiende a los modelos de ropero |
| `port` | — (sin `--port`) | puerto donde debe estar el modelo; **manda sobre el `MODEL_PORT` pineado** |
| `titleMaxTokens` | `64` | tope de tokens de la petición de título; `0` desactiva |
| `titleReasoning` | `"low"` | effort del razonamiento en la petición de título |
| `titleLocal` | `false` | el título se pone en local y **no** se llama al modelo |

Evidencia: `ropero-code/opencode-plugin/index.ts:42-57`, `:151`, `:164`, `:271-279`.

**R-OC-5** · Los **tres hooks** usados y cuándo:

| Hook | Cuándo | Qué hace |
|---|---|---|
| `context` | justo antes de cada petición del agent loop | `ropero use <modelo>` (+ `--nivel` si el variant coincide con un nivel) y quita `reasoningEffort` del body si se pasó nivel |
| `model.request` | en cada petición (`kind` = `primary\|compaction\|title\|generate`) | `ropero use` también para la petición de título |
| `title` | en la petición de título | acota `maxTokens` y `reasoningEffort`, o pone el título en local |

`hook("prompt")` **no** sirve: no tiene `model`. Evidencia:
`ropero-code/opencode-plugin/index.ts:215-281`, `ropero-code/opencode-plugin/README.md:96-103`.

**R-OC-6** · El plugin **filtra**: consulta `ropero list` (cacheado) y **no
hace nada** si el `modelID` no es un modelo de ropero — «el server ya está
sirviendo lo que sea, y meter a ropero por medio sólo conseguiría romper la
petición». Evidencia: `ropero-code/opencode-plugin/index.ts:76-101`, `:179-191`.

**R-OC-7** · Si `ropero use` falla, el hook **lanza**: la petición de
opencode **falla**. No se degrada en silencio. Evidencia:
`ropero-code/opencode-plugin/index.ts:167-172`, `ropero-code/opencode-plugin/README.md:60-62`.

**R-OC-8** · El `variant` de opencode se pasa como `--nivel` **sólo si
coincide exactamente** con un nivel declarado por el modelo (`ropero levels
<modelo>`). Evidencia: `ropero-code/opencode-plugin/index.ts:230-232`.

**R-OC-9** · La deduplicación usa `servido` + `enCurso` para no lanzar dos
`ropero use` por dos peticiones seguidas; la clave incluye el **nivel**,
porque el mismo modelo con otro nivel son args distintos y el server se
reinicia. Evidencia: `ropero-code/opencode-plugin/index.ts:153-160`, `:192-210`.

**R-OC-10** · El contrato estable con la CLI es de tres cosas: `list` (dos
espacios + nombre), `levels` (uno por línea) y `use` (última línea
`ROPERO_PORT=<n>`). Evidence: `ropero-code/opencode-plugin/index.ts:173`,
`ropero-code/bin/lib/cmd-runtime.sh:273`, `:291`; `ropero-code/README.md:163-164`.

**R-OC-11** · El `provider` por defecto del plugin es **`"marichocho"`**, un
provider que **no existe** en la configuración actual de opencode del
dueño. Es un residuo vivo en el código. Evidencia:
`ropero-code/opencode-plugin/index.ts:151`.

---

### 4.13 El motor Strata y la integración de `groot-strata.sh`

Esta es la sección que responde a la pregunta «¿qué habría que construir para
que `groot-strata.sh` fuera parte de ropero?». Antes de los requisitos, el
estado del arte.

#### 4.13.1 Qué es Strata, y por qué ropero lo trata aparte

**R-STRATA-1** · Strata **no es un servidor de modelos**: es un *terminal* que
baja un motor, prepara los artefactos y deja el modelo en disco. De ahí que
el engine de ropero tenga **tres piezas extra** sobre el contrato de 8
hooks: `install_model`, el *puente* `strata-run.sh` y la resolución de
*packs*. Evidencia: `ropero-code/test/test-strata.sh:5-16`, `:102-109`.

**R-STRATA-2** · Un modelo de Strata en el catálogo es un **puntero** al
catálogo de Strata: `[model].path = "<familia>/<paquete>"` (p. ej.
`gyro/Gyro-M`), **no** una ruta. `_rp_eng_strata_pack` lo dice
explícitamente y **no** usa `_rp_engine_model_path`, que le pegaría el
prefijo de `$SAFETENSORS_DIR`. Evidencia:
`ropero-code/bin/lib/engines/strata.sh:160-176`; `ropero-code/models/strata/gyro-m.toml:22-25`.

**R-STRATA-3** · Los ficheros **no los baja ropero**: los baja Strata al
arrancar. En Strata **no hay modo «descargar sin arrancar»**, y el
`install_model` de ropero lo dice con todas las letras («no se ha bajado
NADA») y da la receta (`ROPERO_START_TIMEOUT=7200 ropero use <modelo>`).
Evidencia: `ropero-code/bin/lib/engines/strata.sh:188-237`;
`docs/strata.md`.

**R-STRATA-4** · La instalación de Strata es la **oficial**: se descarga el
instalador a un **fichero temporal** (nunca `curl | sh`) y se ejecuta con
`env STRATA_BIN_DIR=$BIN_DIR sh $tmp`. La razón: `strata install` usa la
misma variable de entorno para **buscar** `setup.py` y para decidir **dónde
escribir**, así que no sirve para aislar las dos rutas. Evidencia:
`ropero-code/bin/lib/engines/strata.sh:87-111`.

**R-STRATA-5** · La precedencia de `resolve` es **PATH → `$BIN_DIR` →
`~/.local/bin/strata` → `~/.local/share/strata/terminal/current/bin/strata`**
— el *launcher activo*, no la versión más nueva que haya en disco.
Evidencia: `ropero-code/bin/lib/engines/strata.sh:124-139`.

**R-STRATA-6** · El **probe** distingue tres estados: `missing` (Strata
ausente), `down` (Strata presente, puerto sin servicio) y `ok`. Y mira el
**puerto de ropero** (el proxy), no el interno de Strata: lo que tiene que
contestar es el endpoint de ropero con su `/health`. Evidencia:
`ropero-code/bin/lib/engines/strata.sh:141-154`.

**R-STRATA-7** · Strata **no hace auth** (`native_auth` → `no`): la clave la
pone el proxy de ropero. Strata arranca con `--access local --no-api-key`,
de modo que sólo escucha en 127.0.0.1. Evidencia:
`ropero-code/bin/lib/engines/strata.sh:157-163`.

**R-STRATA-8** · El `launch` de ropero **devuelve un PID**, y el proceso
supervisor es otro: `strata launch --no-tui` **devuelve** cuando el server ya
sirve. De ahí el puente. Evidencia: `ropero-code/test/test-strata.sh:160-161`;
`ropero-code/bin/server/strata-run.sh:14-30`.

**R-STRATA-9** · El **puente** `strata-run.sh` (1) lanza Strata y **espera a
que termine** —la descarga puede ser de 70 GB—, (2) **se queda vivo**
mientras el modelo sirve, y (3) al recibir SIGTERM/SIGINT **para lo que él
mismo arrancó** y sale. Evidencia: `ropero-code/bin/server/strata-run.sh:14-30`, `:100-152`.

**R-STRATA-10** · Para poder parar sólo lo suyo, el launch se guarda con
`--save-preset <nombre>` y al parar se usa `strata launch --preset <nombre>
--stop`: `strata launch --stop` a secas para **todos** los launches, y
parar el Strata del usuario sería una grosería. Evidencia:
`ropero-code/bin/server/strata-run.sh:36-48`, `:100-110`.

**R-STRATA-11** · El puente comprueba **cada 15 s** que el launch sigue
corriendo (`strata launch --status --output json`): si Strata lo paró o
murió el motor, el wrapper sale con error para que ropero **limpie el
estado** en vez de quedarse con un «modelo» que no existe. Y el `sleep` es
de 1 s, no de 1 h, porque el trap de bash **se aplaza** mientras duerme un
comando (medido). Evidencia: `ropero-code/bin/server/strata-run.sh:130-152`.

**R-STRATA-12** · El `launch` de ropero pasa `--backend strata`,
`--backend-bin <binario>`, `--model <familia/paquete>`, el **id publicado de
ropero** (`strata-qwen38-q2`), el **contexto** del modelo y el **puerto
interno = externo + 1000** (10998). Los flags extra del TOML viajan como
`--extra-arg` y llegan al backend. Evidencia: `ropero-code/test/test-strata.sh:169-177`.

**R-STRATA-13** · El **prefijo `--set`** es el mecanismo con el que se pasan
opciones con valor a Strata: `ropero use strata-qwen36-q4 --vram-reserve 1344`
se traduce en `--set vram_reserve_mib=1344`. El valor por defecto de
1344 MiB es **medido en esta máquina** (RTX 5080 Laptop): con menos de
512 MiB libres el driver de NVIDIA pagina y la petición no vuelve nunca.
Evidencia: `ropero-code/docs/strata.md:132-137`;
`ropero-code/models/strata/gyro-m.toml:56-60`.

#### 4.13.2 El hueco: `strata launch` no puede servir estos modelos

**R-STRATA-14** · 🔴 **El motor de ropero lanza Strata con `strata launch`,
que vuelve a pasar por `setup.py`**. `setup.py` **reescribe el run config**
con el perfil de expertos de Flash-Next (**48×512**), que el motor **rechaza**
para Groot (**48×256**):

> «read_expert_profile: … is 48x512 but this model is 48x256 — it is a profile
> for a different artifact»

Sin un profile propio, el *verify* del motor mata el arranque: «verify: needs
the profile-filled VRAM expert tier». Evidencia: `groot-strata.sh:15-18`,
`:184-205`.

⇒ **El camino `strata launch` es insuficiente por construcción** para
cualquier GGUF cuyo perfil de expertos no coincida con el del modelo
instalado. Y no es un caso hipotético: es **exactamente** el caso de los dos
modelos Groot Gyro-M.

**R-STRATA-15** · 🔴 **Cada GGUF necesita su propio pack**, no el del modelo
ya instalado. El pack de `gyro-m` (Flash-Next, 91 GB) tiene las **posiciones
absolutas de los tensores** dentro de su GGUF; con otro GGUF (Groot, 54 GB)
esas posiciones no valen. El síntoma es desconcertante:

> «this pack holds the tensor only in its GGUF form» → y luego
> «ggml_cuda_init: failed to initialize CUDA: no CUDA-capable device is
> detected», que **parece** un problema de GPU y es del pack.

Evidencia: `groot-strata.sh:260-271`.

**R-STRATA-16** · El **profile de expertos** propio se genera **sintético y
neutro** (no calibrado con activaciones), con **ranking round-robin por
capa**: capa 0 exp0, capa 1 exp0, … capa 47 exp0, capa 0 exp1, … El cache
cubre así las 48 capas por igual; con el orden natural el cache se llenaría
de las primeras capas y dejaría las últimas sin cubrir. Formato: `"STRP"` +
u32 versión + u32 capas + u32 expertos + u32 n1 + u32 n2, con n1 pares
(u16 capa, u16 experto) = ranking, y n2 u32 = permutación inversa.
Evidencia: `groot-strata.sh:184-205`, `:206-258`.

**R-STRATA-17** · El script **no se limita a lanzar**: el ciclo completo es
`preparar` → `afinar` → `lanzar`, y cada uno tiene su razón:
- `preparar`: carpetas + descarga + **verificación** (`strata models import
  <gguf> --plan`) + **un profile por GGUF** (setup.py aislado por modelo,
  pack propio, profile propio, args parcheados).
- `afinar`: **mide este PC** (`strata tune auto`) y guarda su preset, que
  Strata aplica al arrancar. Modo `quick` (~20 min) o `thorough` (hasta 2 h).
- `lanzar`: `strata serve --profile <cfg> --port <n>`.

Evidencia: `groot-strata.sh:5-11`, `:363-399`, `:402-413`, `:449-473`.

**R-STRATA-18** · El `setup.py` de Strata se invoca **directamente** con
`STRATA_CONTROL_INSTALL=1` y un `STRATA_INSTALL_DIR` **privado por modelo**,
porque `setup.py` usa `CONTROL` para dirigir **todas** las escrituras a
`home()` (no al `gyro-m` compartido) y `strata install` usa la misma variable
también para **buscar** `setup.py` — no sirve para aislar ambas rutas.
Evidencia: `groot-strata.sh:160-182`.

**R-STRATA-19** · El **parche de args** reconstruye la lista conservando las
opciones de tuning actuales y **sustituyendo sólo** pack, profile y reserva
(incluidos duplicados). Elimina `--mmap-experts` (un switch sin valor: el
motor lo decide solo) y añade `--pack <propio> --expert-profile <propio>
--vram-reserve-mib <reserva>`. Además **reescribe** `tokenizer` (el del pack
propio), `model_name` y `log` (cada profile escribe en **su** log: si no, se
pisan). Evidencia: `groot-strata.sh:301-354`.

**R-STRATA-20** · El `afinar` **exige GPU libre** y **verifica el resultado
del tune por JSON**, no por el exit code: exige `ok is True`, `outcome` ∈
{`improved`, `default_confirmed`} y un `preset.id` presente. Un `strata tune`
que sale 0 pero no midió nada **es un fallo**, no un éxito. Evidencia:
`groot-strata.sh:363-399`.

**R-STRATA-21** · La comprobación de GPU libre es **estricta a propósito**:
tolera usos triviales (≤500 MiB por proceso —el escritorio puede aparecer como
proceso CUDA con unos pocos MiB—) pero **cualquier proceso significativo o un
dato que no se pueda interpretar bloquea la operación**, y **nunca se mata
nada**. Evidencia: `groot-strata.sh:100-125`.

**R-STRATA-22** · El `parar` del script **se niega a parar procesos**:
«este script no registra propiedad de motores; deténlos con su operador». Es
la política correcta para un script sin registro de propiedad, y **es
precisamente lo que ropero ya sabe hacer mejor**: ropero tiene el fichero de
estado por puerto y verifica la identidad del PID antes de matar
(R-EST-6, R-EST-8). Evidencia: `groot-strata.sh:96-98`.

#### 4.13.3 Los requisitos que la integración impone

**R-STRATA-23** · 🔴 **El engine de Strata necesita un segundo modo de
lanzamiento**: `strata serve --profile <run-config>` además del actual
`strata launch --family … --model …`. El modo actual **reescribe el run
config** y es incompatible con los GGUF de perfil propio. Sin este modo, los
modelos Groot Gyro-M **no son servibles por ropero**.

**R-STRATA-24** · 🔴 **`[model].path` necesita un segundo significado** para
Strata: hoy significa `<familia>/<paquete>` del catálogo de Strata; para el
camino del script tendría que significar **la ruta al run config preparado**
(o un discriminante explícito, p. ej. `[model].strata_mode = "profile"` +
`[model].path = "<ruta-al-cfg>"`). El validador `validate_model` y el motor
deben distinguir ambos casos sin ambigüedad.

**R-STRATA-25** · 🔴 **El artefacto preparado necesita un sitio en el
catálogo de ropero**. Hoy un modelo es *un* artefacto (`$GGUF_DIR/<modelo>/<gguf>`).
El camino del script necesita **cuatro** por modelo:

| Artefacto | Ruta (según el script) | Qué es |
|---|---|---|
| GGUF | `~/models/gguf/<id>/Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf` | los pesos |
| pack | `~/models/gguf/<id>/packs/gyro-gyro-m/` | posiciones absolutas de tensores **de este** GGUF |
| profile | `~/models/gguf/<id>/expert-profile.bin` | ranking de expertos 48×256 |
| run config | `~/.local/share/strata/install/strata-<id>.json` | los args parcheados |

El catálogo actual **no tiene un sitio para los derivados**. La convención
que sí existe es la de FLM (`~/models/q4nx/<modelo>` con symlinks), que
sugiere un `~/models/<formato>/<modelo>/` por modelo con los artefactos
derivados dentro.

**R-STRATA-26** · El **nombre público del GGUF** es una decisión con
consecuencias: el script renombra **ambos** modelos a
`Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf` porque «setup.py sólo encuentra ese
nombre». En ropero, `[files].gguf` puede differing por modelo, y el
directorio (`$GGUF_DIR/<modelo>/`) es lo que los separa. ⇒ El TOML de cada
modelo debe declarar `[files].gguf = "Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf"`,
y conviene que ropero **no** imponga un nombre por modelo cuando el motor
imone un nombre por artefacto. (Consecuencia colateral: la regla R-MOD-24 de
reutilización compara repo + nombre ⇒ los dos modelos **no** se enlazan,
porque los repos son distintos. Correcto.)

**R-STRATA-27** · **El `tune` debe entrar en ropero como una operación de
primer nivel**: `ropero tune <modelo> [--mode quick|thorough]` que mide la
máquina y guarda el preset, con la misma verificación estricta del script
(`ok is True` + `outcome` + `preset.id`). Hoy el tune sólo existe como
**opción del TOML** (`[options] list = ["tune", …]`) y está **comentado** en
todos los TOML de Strata.

**R-STRATA-28** · La **opción `vram-reserve` está declarada pero muerta**:
aparece en `[options].list` y su bloque `[args.option.vram-reserve]` está
**comentado** en los dos TOML de Strata. Y el mecanismo del comentario
(`--set vram_reserve_mib=${value}`) **no es** el que usa el script
(`--vram-reserve-mib <n>` como flag directo). ⇒ Hay que decidir **una**
forma y dejarla viva y probada: sin la reserva, por debajo de 512 MiB libres
el driver de NVIDIA pagina y la petición no vuelve nunca. Evidencia:
`ropero-code/models/strata/gyro-m.toml:29`, `:56-60`; `ropero-code/docs/strata.md:132-137`;
`groot-strata.sh:343-344`.

**R-STRATA-29** · La **reserva de VRAM** (1344 MiB) es un valor **medido
en una máquina concreta** (MSI Stealth AI+ A16, RTX 5080 Laptop). Si ropero
la adopta como valor por defecto, debe ser **una variable de entorno
sobreescribible** y el valor por defecto debe quedar documentado como
*medido en esta máquina*, no como universal. El propio script ya lo hace
(`RESERVA="${RESERVA:-1344}"`).

**R-STRATA-30** · La **preparación del pack** necesita `llama.cpp` con
`gguf-py` **ya instalado** en el árbol de Strata: el script **no lo descarga
automáticamente** («Instálalo previamente y vuelve a ejecutar
`preparar_pack <id>`»), y se niega a seguir si falta. Es una precondición
explícita que ropero debe poder **comprobar y nombrar**, no descubrir a
mitad de la preparación.

**R-STRATA-31** · El **aislamiento del `setup.py`** exige que el directorio
privado de instalación **no sea un symlink** y **no coincida** con
`INSTALL_DIR` compartido. Son dos comprobaciones explícitas del script que
evitan que la preparación de un modelo pise la instalación compartida.
Evidencia: `groot-strata.sh:168-170`.

**R-STRATA-32** · El **estado del script es su propio disco**, no el de
ropero: `$RAIZ/<id>/tune.json`, `$RAIZ/<id>/tune.log`,
`$RAIZ/<id>/prepare-pack.log` y el run config en
`$INSTALL_DIR/strata-<id>.json`. Para que ropero pueda parar el modelo y
limpiar su estado, necesita **apuntarlos** en su fichero de estado por puerto
(`$STATE_DIR/ports/<puerto>`), cuyo formato actual (líneas `KEY=`) admite
una extensión.

**R-STRATA-33** · La **comprobación de GPU libre** del script es más estricta
que la de ropero, y ambas son razonables en su contexto: ropero es
**idempotente** (si el modelo ya corre en el puerto, no lo toca — R-RUN-13) y
el script **no tiene** registro de propiedad, así que no puede permitirse
tocar nada (R-STRATA-21, R-STRATA-22). En ropero, la comprobación correcta
es: *«¿el PID del estado de este puerto es el motor que levanto yo y sigue
vivo?»* — que es R-EST-8. **No se debe importar la comprobación del script
tal cual**: importarla haría que ropero se negara a arrancar un modelo
porque el escritorio ocupa 300 MiB de VRAM.

**R-STRATA-34** · El script **no es portable a macOS** por tres razones
concretas, y ropero tiene una restricción dura equivalente:
`readlink -f` (ropero tiene `real_dir`/`real_path`/`_rp_link_real` propios,
R-NF-4), `ln -sT` (GNU) y `stat -c%s` (GNU). Además usa `set -euo pipefail`,
mientras que el instalador de ropero evita `errexit` **a propósito** para que
un fallo implícito no aborte a medias sin mensaje. Evidencia:
`groot-strata.sh:48`, `:140`, `:288`; `ropero-code/install.sh:61-66`;
`bin/lib/00-utils.sh`.

**R-STRATA-35** · El `estado` del script usa `mktemp` + fichero temporal
para el `strata models list --installed --output json`, con un motivo
concreto y medido: en tubería, si el lector de Python se adelanta y cierra,
el `strata models list` recibe SIGPIPE y **aborta con core dump**. Es el
mismo patrón que el aviso de R-EST-3 y el mismo bug de `grep -q` + `pipefail`
documentado en la memoria del dueño. Evidencia: `groot-strata.sh:422-428`.

#### 4.13.5 El estado objetivo

**R-STRATA-36** · 🔶 **Strata es un motor de primera clase.** El objetivo
declarado es que `ropero <modelo>` lo sirva como a los demás, sin que el
usuario tenga que saber que Strata es especial. Hoy está **a medio
integrar**: ropero lo trata como un puntero al catálogo de Strata, y el
camino de Groot/Gyro-M necesita una preparación que ropero todavía no hace.

**R-STRATA-37** · 🔶 **La preparación es la parte que falta**, y son tres
cosas que hoy hace el script a mano: el **pack propio** del GGUF (R-STRATA-15),
el **expert-profile propio** (R-STRATA-16) y el **tune** medido sobre la
máquina (R-STRATA-20). El estado actual del trabajo:

- **Lo que Strata hace mejor que nadie**: bajar y preparar el motor. En
  Strata **no hay modo «descargar sin arrancar»**: el modelo se baja al
  arrancar. Lo que ropero **no** puede ofrecer es la vía «build» —bajar
  pesos, convertirlos, cuantizarlos— porque el motor de Strata no la tiene.
- **El equivalente manual de «build»** para Strata es **descargar el modelo
  a mano y configurarlo a mano**: eso es exactamente lo que hace
  `groot-strata.sh` (preparar → afinar → lanzar), y es el camino que hoy
  cubre el hueco mientras ropero no lo Asuma.
- **La regla de oro del camino manual**: el `setup.py` de Strata **reescribe
  el run config** con el perfil de expertos del modelo ya instalado, y por
  eso hay que preparar el profile **una vez** y arrancar con
  `strata serve --profile` en vez de con `strata launch` (R-STRATA-23).

**R-STRATA-38** · 🔶 **El script es el camino manual, no el camino
integrado.** Su valor en la especificación es el de **especificar el
contrato** de lo que ropero deberá hacer: el `profile` por GGUF, el pack por
GGUF, la reserva de VRAM, el tune verificado y el arranque por `--profile`.
Cuando ropero lo asuma, el script deja de ser necesario y su lógica pasa a
vivir en el engine.

#### 4.13.4 Tabla de contraste: script ↔ ropero

| Capacidad | `groot-strata.sh` | ropero hoy | Gap |
|---|---|---|---|
| Descargar el GGUF | `hf download` al directorio del modelo | `hf` a staging + rename (§ 4.10.4) | 🟢 compatible (mismo `hf`, mismo destino) |
| Verificar el GGUF | `strata models import --plan` | ✖ no existe | 🟡 **nuevo** |
| Run config por modelo | `setup.py` con `INSTALL_DIR` privado | ✖ un install compartido | 🔴 **nuevo** |
| Pack por GGUF | `setup.py --prepare-pack` | ✖ | 🔴 **nuevo** |
| Profile de expertos | round-robin 48×256 | ✖ (el motor usa el de fábrica) | 🔴 **nuevo** |
| Reserva de VRAM | `--vram-reserve-mib 1344` en los args | declarada y **comentada** | 🟠 **reanimar** |
| Afinar (tune) | `strata tune auto` + verificación de JSON | sólo como opción comentada | 🔴 **nuevo** |
| Lanzar | `strata serve --profile` | `strata launch --family/--model` | 🔴 **nuevo modo** |
| Parar | se niega (sin propiedad) | mata por puerto con PID verificado | 🟢 **ropero es mejor** |
| Estado | ficheros propios en disco | `$STATE_DIR/ports/<n>` | 🟠 **puente** |
| Idempotencia | `preparar` es idempotente por tamaño | idem | 🟢 |
| GPU libre | estricta (≤500 MiB) | idempotente por puerto | 🟡 **no portar tal cual** |
| Portabilidad | GNU-only (`readlink -f`, `ln -sT`, `stat -c`) | bash 3.2 | 🔴 **adaptar** |

**Veredicto**: el script es un **cliente de Strata de un solo uso**, no un
lanzador. Aporta tres cosas que ropero no tiene (pack propio, profile propio,
tune verificado) y una cuarta que ropero ya tiene mejor (parar). Integrarlo
**no** es «importar un script»: es **añadir un segundo modo de lanzamiento**
al engine de Strata, **un discriminante en el modelo** para distinguir
«puntero al catálogo» de «run config preparado», **un sitio en el catálogo
para los artefactos derivados** y **una operación `tune` de primer nivel**.

Y hay una comprobación que **no** hay que hacer: `strata launch` **no puede**
servir estos modelos, y no es un bug de configuración sino una incompatibilidad
de diseño documentada por el propio script. Cualquier propuesta que salga de
«arreglar el TOML de Groot» está muerta antes de empezar.

---

### 4.14 La fabricación de GGUFs (el gguffer)

#### 4.14.1 Qué es

**R-GGUF-1** · `ropero gguffer` es la **única** implementación de fabricación
de GGUFs (descarga → conversión → imatrix → cuantización → verificación →
model card). La comparten el CLI y la ruta `[build]` de los TOML (vía
`rp_model_build_argv`): **no hay dos Makers de GGUF**. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1-15`.

**R-GGUF-2** · El gguffer **no ejecuta el TOML**: lee sus campos y monta el
argv, igual que el resto del sistema. Un modelo es DATO (P-6).

**R-GGUF-59** · 🔶 **El gguffer tiene una precondición que ningún otro
comando tiene**: necesita un **motor llama.cpp disponible** (el oficial o el
fork), porque es el que convierte los pesos, cuantiza y une shards. El resto
de ropero habla con un motor ya instalado; el gguffer lo **usa como
herramienta**. Es la diferencia entre «arrancar un modelo» y «fabricar un
GGUF». Detalle en R-GGUF-54…R-GGUF-58.

#### 4.14.2 El contrato de la CLI

**R-GGUF-3** · La fuente es **una de tres**, y son excluyentes:
`--path <dir>` (un repo ya clonado), `--repo <owner/name>` (a clonar) o
`--gguf <fichero|owner/repo>` (un GGUF existente). Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1418`.

**R-GGUF-4** · Las opciones de salida son **dos y distintas**:
`--out NOMBRE|RUTA` (nombre/carpeta/ruta) y `--out-principal RUTA` (la ruta
**exacta** del primer quant de la tirada). Con varios quants, cada uno
necesita la suya, y `--out-principal` es la del que va a terminar en la
carpeta del modelo (la que espera el TOML); los demás se quedan en el
directorio normal con su nombre `<repo>-<quant>.gguf`. Y `--out-principal`
**exige** una ruta de fichero `.gguf`. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:27-32`, `:90-121`, `:1466-1485`.

**R-GGUF-5** · Con **varios** quants, `--out` **no** puede ser la ruta exacta
de un fichero: debe ser un nombre o un directorio. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1466-1479`.

**R-GGUF-6** · Las reglas del imatrix son **duras y comprobadas antes de
nada**:
- `--imatrix` (fichero local) es **excluyente** con el par
  `--imatrix-repo`/`--imatrix-file`;
- `--imatrix-repo` y `--imatrix-file` **van siempre juntos**;
- un imatrix **sin `--quant`** no tiene sentido: no hay nada que cuantizar
  (con la excepción de `--diagnostico`, que está exento).

Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:45-48`, `:1350-1371`,
`ropero-code/test/test-gguffer.sh:191-214`, `:242-246`.

**R-GGUF-7** · `--quant` es **repetible** y admite coma:
`--quant IQ2_M --quant IQ3_XS` o `--quant IQ2_M,IQ3_XS`. Y **cuantizar ya
cuantizado no es un error**: si el repo ya viene en el quant pedido, se
coloca el GGUF y se sale 0 **sin borrar nada**. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:27`, `:54-58`, `:1288-1295`;
`ropero-code/test/test-gguffer.sh:152-156`.

#### 4.14.3 El ahorro que da forma al diseño

**R-GGUF-8** · 🔴 **La comprobación de viability ocurre ANTES de descargar
los pesos**. Si el quant pedido es distinto del que ya traen los pesos, el
gguffer **falla con rc≠0, sin bajar nada**, y **sugiere el repo hermano sin
cuantizar** que sí serviría (`acme/modelo-BF16`).

> «el ahorro de 60 GB tontos» — `ropero-code/test/test-gguffer.sh:162-169`

La razón es dura: el motor **no re-cuantiza** (`requantizing from type X is
disabled`). Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1619-1685`;
`ropero-code/test/fakes/gguffer/llama-quantize:121-129`.

**R-GGUF-9** · 🔴 **Un repo ya cuantizado (NVFP4) se conserva, no se
cuantiza**: el GGUF sale con `general.file_type = 39` (NVFP4) y el mensaje
lo dice. Y si se pide un quant **distinto**, es error. La regla del motor es
la regla del gguffer: *lo que ya está en el quant pedido no se toca*.
Evidencia: `ropero-code/test/test-gguffer.sh:144-161`, `:219-223`.

**R-GGUF-10** · Un repo en **FP8** sí puede cuantizarse desde su F16
dequantizado, y el gguffer **avisa** de que dequantiza (se pierde la ventaja
del quant de fábrica). Evidencia: `ropero-code/test/test-gguffer.sh:235-240`.

#### 4.14.4 La cabeza MTP: el bloque que el imatrix no cubre

**R-GGUF-11** · El imatrix se calcula sobre el **modelo base** (64 capas),
pero el GGUF lleva la cabeza MTP como **un bloque más** (el 65). Ese bloque
no está en el imatrix, y si el quant es *very-low-bit* (IQ2_M, IQ3_XXS…) el
cuantizador aborta con «Missing importance matrix for tensor blk.64…».

Es lo que hacía bartowski con su IQ2_M del Qwen3.8: dejó el bloque de la MTP
en Q4_0, que no exige imatrix. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:123-140`.

**R-GGUF-12** · La cobertura del imatrix se comprueba **por bloque entero**,
no tensor a tensor: los tensores que el imatrix no tiene son normalmente las
normas f32, que no se cuantizan y no exigen imatrix; lo que aborta al
cuantizador es un bloque entero sin cobertura. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:123-140`.

**R-GGUF-13** · El bloque MTP se localiza por **metadata**, no por el nombre
del tensor: los tensores de la MTP se llaman como los de cualquier otro
bloque (`blk.64.attn_k.weight`), **sin «nextn»** en el nombre. Buscar
«nextn.» no encuentra nada, y por eso el fix del MTP no se aplicaba y la
cuantización moría con el mismo error. Lo que sí lo dice es:

```
bloque_mtp = block_count - nextn_predict_layers
```

Qwen3.8 denso: `block_count=65`, `nextn=1` → bloque 64. `fable`: 41 → 40.
`nemotron`: `nextn=0` → no hay MTP. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:151-176`.

**R-GGUF-14** · Cuando el error es de imatrix, el gguffer **traduce** el
mensaje opaco del cuantizador («Missing importance matrix for tensor
blk.64.attn_k.weight») al **lugar del que sale**: «es el bloque 64, el de la
cabeza MTP; el imatrix se calcula sobre el modelo base, que no la tiene», y
ofrece las dos salidas (dejar ese bloque en Q4_0 con `--tensor-type`, o
convertir sin MTP con `--no-mtp` y renombrar el GGUF). Evidencia:
`bin/lib/cmd-gguffer.sh` (fase 3, cola del error de cuantización).

#### 4.14.5 Las seis fases

**R-GGUF-15** · El gguffer tiene **seis fases** en orden fijo:
1. **descargar / clonar** el repo (o usar el GGUF traido);
2. **convertir** a GGUF (o cuantizar el GGUF existente sin pasar por el
   conversor);
3. **cuantizar** (imatrix → cuantizador), con un **log aparte** para que el
   último error del cuantizador diga por qué (el mensaje del gguffer solo no
   alcanza);
4. **verificar** los GGUFs;
5. **limpiar**;
6. **escribir la procedencia**.

Evidencia: `bin/lib/cmd-gguffer.sh` (fases 1–6, comentadas).

**R-GGUF-16** · 🔴 **Al fallar la cuantización NO se borra nada**: el clon de
los safetensors y el GGUF intermedio **se quedan**, y el error dice
exactamente cómo reintentar **sólo** la cuantización (que es la parte cara).
Un fallo de cuantización no debe costar 60 GB de descarga. Evidencia:
`bin/lib/cmd-gguffer.sh` (fase 3, `conservo los pesos en: $work`).

**R-GGUF-17** · 🔴 **Los pesos sólo se borran si el GGUF supera la
verificación**: «NO borro los pesos: están en $work». Si el GGUF está
corrupto, tener los safetensors vale más que 60 GB de disco. Y el GGUF de
**entrada** (traído por el usuario) **jamás** se borra. Evidencia:
`bin/lib/cmd-gguffer.sh` (fases 4 y 5).

**R-GGUF-18** · El intermedio (F16) se borra **después de todos** los quants,
no en cada uno: con varios quants en una tirada, el siguiente necesita el
F16. Y si el F16 **es** el resultado (quant `F16`), no se toca. Evidencia:
`bin/lib/cmd-gguffer.sh` (fase 5).

#### 4.14.6 La procedencia: por qué no es opcional

**R-GGUF-19** · 🔴 **Todo GGUF producido deja un `<out>.fuente.json`** con
de dónde salió el modelo, con qué quant, con qué importance matrix, y si la
cabeza MTP quedó en un quant aparte.

> «Un GGUF sin saber CÓMO se hizo no se puede publicar con vergüenza»

La procedencia declara el quant **por nombre** (`"quant": "Q4_K_M"`), no por
número. Es lo que va al model card. Evidencia:
`bin/lib/cmd-gguffer.sh` (fase 6); `ropero-code/test/test-gguffer.sh:179-182`.

**R-GGUF-20** · El gguffer **no miente sobre el resultado**: si los pesos
venían en NVFP4 y no se cuantizó nada, el mensaje final dice «NO he
cuantizado nada: el GGUF se ha quedado en **NVFP4**» —el quant **real**, no el
supuesto— y da la receta para cuantizarlo **reutilizando el intermedio**,
sin volver a descargar. Evidencia: `bin/lib/cmd-gguffer.sh` (cola de la fase 6).

#### 4.14.7 `ggufcheck`: el que sólo lee la cabecera

**R-GGUF-21** · `bin/python/ggufcheck.py` lee **sólo la cabecera** en todos
los modos: ningún `read()` puede pasar de 100 MiB aunque el fichero sea de
300 MiB (fichero disperso). Y distingue dos espacios de números que se
confunden: `--file-type` (`general.file_type` / `llama_ftype`) y
`--tipo-predominante` (el `ggml_type` del tensor). Evidencia:
`ropero-code/test/test-ggufcheck.sh:38-47`, `:51-75`.

**R-GGUF-54** · 🔶 **El gguffer exige un motor llama.cpp disponible** —el
oficial o el fork— y esa es una **precondición**, no un detalle: sin ella no
hay quien convierta los pesos, ni quien cuantice, ni quien una los shards.

> «ropero gguffer sólo funciona si está llama-server disponible, ya sea el
> oficial o mi fork» — el dueño, 2026-11-12

El motor se busca en **`$LLAMA_DIR`** (por defecto `~/llama.cpp`), y de ahí
toma cuatro cosas:

| Qué | Dónde |
|---|---|
| el conversor | `$LLAMA_DIR/convert_hf_to_gguf.py` (**sin alternativa**) |
| el cuantizador | `command -v llama-quantize` → si no, `$LLAMA_DIR/build/bin/llama-quantize` |
| el unidor de shards | `command -v llama-gguf-split` → si no, `$LLAMA_DIR/build/bin/llama-gguf-split` |
| `libggml` (para `ggufcheck`) | `$LLAMA_DIR/build/bin/libggml.{so,dylib}` o `$LLAMA_DIR/build/lib/libggml.so` |

Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1376-1392`, `:840-846`;
`ropero-code/bin/lib/01-env.sh:33-34`.

**R-GGUF-55** · 🔶 **El conversor es el punto que no admite sustituto**: sin
`$LLAMA_DIR/convert_hf_to_gguf.py` **no hay conversión**, y por tanto no hay
GGUF — con o sin quant. El cuantizador y el unidor de shards sí tienen
alternativa en el PATH; el conversor, no. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1377`, `:1992`.

**R-GGUF-56** · 🔶 **La precondición se comprueba ANTES de descargar los
pesos**, con un mensaje que nombre el motor que falta y la variable que lo
apunta. Hoy **no hay guard**: si el motor no está, la conversión falla y el
clasificador de errores la etiqueta como «faltan ficheros en el clon: la
descarga está incompleta» — un diagnóstico **falso** que empuja a borrar y
rebajar 60 GB por un problema que no es la descarga (H-27). Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1957` (el único `-f` sobre el motor es para el
parche), `:2035-2041` (el clasificador).

**R-GGUF-57** · 🔶 **Los dos motores no dejan el clon en el mismo sitio**, y
eso es una costura del gguffer:

| Engine | Clon | ¿Lo encuentra el gguffer? |
|---|---|---|
| `llama-server` (el fork) | `$LLAMA_DIR` = `~/llama.cpp` | **sí**, de serie |
| `llama-server-official` | `$LLAMA_OFFICIAL_DIR` = `~/llama.cpp-official` | **no**: el gguffer sólo mira `$LLAMA_DIR` |

⇒ Con **sólo** el motor oficial instalado, `ropero gguffer` no encuentra el
motor salvo que se exporte `LLAMA_DIR=~/llama.cpp-official`. Ni el README ni
`docs/` mencionan `LLAMA_DIR` ni `LLAMA_OFFICIAL_DIR` (sí están en
`--help` de los engines: R-GGUF-33). Evidencia:
`ropero-code/bin/lib/01-env.sh:33-34`; `ropero-code/bin/lib/engines/llama-server.sh:78`, `:155`;
`ropero-code/bin/lib/engines/llama-server-official.sh:60`, `:77`;
`ropero-code/bin/lib/cmd-gguffer.sh:1376`.

**R-GGUF-58** · 🔶 **Ni `llama-quantize` ni `llama-gguf-split` se enlazan** en
`$BIN_DIR` al instalar un engine: los engines instalan `llama-server` y
`llama-server-official`, nada más. Es decir, la alternativa por el PATH que
el gguffer contempla (R-GGUF-54) **no la activa ningún instalador**: en la
práctica, el motor se encuentra **o no se encuentra**. Evidencia:
`ropero-code/bin/lib/cmd-install.sh:599-613` (el enlace es el `basename` del binario
resuelto), `bin/lib/engines/llama-server.sh` (enlaza `llama-server`).

**R-GGUF-22** · Los modos son `--meta`, `--tipo-predominante`, `--file-type`,
`--nombres`, `--bloque`, `--info`, y la **validación por defecto**, y cada uno
tiene una salida exacta esperada. Evidencia:
`ropero-code/test/test-ggufcheck.sh:38-47`.

**R-GGUF-23** · Un GGUF **inválido** falla con **rc=1, mensaje claro y sin
traceback**: un índice de tensores incompleto (declara 3, tiene 2) informa del
recuento real; un `n_tensors` imposible (declara 1000, trae 2) también; y un
`n_tensors = 2^40` en un fichero de 1 GiB **falla rápido** (<5 s, «GGUF
inválido»), **no** se cuelga ni reserva 1 GiB. Evidencia:
`ropero-code/test/test-ggufcheck.sh:115-134`.

**R-GGUF-24** · El parámetro deprecado `solo_cabecera=True` **se sigue
aceptando** (y se ignora): compatibilidad hacia atrás. Y `model_card.py
--gguf` debe terminar con rc=0 **sin** `TypeError` y generar README y CUANT.
Evidencia: `ropero-code/test/test-ggufcheck.sh:79-93`.

**R-GGUF-25** · `ggufcheck` se puede ejecutar contra **cualquier** GGUF, no
sólo los de ropero: acepta `--lib <ruta>` para la biblioteca de ggml. Y en el
gguffer, la verificación usa primero el **split** (`--dry-run`) y, si no está
disponible, cae al `ggufcheck` de python. Evidencia:
`bin/lib/cmd-gguffer.sh` (fase 4); `bin/python/ggufcheck.py`.


#### 4.14.8 La taxonomía del checkpoint: cinco clases

**R-GGUF-26** · El preflight **clasifica el checkpoint antes de bajar nada**,
leyendo `config.json` y `hf_quant_config.json` (del repo por HTTP con
`curl -fsS`, o del directorio local) y devolviendo una de **cinco clases**:

| Clase | Qué significa | `file_type` esperado |
|---|---|---|
| `NVFP4` | los pesos **ya vienen** en NVFP4; el conversor los conserva | 39 |
| `MXFP4` | los pesos ya vienen en MXFP4 (gpt-oss); el motor lo repackea a MXFP4_MOE | 38 |
| `MIXTO` | *mixed-precision*: NVFP4 + FP8 en el mismo checkpoint | 39 |
| `DEQUANT` | FP8/AWQ/GPTQ: el conversor los **dequantiza** a F16 | 1 |
| `NINGUNO` | sin quant de fábrica (BF16/F16) | 1 o 32 |

Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:288-378`, `:1647-1673`.

**R-GGUF-27** · Las reglas de parseo tienen tres trampas resueltas: el
`quant_algo` va **anidado** en `quantization`, no en la raíz; el
`quant_method` puede venir de `producer.name`; y **un 404 de HF no es JSON
válido** y se ignora ⇒ un repo inexistente se lee como `NINGUNO`, que es lo
correcto. Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:223-245`, `:279-283`,
`:305-306`.

**R-GGUF-28** · El plan de cuantización decide por cada quant pedido en
**cuatro situaciones**: (1) el GGUF ya está en ese quant → **no se
cuantiza**; (2) está en *otro* quant → **imposible** (el motor no
re-cuantiza); (3) no está cuantizado y el pedido es un quant real → se
cuantiza y hace falta imatrix; (4) no está cuantizado y el pedido es «plano»
(F32/F16/BF16) → simple conversión de dtype, **sin imatrix**.
Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:426-483`.

**R-GGUF-29** · «Cuantizado» significa: todo `file_type` salvo `""`, `0`
(F32), `1` (F16) y `32` (BF16). Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:389-394`.

**R-GGUF-30** · Un **quant desconocido** aborta con rc 1 **antes de
descargar**: la lista se parsea del `llama-quantize --help` del motor, en dos
versiones (con alias y canónica, `grep -v "alias for"`), y el error dice
`ropero gguffer --list-quants`. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1440-1465`.

#### 4.14.9 La superficie completa de la CLI

**R-GGUF-31** · `ropero gguffer` **no tiene subcomandos**: es un comando con
flags, con dos modos activados por flag: `--list-quants` (o `--quant` **sin
valor**) lista los quants del motor y los publicados para ese modelo base, y
`--diagnostico` imprime el informe de por qué no se cuantiza **sin
cuantizar**. Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1324-1326`, `:1391-1415`,
`:2060-2064`.

**R-GGUF-32** · Los flags completos, con su obligatoriedad:

| Flag | Argumento | Por defecto | Condición |
|---|---|---|---|
| `--repo` | `OWNER/NAME` | — | una de `--repo`/`--path`/`--gguf` |
| `--path` | `DIR` | — | excluyente con `--repo` |
| `--gguf` | `FICHERO` \| `OWNER/REPO` \| `OWNER/REPO:fichero` | — | — |
| `--gguf-file` | `NOMBRE` | — | obligatorio si el repo tiene varios GGUFs sin cuantizar |
| `--quant` | `TIPO[,TIPO]` | — | repetible; **sin valor ⇒ `--list-quants`** |
| `--imatrix` | `FICHERO` | — | excluyente con el par del repo |
| `--imatrix-repo` / `--imatrix-file` | | — | **siempre juntos** |
| `--out` | `NOMBRE` \| `RUTA/` \| `RUTA.gguf` | `$GGUF_DIR` | con varios quants, no puede ser fichero |
| `--out-principal` | `RUTA.gguf` \| `DIR/` | — | opcional |
| `--from-subdir` | `DIR` | — | subcarpeta del clon con los pesos |
| `--include` | `PATRÓN` | — | filtro de `hf download` |
| `--intermedio` | `f16` \| `bf16` | `f16` | valor inválido ⇒ rc 2 |
| `--keep-source` / `--keep-intermedio` / `--no-mtp` / `--force` | — | apagado | — |
| `--list-quants` / `--diagnostico` | — | apagado | — |
| `--chat-template` | `FICHERO` | — | **no documentado en `--help`** |
| `--tokenizer` | `FICHERO` | — | **no documentado en `--help`** |

Todos aceptan `--flag=valor`. Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1271-1327`.

**R-GGUF-33** · Las **variables de entorno** que gobiernan el comportamiento
y **no salen en `--help`**: `GGUF_DIR` (`$HOME/models/gguf`), `GGUF_WORK_DIR`
(`$HOME/.cache/ropero/gguffer`), `LLAMA_DIR` (`$HOME/llama.cpp`), `ROPERO_ROOT`,
`MTP_QUANT` (`Q4_0`), `ROPERO_CHAT_TEMPLATE`. Se descubren leyendo el
código. Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:975`, `:1271`, `:1376`, `:1476-1477`,
`:1801`.

**R-GGUF-34** · La ruta `[build]` del TOML se traduce al argv del gguffer
por una **tabla explícita** en `modelconf.py`:
`repo→--repo`, `gguf→--gguf`, `gguf_file→--gguf-file`, `imatrix→--imatrix`,
`imatrix_repo→--imatrix-repo`, `imatrix_file→--imatrix-file`,
`out→--out`, `out_principal→--out-principal`, `from_subdir→--from-subdir`,
`include→--include`, `intermedio→--intermedio`; más las banderas
`keep_source`, `keep_intermedio`, `no_mtp`, `force`. Y `quant` admite lista
o cadena con comas. El contrato es explícito: **la receta del TOML y el
comando a mano son lo mismo**. Evidencia:
`ropero-code/bin/python/modelconf.py:1248-1263`, `:1326-1334`;
`ropero-code/bin/lib/02-modelconf.sh:323-336`; `ropero-code/bin/quant/ACRITO.md:23-26`.

#### 4.14.10 La conversión: tres arreglos que existen por casos reales

**R-GGUF-35** · El **nombre del intermedio describe lo que lleva dentro**:
si el checkpoint es NVFP4 el intermedio es `-NVFP4`, si es MXFP4
`-MXFP4_MOE`, si no `-F16`/`-BF16` según `--intermedio`. No se pone «-F16» a
un fichero NVFP4. Y al reintentar se **reutiliza** el intermedio existente:
para F16/BF16 se busca cualquiera de los dos; para NVFP4/MXFP4_MOE sólo el
exacto. Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1630-1635`, `:1802-1826`.

**R-GGUF-36** · `--no-mtp` **sólo se pasa si el conversor lo entiende**: se
comprueba con `grep -qE "no-mtp|no_nextn"` sobre el conversor. Motivo: la
clase `Qwen3_5TextModel` del motor de ropero **ignora** el flag, y usarlo
aun así aborta la conversión. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1919-1925`; `:187-200`.

**R-GGUF-37** · El **tokenizer se arregla antes de convertir**, en dos pasos:
(i) se inyecta el `chat_template.jinja` del repo dentro del
`tokenizer_config.json` — el conversor de llama.cpp **nunca** lee el
`.jinja`— y (ii) se cambia `tokenizer_class: TokenizersBackend`
(transformers v5) por `PreTrainedTokenizerFast`, guardando un `.bak`.
Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1070-1141`, `:1984-1989`.

**R-GGUF-38** · La conversión tiene **dos reintentos automáticos**:
(1) `Tokenizer class … does not exist` ⇒ se arregla el tokenizer y se
reintenta; (2) `--mtp / --no-nextn are not supported` ⇒ se quita `--no-mtp`
del argv y se reintenta. Y si aun así falla, se hace `tail -20` del log del
conversor, se **borra el GGUF parcial** y se **clasifica la causa**: sin
disco (`no space left|disk quota|write error`), sin RAM
(`MemoryError|OutOfMemory|Cannot allocate memory|Killed`), o clon
incompleto. Siempre se **conservan los pesos** y se imprime el comando
exacto de reintento. Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1992-2051`.

**R-GGUF-39** · Las flags del cuantizador van **antes de los posicionales**:
`llama-quantize` sólo parsea opciones mientras el argumento empiece por
`--`; si no, muere con `invalid nthread '--imatrix'`. El argv final es
`llama-quantize [--imatrix F] [--tensor-type …] <f16> <salida> <QUANT>`.
Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:2213-2219`.

**R-GGUF-40** · El bloque MTP se localiza por **dos vías que se unen**: la
metadata (`bloque_mtp = block_count - nextn_predict_layers`) **y** la
comparación bloque a bloque contra el imatrix. Cada bloque sin cobertura
recibe `--tensor-type "blk\.<N>\..*=Q4_0"`, con `Q4_0` configurable por
`MTP_QUANT`. Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:2115-2166`, `:878-906`.

**R-GGUF-41** · El `general.file_type` (llama_ftype: NVFP4=39,
MXFP4_MOE=38, BF16=32, F16=1) **nunca** se compara con el `ggml_type`
(NVFP4=40, MXFP4=39): son espacios de números distintos y «nunca coincide».
El gguffer lee el file_type **real** del GGUF y rehace el plan con ese dato.
Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1435-1438`, `:2072-2098`;
`ropero-code/bin/quant/quant-tipos.sh:9-11`.

#### 4.14.11 Los parches del motor

**R-GGUF-42** · El motor de ropero es un **clon privado**
(`Lorenzo-SF/ropero-llama-turboquant`) con fallback al fork público, y
recibe **parches** que se aplican con `patch -p1` y exigen **recompilar**
después (sin recompilar, el parche no sirve). El estado de los parches se
detecta **por el código**, no por git, y antes de parchear se guarda una
copia `.ropero.bak`. Si tras aplicar siguen faltando parches ⇒ error
(rc 1). Evidencia: `ropero-code/bin/quant/aplicar-parche-motor.sh:103-118`, `:153-158`,
`:199-203`, `:206-220`; `ropero-code/bin/lib/engines/llama-server.sh:77-117`.

**R-GGUF-43** · La aplicación del parche mixed-precision en el gguffer es
**fail-open** (`patch -p1 … || true`): si el parche no entra, se sigue y la
conversión fallará después con un error menos claro. Es un fail-open
justificado —el error real aparece en la conversión— pero conviene saberlo.
Evidencia: `ropero-code/bin/lib/cmd-gguffer.sh:1959`.

#### 4.14.12 La capa de scripts (`bin/quant/`)

**R-GGUF-44** · `bin/quant/` es una **capa de guiones** sobre el gguffer, y
la tabla `modelos.tsv` (7 columnas: `n pesos q imr imf flags fuente`) es su
**fuente de verdad** —los guiones no llevan la lista dentro—. La ruta de
destino **no se calcula en el script**: se le pide a ropero
(`ropero path "$modelo"`), para no duplicar la verdad de los TOMLs.
Evidencia: `ropero-code/bin/quant/cuantizar-todos.sh:5-6`, `:92-102`, `:113`, `:227`;
`ropero-code/bin/quant/README.md:71-72`, `:78-81`.

**R-GGUF-45** · El **nombre del destino nunca es mentiroso**: si el nombre
de la ruta del TOML **no contiene** el nombre del quant principal, el GGUF
va a la carpeta con su nombre propio en vez de llevar el del principal («el
launcher creería que el modelo es el IQ2_M»). Los quants secundarios van a
`$GGUF_DIR/_variantes/<modelo>/`. Evidencia:
`ropero-code/bin/quant/cuantizar-todos.sh:146-186`; `ropero-code/bin/lib/cmd-gguffer.sh:105-118`,
`:1853-1858`.

**R-GGUF-46** · Un **fallo de un modelo no para la tirada** (salvo
`--parar`): «un modelo que no se deja fabricar no puede impedir que los
demás sí». Cada modelo tiene su log y el resumen final lista OK/FALLO por
modelo. Evidencia: `ropero-code/bin/quant/cuantizar-todos.sh:35-37`, `:204-212`,
`:244-272`.

**R-GGUF-47** · `bin/quant/todo.sh` es el guion completo y su **orden es
significantivo**: **cuantizar → verificar → subir**. La verificación va
**por delante** de la subida y, si algún GGUF falla, **no se sube nada**
(`exit 1`). Evidencia: `ropero-code/bin/quant/todo.sh:47-92`, `:66-82`.

**R-GGUF-48** · `verificar-gguf.sh` es la **verificación posterior sin GPU**:
exige ≥ 100 MiB, metadata legible, `arch` presente, tensores > 0 y
`block_count`; la ausencia de `nextn_predict_layers` **no** es error (se
informa como «(ninguno)»); y si se le pasa el quant esperado, compara
`general.file_type` con el número de ese quant. Códigos: 0 ok · 1 fallos ·
2 sin argumentos. Evidencia: `ropero-code/bin/quant/verificar-gguf.sh:28-96`.

**R-GGUF-49** · La subida a HF (`subir-hf.sh`) crea el repo
`<HF_USER>/<nombre-modelo>-<QUANT>-GGUF` (el nombre se deriva del fichero y
del `general.file_type` real), sube el GGUF, el imatrix si está en la caché,
el `README.md` y el `CUANTIZATION.md`, y deja un marcador `.hf-uploaded` en
la misma carpeta. El `base_model` sale del **propio GGUF**
(`general.base_model.0.repo_url`), no de una tabla. Los GGUFs de
`_variantes/` **no** se suben. Evidencia:
`ropero-code/bin/quant/subir-hf.sh:46`, `:105-129`, `:184`, `:200-300`.

**R-GGUF-50** · El **model card sale del propio GGUF**, no de una tabla, y
**debe declarar** que el modelo no es del cuantizador; si no hay imatrix, lo
dice («sin importance matrix… suele dar peor calidad»). Y el `.fuente.json`
es la **única** fuente de qué imatrix se usó, porque el conversor no escribe
`base_model` en la metadata. Evidencia:
`ropero-code/bin/python/model_card.py:21-27`, `:186-188`, `:272`, `:305-309`;
`ropero-code/bin/quant/subir-hf.sh:230-252`.

#### 4.14.13 Tres discrepancias del gguffer, medidas

**R-GGUF-51** · 🟠 **`subir-hf.sh --subir` NO verifica el GGUF que sube**:
sólo mira el marcador `.hf-uploaded`. La puerta de verificación está en el
gguffer (fase 4) y en `todo.sh`; llamando al script de subida directamente,
**un GGUF no verificado se sube**. El `ropero-code/README.md:84-85` afirma «un GGUF que
no pasa la verificación no se sube», y eso sólo se cumple si se pasa por
`todo.sh` o si lo fabricó el gguffer. Evidencia:
`ropero-code/bin/quant/subir-hf.sh:203-206`, `:278`; `ropero-code/bin/quant/README.md:82-83`.

**R-GGUF-52** · 🟠 **La escritura de la procedencia es fail-open y su código
de retorno se ignora**: si `_gguffer_escribe_fuente` falla, el GGUF queda
**sin** `.fuente.json` y la tirada **sale 0**. Un artefacto sin procedencia
publicable, con el código de salida diciendo que todo fue bien. Evidencia:
`ropero-code/bin/lib/cmd-gguffer.sh:1006`, `:2315`.

**R-GGUF-53** · 🟡 **El comentario de `cuantizar-todos.sh` está desfasado
respecto al código**: dice que el gguffer borra los safetensors «tanto si
todo va bien como si algo falla»; el código hace lo contrario (los conserva
en caso de fallo, a propósito). Y `bin/quant/patches/parche-json-schema-to-grammar.patch`
es un **duplicado exacto** de un hunk de otro parche que **ningún script
aplica**. Y `model_card.py::_tipos()` resuelve los alias **al revés** que
`quant-tipos.sh::quant_nombre`: Python deja el último (el alias) y bash deja
el primero (el canónico). Evidencia:
`ropero-code/bin/quant/cuantizar-todos.sh:30-33` vs. `ropero-code/bin/lib/cmd-gguffer.sh:2040-2047`;
`bin/quant/patches/`; `bin/python/model_card.py` vs.
`bin/quant/quant-tipos.sh`.

---

### 4.12 Contrato operativo vigente (working tree)

Esta sección fija el comportamiento operativo implementado en el árbol de trabajo
y complementa a [ADR-008](../docs/adr/ADR-008-configuracion-externa-y-bootstrap.md)
y [ADR-010](../docs/adr/ADR-010-comportamiento-operativo-flm-logs-y-remove.md).

#### 4.12.1 Configuración, catálogo y manifiestos

- **Catálogo por defecto (uso normal):** externo en
  `<directorio_config>/models/<motor>/`, con manifiestos en
  `<directorio_config>/engines/`. `<directorio_config>` es
  `$XDG_CONFIG_HOME/ropero` o `~/.config/ropero`.
- **Sin autodiscovery de repo:** los TOML/manifiestos del repositorio son para
  desarrollo y pruebas y no se auto-descubren en uso normal.
- **Overrides preservados:** `ROPERO_MODELS_DIR` y `ROPERO_ROOT`. Si apuntan al
  directorio `models/` de una raíz de catálogo/repositorio, se mantiene la
  resolución de manifiestos en la raíz hermana `<raíz>/engines/`.
- **Bootstrap:** en el primer uso se crea el directorio de configuración y
  `config.toml` por defecto de forma idempotente y **sin sobrescribir**.
- **Precedencia:** `variable de entorno > config.toml > valor por defecto` para
  `models_dir` (`ROPERO_MODELS_DIR`), `gguf_dir` (`GGUF_DIR`), `safetensors_dir`
  (`SAFETENSORS_DIR`), `bin_dir` (`ROPERO_BIN_DIR`) y `logs_dir` (`LOG_DIR`).
  Por defecto, `logs_dir` es `<directorio_config>/logs`.
- **Manifiestos de usuario:** no embutidos; se leen desde el directorio
  `engines/` asociado a la raíz del catálogo en uso.
- **Comandos:** `ropero config` muestra rutas/origen; `ropero doctor` incluye
  estado de configuración y dependencias; `ropero template` sugiere plantilla sin
  escribirla. No modifican artefactos de modelos, aunque el bootstrap
  idempotente de configuración puede ejecutarse si falta.
- **Migración:** copiar TOML heredados a `~/.config/ropero/models/<motor>/` y
  manifiestos a `~/.config/ropero/engines/` (o a las rutas configuradas).

#### 4.12.2 FLM delegado (operativo)

- **Schema/identidad:** el tag primario es `[model].path` si no está vacío; en
  su defecto, el valor de `--decision` en `[args].base`. Los tags relevantes se
  derivan del modelo (primario y auxiliares declarados).
- **Verbos delegados:** instalación (`pull`), eliminación (`remove`) y listado
  (`list`) según lo declare el manifiesto del motor; el arranque puede servirse
  como `flm serve <tag> ...` con defaults del manifiesto y `--ctx-len` si falta.
- **Estado:** la consulta `flm list --json` es de **mejor esfuerzo**; una
  respuesta no concluyente no debe interpretarse como verificación fuerte. No se
  afirma verificación extremo a extremo de FLM en este contrato.

#### 4.12.3 Logs, colorizado y uso

- **Foreground:** salida en vivo del motor en terminal (colorizada cuando
  procede) y escritura del log del modelo para diagnóstico/estado.
- **`--background`:** proceso desacoplado, sin salida en terminal ni emisión de
  `ROPERO_PORT`; los logs se conservan bajo `logs_dir`.
- **Colorizado:** tratamiento **común y semántico/heurístico**; no se afirma
  colorizador específico por motor.
- **`usage.log`:** registro local append-only en `logs_dir/usage.log` con datos
  mínimos (instante, modelo, motor, puerto si se conoce, resultado y modo), sin
  claves de API ni flags/argumentos.

#### 4.12.4 Eliminación de modelos (`ropero <alias> remove`)

- **Plan:** sólo artefactos derivables del TOML y rutas resueltas (ficheros
  declarados/auxiliares de runtime) y, si aplica, operación delegada FLM.
- **Confirmación:** interactiva en TTY antes de borrar; sin TTY o sin respuesta
  afirmativa, no se elimina nada.
- **Límites:** no se borran directorios compartidos ni datos no declarados; el
  directorio concreto del modelo en `GGUF_DIR` sólo se retira si queda vacío tras
  borrar ficheros concretos.
- **Safetensors:** si `[model].path` identifica checkpoint/directorio
  safetensors, la eliminación automática se **bloquea** por seguridad (retirada
  manual si procede).
- **FLM:** elimina el **tag primario** mediante `flm remove <tag>` y muestra los
  tags declarados; no elimina automáticamente otros tags declarados por esta
  operación.


## 5. Matriz de trazabilidad

### 5.1 Recuento por área

**317 requisitos**, todos con identificador único, todos con evidencia
localizable (`fichero:línea`) salvo los marcados `[derivado]`, que son
consecuencia de otro requisito y no de una línea de código concreta, y los
marcados 🔶, que son el **estado objetivo declarado por el dueño** y todavía
no están en el código (17 de ellos).

| Área | Requisitos | Ficheros fuente principales | Cobertura de tests |
|---|---|---|---|
| `R-CTX` · Contexto y plataformas | 3 | `bin/lib/engines/*.sh` (hook `compatible`) | 🔴 ninguno |
| `R-ARQ` · Arquitectura | 11 | `bin/ropero`, `bin/lib/*` | 🟡 parcial |
| `R-DATA` · Modelo de datos | 23 | `models/_template.toml`, `bin/python/modelconf.py` | 🟢 parcial |
| `R-ENG` · Contrato de engine | 7 | `bin/lib/03-engines.sh`, `bin/lib/engines/*.sh` | 🟢 2 de 8 motores |
| `R-CLI` · Línea de comandos | 17 | `bin/ropero`, `bin/lib/cmd-*.sh` | 🟡 parcial |
| `R-RUN` · Ciclo de vida | 17 | `bin/lib/04-launch.sh` | 🟡 parcial |
| `R-EST` · Estado y puertos | 15 | `bin/lib/04-launch.sh` | 🟡 parcial |
| `R-API` · Endpoint | 25 | `bin/server/endpoint.py` | 🔴 **rota** (R-TST-18) |
| `R-ROUTE` · Enrutado | 5 | `bin/router.sh` | 🔴 ninguno |
| `R-INST` · Instalación | 27 | `install.sh`, `bin/lib/cmd-install.sh` | 🔴 ninguno |
| `R-MOD` · Catálogo y descargas | 31 | `bin/lib/cmd-models.sh`, `bin/lib/05-download.sh` | 🟢 bueno |
| `R-TST` · Batería | 28 | `test/` | 🟢 es la batería |
| `R-OC` · opencode | 11 | `opencode-plugin/index.ts` | 🔴 ninguno |
| `R-STRATA` · Strata + `groot-strata.sh` | 38 | `bin/lib/engines/strata.sh`, `bin/server/strata-run.sh`, `groot-strata.sh` | 🟢 bueno (ropero) · 🟡 el script no |
| `R-GGUF` · Fabricación GGUF | 59 | `bin/lib/cmd-gguffer.sh`, `bin/quant/*`, `bin/python/{ggufcheck,model_card}.py` | 🟢 bueno (no hermético) |

### 5.2 La matriz de los requisitos que no están garantizados

La parte que de verdad importa: **qué requisitos tienen una prueba que los
garantiza y qué no**. SeAnotó cada requisito cuyo comportamiento observable
puede comprobarse con el estado actual del repo.

| Requisito | ¿Verificado por un test? | Nota |
|---|---|---|
| `R-TST-18` (guard de auth) | ❌ **falso positivo** | el test pasa en verde con la auth rota |
| `R-API-1`…`R-API-22` | ❌ | bloqueados por R-TST-18 |
| `R-INST-1`…`R-INST-27` | ❌ | 0 tests de `install.sh` y `cmd-install.sh` |
| `R-OC-1`…`R-OC-11` | ❌ | 0 tests del plugin |
| `R-ROUTE-1`…`R-ROUTE-5` | ❌ | `router.sh` no lo toca ningún test |
| `R-CTX-1`…`R-CTX-3` (compatibilidad y madurez) | ❌ | el hook `compatible` no tiene test |
| `R-GGUF-54`…`R-GGUF-58` (precondición del motor) | ❌ | 🔶 sin guard: H-27, H-28, H-29 |
| `R-CLI-14`…`R-CLI-17` (superficie y `stats`) | ❌ | 🔶 estado objetivo, no implementado |
| `R-DATA-21` (`name` = fichero) | ❌ | el validador no lo comprueba (H-23) |
| `R-EST-15` (motores ajenos) | ❌ | 🔶 sin decidir (H-26) |
| `R-RUN-13` (sello) | ❌ | verificado **a mano** en esta auditoría |
| `R-MOD-24`…`R-MOD-25` (symlink) | ✅ | `test-models-symlink.sh` |
| `R-GGUF-8`…`R-GGUF-10` (viability) | ⚠️ | la **lógica** se prueba; el **entorno** no (R-TST-19) |
| `R-MOD-20` (no hay hash) | ✅ | verificado por lectura **y** por test |
| `R-STRATA-1`…`R-STRATA-13` | ✅ | `test-strata.sh` (45 checks) |

### 5.3 Los cinco requisitos más caros de incumplir

| # | Requisito | Por qué |
|---|---|---|
| 1 | `R-STRATA-23` (modo `serve --profile`) | sin él, **los dos modelos Groot no son servibles por ropero**. No es una mejora: es la condición de posibilidad. |
| 2 | `R-TST-18` (propagar el rc del bloque python) | un checker que da verde sin mirar **consume la confianza que debería consumir**. Peor que no tener checker. |
| 3 | `R-API-13` (no reenviar la clave al backend) | la clave en el argv de un proceso es legible por cualquier usuario del mismo host (`/proc/<pid>/cmdline`). |
| 4 | `R-EST-8` (no matar un PID reciclado) | el SO recicla PIDs; sin la verificación, `ropero stop` mata a un navegador. |
| 5 | `R-STRATA-23` (modo `serve --profile`) | sin él, los dos Groot Gyro-M no son servibles por ropero: la incompatibilidad es de diseño, no de configuración. |

---

## 6. Huecos, deuda y contradicciones

### 6.1 El hueco principal

**H-1 · La spec que el código cita no existe.** El código referencia
`spec §N` y `ADR-1` en docstrings, comentarios y nombres de función
(`ropero-code/bin/lib/cmd-gguffer.sh:3` dice «(spec §8.15)»; `bin/lib/03-engines.sh`
remite a `ADR-1`; `bin/server/strata-run.sh` cita «ADR-1» como origen de la
restricción bash 3.2). El `ropero-code/README.md:177` anuncia `docs/specs/`.
**Ese directorio no existe en el repo.** `docs/` sólo contiene ficheros
`.md` de pruebas y de Strata/FLM.

⇒ **Este documento es la reconstrucción de esa spec**, hecha desde el
código, no desde la spec ausente. Es la diferencia entre *documentar lo que
hay* y *especificar lo que hay*, y por eso cada requisito lleva evidencia
de línea.

**H-2 · El runner de la batería que la documentación anuncia no existe.**
`ropero-code/README.md:194` y `ropero-code/CHANGELOG.md:205` dan `tests/suite/run.sh`; el path real
es `test/` (movido en `4b133bf`) y el propio `ropero-code/README.md:176` lo dice bien.
No hay `.github/`, ni `Makefile`, ni `justfile`: **no existe un comando
documentado que ejecute la batería completa**.

### 6.2 Contradicciones entre documentación y código

| # | Contradicción | Veredicto |
|---|---|---|
| H-3 | `ropero-code/CHANGELOG.md:23-25` y `ropero-code/bin/lib/engines/fastflowlm.sh:6-8` dicen que FLM va **detrás** de `endpoint.py`; el launch real es `flm serve` **directo** y el test lo **exige** (`ropero-code/test/test-endpoint-auth.sh:109-122`) | **el test manda**. Consecuencia: el puerto de FLM queda abierto a local sin key |
| H-4 | `ropero-code/CHANGELOG.md:179-183` dice que `llama-server-official` está **oculto**; el código lo **visibiliza** y `install engines` lo ofrece en igualdad | **el código manda** |
| H-5 | `ropero-code/CHANGELOG.md:98-102` afirma que el engine FLM «se niega a arrancar sin `LLAMA_API_KEY`»; el test afirma lo contrario | **el test manda** |
| H-6 | `ropero-code/README.md:163-164` promete `tests/suite/run.sh` | **no existe** |
| H-7 | `ropero-code/bin/lib/engines/llama-server.sh:12-28` y `ropero-code/CHANGELOG.md:158-178` hablan de un fork de AtomicBot; el motor real es **privado** (`Lorenzo-SF/ropero-llama-turboquant`) | **el código manda** |

### 6.3 Deuda funcional medida

| # | Deuda | Tamaño | Consecuencia |
|---|---|---|---|
| H-8 | **`R-ROUTE-3`**: el fallback de `router.sh` es `minimax/m3.1-flash`, que no corresponde a ningún modelo del catálogo | 1 línea | el selector puede devolver un modelo que ropero no puede servir |
| H-9 | **`R-ROUTE-4`**: `router.sh` no tiene consumidor, y su cabecera promete consultar un `MODELS.md` **inexistente** en este repo | ~40 líneas | un script muerto que además cita un fichero que no está |
| H-10 | **`R-ROUTE-5`**: el cuerpo de la petición al clasificador se construye por interpolación **sin escapar** | 1 línea | una descripción con comillas rompe el JSON y cae al fallback sin mensaje |
| H-11 | **`R-STRATA-28`**: la opción `vram-reserve` está declarada en `[options].list` y su bloque **está comentado** en los dos TOML de Strata | 5 líneas | el valor medido (1344 MiB) que evita el cuelgue del driver NVIDIA no se puede pasar |
| H-12 | **`R-OC-11`**: el `provider` por defecto del plugin es `"marichocho"`, un provider que **no existe** en la config actual | 1 línea | el plugin no puede enrutar nada tal como está |
| H-13 | **`R-API`**: el endpoint **no tiene CORS** ni **rate limiting**, y la clave vive en el **argv** del proceso (legible por otros usuarios del mismo host vía `/proc/<pid>/cmdline`) | — | exposición real si el host es compartido |
| H-14 | **`R-GGUF`/`R-TST-19`**: el test del gguffer depende del `transformers` del host | 1 stub | 15 de 38 checks fallan en un entorno limpio |
| H-15 | **`R-TST-22`**: la resolución de `[levels]` **no tiene un solo test** | — | la mitad de la superficie de configuración del catálogo sin red de seguridad |
| H-16 | **`R-TST-23`**: 21 ficheros de código sin ningún test (6 de los 8 motores incluidos) | 21 ficheros | el doctor, el install, el catálogo y 6 motores sin garantía |
| H-17 | **`docs/strata.md`** cita modelos que **ya no existen** en el catálogo, y `CHANGELOG.md` tiene una **sección de FLM duplicada** | — | documentación que miente |
| H-18 | **La puerta del repo**: el `README.md` y el CHANGELOG **no coinciden con el árbol de ficheros** | — | el primer contacto del usuario con el repo es misleading |
| H-19 | **`R-GGUF-51`**: `subir-hf.sh --subir` **no verifica** el GGUF que sube (sólo mira el marcador `.hf-uploaded`). La puerta está en el gguffer y en `todo.sh`; llamando al script directamente, **un GGUF no verificado se sube**, y el README afirma lo contrario | 1 script | una publicación con un artefacto roto |
| H-20 | **`R-GGUF-52`**: la escritura del `.fuente.json` es fail-open y **su código de retorno se ignora**: un GGUF puede quedar **sin procedencia** y la tirada salir **0** | 1 línea | un artefacto sin trazabilidad, con el exit code diciendo que todo fue bien |
| H-21 | **`R-GGUF-53`**: tres contradicciones internas — el comentario de `cuantizar-todos.sh` afirma que los safetensors se borran «tanto si todo va bien como si algo falla» (el código hace lo contrario); un fichero de parches es un **duplicado exacto** que ningún script aplica; y `model_card.py` resuelve los alias de quant **al revés** que `quant-tipos.sh` | 3 sitios | documentación y código que cuentan historias distintas |
| H-22 | **`R-CLI-14`**: `install` expone **cinco** subcomandos (`setup`, `engines`, `models`, `link`, `opencode-plugin`) y el objetivo declarado son **dos** (`engines` y `models`) | 3 subcomandos | el usuario tiene que saber más de lo que debería |
| H-23 | **`R-DATA-21`**: el validador **no comprueba** que `[model].name` coincida con el nombre del fichero, que es la regla que hace único al modelo | 1 check | dos TOMLs con el mismo `name` se colarían en el catálogo |
| H-24 | **`R-CLI-17`**: `ropero stats` **no existe** en el repo. Lo más cercano es `ropero doctor` (estado de engines y puertos) y `bin/diagnostico.sh` (recolector de un fichero). No hay métricas de CPU/GPU en vivo ni `--watch` | comando entero | falta el indicador que se pidió |
| H-25 | **`R-API-24`**: la cabecera de `fastflowlm.sh` y el `CHANGELOG.md` dicen que FLM va por el proxy; el launch y el test dicen que va directo | 2 sitios | la documentación del engine afirma lo contrario de lo que hace |
| H-26 | **`R-EST-15`**: ropero **no ve** los motores que el usuario lanzó a mano. En Linux, `nvidia-smi --query-compute-apps` los daría; en macOS no está resuelto | 1 capacidad | «ropero no arranca» y «la GPU está ocupada por otro motor» son el mismo síntoma hasta que alguien mira |
| H-27 | **`R-GGUF-56`**: el gguffer **no comprueba que el motor esté** antes de descargar los pesos. Sin él, la conversión falla y el clasificador la etiqueta como «descarga incompleta» (H-28), así que el diagnóstico **miente** y empuja a borrar y rebajar 60 GB | 1 guard | un fallo de entorno se presenta como un fallo de descarga |
| H-28 | **`R-GGUF-57`**: con **sólo** `llama-server-official` instalado, el clon está en `~/llama.cpp-official` y el gguffer mira `~/llama.cpp`: **no lo encuentra** salvo `LLAMA_DIR` exportada a mano. Y ninguna variable de entorno de las que gobiernan el gguffer está en `--help` | 1 variable | la mitad de los motores instalados no habilita la fabricación |
| H-29 | **`R-GGUF-58`**: `llama-quantize` y `llama-gguf-split` **no se enlazan** en `$BIN_DIR` al instalar un engine, así que la alternativa por el PATH que el gguffer contempla no la activa nadie | 2 binarios | el gguffer sólo funciona con el clon en `$LLAMA_DIR` |
| H-30 | **`R-MOD-29`**: un `--prio` que el modelo **no tiene** (o que ya es su defecto) lo **ignora** en la v2; este catálogo lo documentaba como **error de uso (exit 2)** (decisión del dueño, 2026-11-14) | 1 regla | el código que herede la conducta de la v1 aborta una instalación que debe seguir: los demás modelos se instalan igual |

### 6.4 Lo que este documento **no** cubre

- **La calidad de los modelos**: la batería de evaluación produce un
  informe de juez, no un número reproducible. Este documento especifica el
  mecanismo, no el resultado.
- **El rendimiento medido**: `bench-motor.sh` es un banco, no un test, y no
  hay cifras de throughput en el repo. Ningún requisito de este documento
  afirma una cifra de velocidad.
- **La seguridad del host**: ropero protege el acceso al modelo, no el host.
  Un endpoint en `0.0.0.0` sin CORS ni rate limiting es una decisión de
  exposición, no un fallo de ropero — pero es una decisión que alguien tiene
  que tomar.
- **El soporte de Windows**: el repo es bash y Unix; no hay pretensión.

---

## 7. Glosario

| Término | Significado en ropero |
|---|---|
| **engine** | Un motor de inferencia. En el código es **un fichero** (`bin/lib/engines/<id>.sh`) con 8 hooks. No es una clase, no es un plugin. |
| **modelo** | Una entrada del catálogo: un TOML. Un modelo **es un dato**; el motor es el que lo sirve. |
| **TOML** | El formato del catálogo. Describe **qué** servir, **con qué** flags y **en qué** nivel; nunca **cómo** lanzarlo (el argv lo monta el motor). |
| **level / nivel** | Un perfil de configuración del mismo modelo (`think`, `instruct`…). Cada nivel produce **args distintos** ⇒ el server se reinicia. |
| **alias** | Un nombre alternativo para un modelo o para un nivel. Se materializa como una fila extra en las arrays del catálogo. |
| **fired** | Un modelo **retirado**: sigue en el repo como registro, no se instala ni se arranca. |
| **stamp** | El `cksum` de los args de arranque. Detecta que el TOML cambió mientras el modelo corría. |
| **estado** | Los ficheros `$STATE_DIR/ports/<puerto>` con líneas `MODEL= ENGINE= PID= PORT= LEVEL= TS= LOG= STAMP=`. Es la fuente de verdad. |
| **preflight** | La batería de comprobaciones **antes** de tocar el puerto: motor instalado, argv válido, artefacto en disco, chat template presente. |
| **readiness** | La espera a que el modelo **responda**, no a que el puerto escuche. |
| **endpoint** | `bin/server/endpoint.py`: el proxy OpenAI-compatible con auth propia. La puerta única. |
| **proxy** | Lo que hace el endpoint hacia el backend: reescribe el `model`, quita `authorization`, traduce `/v1/completions`. |
| **backend** | El proceso de detrás del proxy (`llama-server`, `flm serve`, `strata launch`…). El único proceso expuesto a la red es el proxy. |
| **pack** | (Strata) El índice de posiciones absolutas de los tensores **de un GGUF concreto**. No se puede reutilizar entre GGUF. |
| **expert-profile** | (Strata) El ranking de expertos que decide qué se queda en la VRAM. Es **el techo** del cache de expertos. |
| **run config** | (Strata) El JSON con los args de arranque de un modelo. Lo escribe `setup.py`; lo modifica el script para cada GGUF. |
| **tune** | (Strata) La medición de la máquina que produce un preset que Strata aplica en cada arranque. |
| **gguffer** | La fabricación de GGUFs: descarga → conversión → imatrix → cuantización → verificación → model card. |
| **imatrix** | La *importance matrix*: dice qué tensores cuantizar con más bits. Sin ella, los quants very-low-bit abortan. |
| **MTP** | La cabeza de predicción de siguiente token, un bloque **extra** (el 65) que el imatrix del modelo base **no cubre**. |
| **fuente.json** | La procedencia de un GGUF: de dónde salió, con qué quant, con qué imatrix, y si la MTP quedó aparte. |
| **symlink-only** | La política del instalador: el HOME sólo recibe symlinks; el repo es la fuente de verdad y nunca se modifica. |
| **herramienta doblada** | Un doble de test (`test/fakes/`): un binario falso que sustituye a uno real para poder verificar un contrato sin GPU, sin red y sin tocar el estado del usuario. |
| **nivel** (o modo) | Un perfil de configuración con nombre de un modelo (`think`, `instruct`, `low`…). Vive en `[levels]` y se aplica con `--<nivel>`. Si hay nivel por defecto, `ropero <modelo>` ya lo lleva. |
| **flag** | Un interruptor del modelo (`--flash`) que **añade** argumentos al arranque. Vive en `[flags]`. |
| **opción** | Un `clave=valor` del modelo (`--temp 0.7`) con **valor por defecto**. Vive en `[options]`. |
| **build** | La vía de fabricación: bajar los pesos, convertirlos a GGUF sin quantizar y quantizarlos. La otra vía es **download** (bajar el GGUF o los safetensors tal cual). |
| **gguffer** | El comando que fabrica un GGUF a mano: es el equivalente manual de `ropero install models --prio build`. |
| **proxy** | `bin/server/endpoint.py`: el servidor OpenAI-compatible con autenticación propia que sirve de puerta a los engines que no traen server. |
| **preflight** | Las comprobaciones que se hacen **antes** de tocar el puerto: motor instalado, argv válido, artefacto en disco, chat template presente. |

---

## 8. Procedencia de este documento

### 8.1 Qué se leyó

| Fuente | Volumen | Cómo se leyó |
|---|---|---|
| `bin/ropero` + `bin/lib/*` | 11.022 líneas de bash | lectura directa de los 6 módulos base y los 7 `cmd-*` |
| `bin/python/*` | 4.188 líneas de Python | lectura directa |
| `bin/lib/engines/*.sh` | 8 ficheros | lectura directa de los 8 |
| `bin/server/*` | endpoint + puente | lectura directa + subagente |
| `install.sh`, `models/`, `opencode-plugin/`, `test/`, `docs/` | todo | subagentes de extracción + lectura directa |
| `groot-strata.sh` (adjunto del encargo) | 486 líneas | lectura íntegra, línea por línea |

**Subagentes de extracción** (sólo lectura, sin modificar el repo): cuatro
tareas paralelas —endpoint y router, gguffer y quant, batería de tests,
instalación y modelos. Todos con `git status --porcelain` limpio al
terminar.

### 8.2 Qué se midió (no se suplementó)

- **317 requisitos** con identificador único y evidencia `fichero:línea`
  (17 de ellos = estado objetivo declarado por el dueño, no implementados).
- **La batería de tests**, ejecutada en un `HOME` temporal con los dobles en
  el PATH: **5/5 baterías ok en ~2 s, exit 0**, sin GPU, sin red y sin matar
  nada.
- **El falso positivo de la auth**, reproducido: se rompió `authed()` en una
  copia para que aceptara cualquier clave → el bloque python reportó 5
  fallos y el script **salió con exit 0**.
- **La no-hermeticidad del gguffer**: 15 de 38 checks fallan en un sandbox sin
  `transformers`.
- **La aritmética de `bin/router.sh`**: el fallback no corresponde a ningún
  modelo del catálogo.
- **El catálogo**: 8 motores, 17 modelos activos, 28 retirados, 16 scripts de
  test.

### 8.3 Lo que **no** se verificó

- **Nada se ejecutó del runtime real**: no se arrancó ningún motor, no se
  descargó ningún modelo, no se tocó la GPU. Las cifras de throughput que
  aparecen en la documentación de Strata (228 t/s a 4K, 273 en carga real) son
  **citas de la documentación de Strata**, no mediciones de esta auditoría.
- **El estado de los motores instalados** en la máquina del dueño: no se
  ejecutó `ropero doctor`.
- **La exactitud de los 28 modelos retirados**: se verificó el mecanismo de
  descubrimiento, no el contenido de cada `fired/`.

### 8.4 Qué no viene del código

Los **17 requisitos marcados 🔶** salen de la declaración del dueño
(2026-11-12), no de una línea de código. Son el contrato que el proyecto
**quiere** tener; el código de hoy los incumple, y cada incumplimiento está
anotado en § 6 con su identificador de deuda. Refutar esos requisitos
requiere **discutir la intención**, no el código.

### 8.5 Cómo se puede refutar este documento

Cada requisito es una afirmación localizable. Para refutarla: abrir el
fichero, ir a la línea, y mirar. Si la línea no dice lo que este documento
dice, el requisito es falso y hay que corregirlo. Ése es el criterio que el
propio repo usa (P-1: la evidencia manda sobre la suposición), y el mismo
que este documento se aplica a sí mismo.
