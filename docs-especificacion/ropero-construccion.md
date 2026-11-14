---
title: "ropero — Especificación de construcción"
subtitle: "Cómo se construye, desde cero y de manera autónoma, el gestor de modelos locales que la especificación de requerimientos describe"
author: "Lorenzo-SF"
date: "2026-11-12"
---

# ropero — Especificación de construcción

> Documento **de construcción**, no de requerimientos. Los requerimientos —qué debe
> hacer el sistema, con qué semántica exacta y con qué evidencia— están en
> `ropero-especificacion.md` (317 requisitos, R-ARQ…R-GGUF). Este documento
> responde a la pregunta distinta: **con qué se construye, cómo se organiza, en
> qué orden, y cómo se sabe que está terminado.**
>
> Leídos juntos, los dos documentos bastan para que una IA construya ropero
> entera sin preguntas: uno dice *qué*, el otro dice *cómo*.

---

## 0. Cómo usar este documento

### 0.1 Los dos documentos y el contrato entre ellos

| | `ropero-especificacion.md` | `ropero-construccion.md` (este) |
|---|---|---|
| Pregunta que responde | ¿Qué hace ropero y con qué semántica? | ¿Cómo se construye ropero? |
| Naturaleza | Contrato de comportamiento | Plan de ejecución |
| Unidad | Requisito (R-XXX-N) | Módulo, tarea, fase, criterio de aceptación |
| Authorities | **NORMATIVA** sobre el comportamiento | **NORMATIVA** sobre la construcción |
| Estado | describe lo que el código hace hoy, con evidencia `fichero:línea` | describe lo que hay que escribir, con versiones y firmas |

**Regla de conflicto.** Si este documento y el de requerimientos discrepan sobre
*qué debe hacer el sistema*, **manda el de requerimientos**. Si discrepan sobre
*cómo construirlo*, **manda este**. Ningún cambio de comportamiento puede
introducirse desde aquí: si una tarea de este documento implica cambiar el
comportamiento descrito en el otro, la tarea está mal y hay que corregirla, no
implementarla.

**Regla de completitud.** Una IA que sólo disponga de este documento no puede
construir ropero: le faltan los 317 requisitos, el vocabulario exacto
(`nivel`/`flag`/`opción`, `[args].base → level → flags → options`), los ocho
hooks del contrato de motor, el orden de montaje del argv y las 29 deudas conocidas. Y una IA que sólo disponga del de requerimientos no puede
construirlo tampoco: no sabe con qué lenguaje, con qué versiones, en qué
módulos, con qué tests, en qué orden. **Los dos son necesarios y ninguno basta.**

### 0.2 Convenciones

- **Idioma del código**: inglés (identificadores, tipos, mensajes de `error!`).
  **Idioma de la documentación y de los mensajes de cara al usuario**: español.
  Es la convención ya establecida en el repo (`bin/ropero`, `install.sh`).
- **Los mensajes de error al usuario** se redactan en español, sin jerga, y
  siempre incluyen **la acción que el usuario debe hacer** (P-2: un fallo
  silencioso es peor que un error visible).
- **Cero red en el camino de ejecución.** Los comandos de ropero no llaman a la
  red salvo en la descarga de modelos y en la instalación de motores, y en
  esos dos casos la operación es explícita yobservable. `install.sh` tiene R6
  como regla dura y se conserva.
- **Cero `sleep` para sincronizar.** La espera es un evento con timeout
  (poll con backoff y deadline), nunca una pausa ciega.

### 0.3 Supuestos declarados

Este documento se escribió sin poder preguntar al dueño en el momento de
redactarlo. Los supuestos que sí se tomaron son **explícitos y reversibles**, y
cada uno lleva su coste de revertir:

| # | Supuesto | Por qué | Coste de revertir |
|---|---|---|---|
| S-1 | El objetivo es **reimplementar ropero en Rust**, conservando el comportamiento observable del doc A. | La pregunta previa fue «si quisiera crear esta aplicación en plan serio, con qué lenguaje»; la respuesta fue Rust con extracción por fases. | Alto: un cambio a Go/Python reescribe § 2, § 3 y § 9. |
| S-2 | La **interfaz de línea de comandos no cambia**: los mismos comandos, las mismas opciones, los mismos códigos de salida, el mismo `stdout` contractual. | El doc A define un contrato de stdout (`ROPERO_PORT=…`) que consume el plugin de opencode. Romperlo rompe el ecosistema. | Alto. |
| S-3 | Los **TOML de modelo no cambian de esquema**: se le añade validación, no se le cambia la forma. | Hay 45 TOML en el repo y un `models/_template.toml` que es la documentación ejecutable del formato. | Medio: una migración de esquema exige tocar 45 ficheros. |
| S-4 | Se **conserva la tesis symlink-only** de la instalación: el instalador no copia ficheros al home, enlaza. | Es una decisión del dueño ya escrita (R-INST) y con reglas duras R1–R13. | Medio:un binario único contradice R1 y obliga a reescribir `install.sh`. |
| S-5 | El **endpoint se reescribe en Rust** (`ropero serve`), no se mantiene `endpoint.py`. | Un solo binario, un solo lenguaje, y el test de auth que hoy da verde falso se rehace con un mecanismo que no puede dar verde falso. | Medio: mantener Python obliga a un runtime de Python en la máquina del usuario. |
| S-6 | Los **motores externos no se reimplementan**: llama.cpp, MLX, TensorRT-LLM, vLLM, AirLLM, FastFlowLM y Strata siguen siendo procesos externos. | Son software de terceros, de cientos de miles de líneas. ropero los *conduce*. | Nulo (es la decisión correcta). |
| S-7 | La **precondición del gguffer** (que hoy no se comprueba) se convierte en un requisito explícito y en un guard que falla antes de descargar 60 GB. | Es la herida medida más caro de todas (H-27). | Nulo. |

**Lo que NO se ha supuesto**: nada sobre el modelo de negocio, el precio, la
distribución fuera del propio repo, ni el soporte de más de dos sistemas. Si
alguien necesita esas cosas, este documento no las cubre y hay que decirlo.

### 0.4 Convenciones de referencia

| Símbolo | Significado |
|---|---|
| `R-XXX-N` | Requisito del doc A. **Cada tarea de § 9 cita los que cierra.** |
| `M-N` | Módulo del sistema. Definido en § 3. |
| `T-NN` | Tarea de construcción. Definida en § 8. |
| `F-N` | Fase del plan. Definida en § 8. |
| `P-N` | Principio de diseño. Los doce del doc A, más los nuevos de § 5.2. |
| `H-N` | Deuda conocida del doc A. Se cierra o se documenta; no se hereda en silencio. |
| 🔶 | Requisito del doc A en estado objetivo (no implementado hoy). |

---

## 1. Qué se construye

### 1.1 La tesis, en una frase

**ropero es un conductor de motores de inferencia local**: lee modelos declarados
en TOML, comprueba que la máquina puede servirlos, arranca el motor adecuado
en un puerto que le pertenece, espera a que esté *de verdad* listo, y expone una
única puerta compatible con la API de OpenAI. Todo lo demás —descargar pesos,
fabricar cuantizaciones, decidir qué motor usar— es Instrumentación de ese
gesto.

### 1.2 Los tres hechos que todo el diseño respeta

Estos no son preferencias; son observaciones sobre el dominio que, si se
ignoran, producen un sistema que parece funcionar y no funciona.

**Hecho 1 — El arranque de un motor de inferencia es lento y opaco.** No hay
forma fiable de saber si un `llama-server` está listo salvo preguntarle. Por
eso la *readiness* es una jerarquía de niveles, no un booleano, y por eso
«esperar un segundo y reintentar» es un defecto de diseño, no una simplificación.

**Hecho 2 — Un modelo sin su fichero no es un modelo.** La mitad de los fallos
que un usuario ve en un gestor de modelos son, en realidad, un fichero que no
está donde el sistema cree que está, o un `.gguf` que se descargó a medias, o un
checkpoint que se quantizó a una resolución distinta de la que se pidió. Por eso
el modelo es **dato** (P-6) y por eso la *procedencia* del artefacto no es
opcional (R-GGUF-18).

**Hecho 3 — La máquina del usuario no es un entorno limpio.** Puede tener otro
programa en el puerto 9999, un motor lanzado a mano, una GPU ocupada por otro
proceso, o un clon de llama.cpp en un directorio distinto del que el sistema
espera. Un gestor de modelos que no distingue «mi motor» de «un motor» es un
gestor de modelos que mata procesos ajenos. De aquí P-3 y la identidad
`pid + start_time`.

### 1.3 Lo que este documento NO es

- **No es un tutorial de Rust.** No explica qué es un `enum` ni cómo funciona un
  `async fn`. Asume que quien lo ejecuta sabe Rust.
- **No es la especificación de requerimientos.** No repite los 317 requisitos:
  los cita. Repetir 140 KB de semántica en dos sitios es la forma más rápida de
  que los dos sitios digan cosas distintas.
- **No es un plan de negocio.** No decide precio, empaquetado comercial, ni
  canales de distribución más allá del `install.sh` que ya existe.

---

## 2. Stack y versiones

### 2.1 Principio

**El stack se fija con versiones exactas, se registra, y se actualiza por
explicación.** Una dependencia sin versión fijada es una deuda con intereses
compuestos: el día que el crate `X` cambia su API, el build falla en la máquina
del usuario, no en el CI.

Consecuencias operativas:

- `Cargo.lock` **se commitea** (es una aplicación, no una librería).
- `rust-toolchain.toml` fija la versión del compilador.
- Toda dependencia nueva pasa por § 6.8 (ADR) antes de entrar.
- Las actualizaciones se hacen en commits separados, para que un fallo de
  build sea atribuible a la actualización y no a un cambio de lógica.

### 2.2 Toolchain

| Elemento | Versión | Fijado en | Nota |
|---|---|---|---|
| **Rust (stable)** | **1.100.0** (2026-11-09) | `rust-toolchain.toml` | La MSRV declarada. Verificado contra `static.rust-lang.org/dist/channel-rust-stable.toml` el 2026-11-12. |
| Edition | **2024** | `Cargo.toml` | `edition = "2024"`, `rust-version = "1.100"` en cada crate. |
| Perfil `release` | `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"` en el binario CLI | `Cargo.toml` | `panic = "abort"` **sólo** en el binario: el servidor necesita unwind para no matar el proceso entero. |
| Target principal | `x86_64-unknown-linux-gnu` (CachyOS) | CI | |
| Targets de publicación | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin` | `.github/workflows/release.yml` | macOS arm64 es obligatorio: el hardware de referencia del dueño es un Mac con Metal. |
| Formato de binario | estático con `musl` para Linux | CI | El target gnu depende de glibc; musl no. El sistema del usuario ya tiene las libs, pero un binario estático elimina una clase entera de «no arranca en tu máquina». |
| Shell de los scripts de soporte | `bash 3.2` (el de macOS) | `shellcheck` + CI | **Restricción dura heredada**: sin `readlink -f`, sin `mapfile`, sin `declare -A`, sin nameref, sin `${var^^}`. R10 del instalador. |

### 2.3 Dependencias de producción

Versiones verificadas contra el índice de crates.io el **2026-11-12**. El
documento fija la *versión mínima aceptada*; `Cargo.lock` fija la exacta que se
compila.

**Recuento: 33 dependencias de producción** (en § 2.3.1 a § 2.3.5) y **9
herramientas de desarrollo** (§ 2.3.6, que no van al binario). Las 7
dependencias prohibidas de § 2.3.7 no se instalan.

#### 2.3.1 Núcleo de dominio

| Crate | Versión | Para qué | Por qué esta y no otra |
|---|---|---|---|
| `serde` | 1.0.229 | Derivar `Serialize`/`Deserialize` de los tipos del dominio | Es el estándar de facto; `toml` y `axum` ya dependen de él, así que no añade peso al árbol. |
| `toml` | 1.2.1 | Parsear los TOML de modelo y de manifiesto de motor | Mantiene el orden de las claves y da mensajes de error con línea y columna. |
| `thiserror` | 2.0.21 | Derivar `Error` para las variantes de dominio | Errores de dominio tipados y exhaustivos. |
| `anyhow` | 1.0.104 | Contexto en los límites (CLI, binario) | Sólo en el borde: el dominio devuelve tipos concretos. |
| `schemars` | 1.2.3 | Generar JSON Schema del TOML de modelo | Publicable: `ropero schema --json` alimenta editores y el validador externo. |
| `camino` | 1.2.6 | Rutas UTF-8 en vez de `PathBuf` | Los nombres de modelo y de fichero pueden no ser ASCII; `Path` en Unix no lo garantiza y rompe. |
| `directories` | 6.0.0 | Resolver `~/.local/bin`, `~/.config`, caché | Respeta XDG en Linux y las rutas correctas en macOS. |

#### 2.3.2 CLI

| Crate | Versión | Para qué |
|---|---|---|
| `clap` | 4.6.7 | Parser de argumentos, con `derive` y subcomandos |
| `clap_complete` | 4.6.11 | Generar los completions de shell (`ropero completion bash`) |
| `shell-words` | 1.1.1 | Trocear una línea de comando en argv sin invocar a la shell |
| `is-terminal` (vía `std::io::IsTerminal`) | std | Detectar TTY: sin TTY, el menú interactivo no se ofrece (R-CLI-16) |

#### 2.3.3 Runtime: procesos, puertos, sistema

| Crate | Versión | Para qué |
|---|---|---|
| `tokio` | 1.54.1 | Runtime asíncrono: timeouts, cancelación, `select!` |
| `nix` | 0.31.3 | `fork`/`setsid`/señales/`waitpid`/`prctl` — identidad de proceso y muerte de grupo |
| `libc` | 0.2.191 | Llamadas al sistema que `nix` no expone |
| `sysinfo` | 0.40.3 | Métricas de CPU/RAM/GPU del proceso y del sistema (`ropero stats`) |
| `fs4` | 1.1.0 | Bloqueo de fichero entre procesos (el lock de puerto) con expiración |
| `signal-hook` | 0.4.6 | `SIGINT`/`SIGTERM` en handlers seguros (async-signal-safe) |
| `which` | 8.0.6 | Localizar binarios en el `PATH` |
| `rustix` | 1.1.6 | `mount`/`statvfs`/espacio en disco: «¿cabe el GGUF?» antes de descargar |
| `notify` | 8.2.2 | Observar cambios en el directorio de modelos (opcional, para `ropero watch`) |

#### 2.3.4 Red y servidor

| Crate | Versión | Para qué |
|---|---|---|
| `axum` | 0.8.9 | Servidor HTTP del endpoint OpenAI-compatible |
| `hyper` | 1.12.0 | Runtime HTTP (vía axum) |
| `hyper-util` | 0.1.21 | Utilidades de `hyper` (server, upgrades) |
| `http-body-util` | 0.1.5 | Cuerpos HTTP como `Body` para streaming |
| `tower` | 0.5.3 | Middleware (auth, CORS, limitación de tasa, trazas) |
| `tower-http` | 0.7.1 | CORS, límite de cuerpo, compression |
| `reqwest` | 0.13.5 | Cliente HTTP: descargas con reintento y reanudación, proxy al motor |
| `futures-util` | 0.3.34 | `StreamExt` para streaming SSE bidireccional |
| `bytes` | 1.12.1 | Buffers efficiently |
| `parking_lot` | 0.13.0 | Mutexes sin poisoning (para el registro de puertos) |

#### 2.3.5 Trazas y errores

| Crate | Versión | Para qué |
|---|---|---|
| `tracing` | 0.1.44 | Instrumentación estructurada |
| `tracing-subscriber` | 0.3.23 | Envolvente, formato json/pretty, filtro por `ROPERO_LOG` |
| `tracing-error` | 0.2.1 | Convertir errores de `anyhow` en spans con su causa |
| `miette` | 7.6.0 | Presentación de errores con diagnóstico (código, fuente, ayuda) |

> **Nota sobre `miette` vs `thiserror`**: `thiserror` define los tipos de error del
> dominio; `miette` los presenta. Un error de dominio no depende de `miette`
> (se puede comprobar en un test sin comparar texto), y el formato del CLI no
> se puede comprobar en un test unitario. La separación evita el test que
> comprueba el *string*.

#### 2.3.6 Herramientas de desarrollo (no van al binario)

| Herramienta | Versión | Uso |
|---|---|---|
| `cargo-nextest` | 0.9.151 | Runner de tests: paralelismo por proceso, `--retries`, Profiles |
| `cargo-llvm-cov` | 0.9.1 | Cobertura LLVM, con informe lcov |
| `cargo-deny` | 0.20.2 | Licencias, advisories, bans, sources duplicados |
| `cargo-binstall` | 1.27.0 | Instalar binarios precompilados (ahorra minutos de CI) |
| `cargo-edit` | 0.13.14 | `cargo add/rm/upgrade` con edición controlada del `Cargo.toml` |
| `cargo-sort` | 2.1.4 | Ordenar las dependencias del `Cargo.toml` (evita ruido en el diff) |
| `taplo` | 0.14.0 | Formatear los TOML de modelo y los manifiestos de motor |
| `shellcheck` | 0.10.0 | Los scripts de soporte que siguen en bash |
| `rustfmt` | con el toolchain | `cargo fmt --check` |

#### 2.3.7 Dependencias **prohibidas** (con motivo)

| Crate | Motivo |
|---|---|
| `openssl` / `native-tls` | El cliente HTTP debe funcionar sin el stack TLS del sistema; `reqwest` con `rustls` lo resuelve. |
| `chrono` | `std::time::SystemTime` cubre el caso de uso; las marcas de tiempo van en epoch y en RFC-3330 formateado a mano. Una dependencia de fecha con zona horaria es un bug esperando. |
| `lazy_static` / `once_cell` | `std::sync::OnceLock` (estable desde 1.70) cubre el caso. |
| `dotenv` | El entorno lo hereda el proceso; no hay fichero `.env` que leer. |
| `indicatif` | Las barras de progreso de `hf` son las de `hf`; una segunda barra duplica el ruido. |
| `reqwest` con `default-features` de más (trae Tokio y OpenSSL) | Activar `default-features = false` y listar sólo lo que se usa: `rustls-tls`, `json`, `stream`. |

### 2.4 Estructura del workspace

Un solo binario distribuible, cuatro librerías internas con dependencias
estrictamente descendentes. **El grafo no puede tener ciclos**, y el test de
arquitectura (§ 8, T-01) lo comprueba.

```
ropero/
├── Cargo.toml                 # workspace: miembros, perfil release, lints
├── Cargo.lock                 # versionado (aplicación, no librería)
├── rust-toolchain.toml        # canal + componentes (rustfmt, clippy)
├── rustfmt.toml               #edition = "2024"
├── clippy.toml                # lints adicionales
├── deny.toml                  # cargo-deny: licencias, bans, advisories
├── README.md
├── CHANGELOG.md
├── crates/
│   ├── ropero-core/           # M-01 · dominio puro. SIN I/O de proceso ni de red.
│   ├── ropero-runtime/        # M-02 · procesos, puertos, estado en disco.
│   ├── ropero-download/       # M-03 · descargas, verificación, reanudación.
│   ├── ropero-forge/          # M-04 · fabricación de GGUFs (gguffer).
│   ├── ropero-server/         # M-05 · endpoint OpenAI-compatible.
│   └── ropero-cli/            # M-06 · binario `ropero`. Sólo Cableado y presentación.
├── models/                    # TOML de modelo (datos, no código)
├── engines/                   # Manifiestos de motor (datos, no código)
├── test/                      # Batería de integración (harnesses)
├── scripts/                   # Lo que siga siendo bash (justificado en § 6.9)
├── opencode-plugin/           # Plugin de opencode (TypeScript, fuera del workspace Rust)
└── docs/
```

**`ropero-code/` (la v1) no aparece en ese árbol a propósito**: es **referencia,
no dependencia** ([ADR-003](../docs/adr/ADR-003-ropero-code-es-referencia.md)).
Todo el desarrollo de la v2 se hace **como si esa carpeta no existiera**; se
consulta para aprender, contrastar y comparar, nunca como fuente de verdad. El
catálogo de la v2 es **nuevo e independiente** y vive en `models/`: **no se
integra en el ejecutable**.

**Regla de dependencia** (verificada por test, § 9 T-01):

```
ropero-cli ──► ropero-core ──► (nada)
     │             ▲
     ├──► ropero-runtime ──┘
     ├──► ropero-download ─┘
     ├──► ropero-forge ────┘
     └──► ropero-server ───┘
```

- `ropero-core` **no depende de nada** del proyecto. Ni de `tokio`, ni de
  `reqwest`, ni de `nix`. Es puro: tipos, reglas, transiciones de estado. Esto
  es lo que hace que el 80 % de la lógica sea testeable en milisegundos y sin
  procesos reales.
- `ropero-runtime`, `-download`, `-forge`, `-server` dependen **sólo** de
  `ropero-core` entre los del proyecto.
- `ropero-cli` depende de todos, y es el único que produce salida por pantalla.

### 2.5 Qué NO usa el proyecto (y por qué)

| No usa | Motivo |
|---|---|
| `async-trait` | Con Rust 2024 las traits asíncronas son estables sin macro. Una dependencia menos. |
| `serde_yaml` | El formato de datos es TOML. YAML añadiría ambigüedad de tipos y una dependencia. |
| `clap` derive vs builder | Derive. El builder se reserva para el caso en que la estructura de un subcomando no sea derivable; si aparece, es una señal de que el subcomando tiene demasiadas variantes. |
| Un ORM o una base de datos | El estado es un directorio de ficheros. Una base de datos sería más infraestructura que datos. |
| Un framework de CLI tipo `ratatui` | ropero es una CLI de tubes, no una TUI. La salida va a un log o a un pipe. |
| `openssl` | Ya cubierto en § 2.3.7. |

---
## 3. Arquitectura

### 3.1 Las cuatro capas y por qué están separadas

```
┌────────────────────────────────────────────────────────────────┐
│  M-06 ropero-cli          Presentación: parseo, salida, códigos │
│  (binario `ropero`)       de salida, TTY o no, localización      │
└───────────────┬────────────────────────────────────────────────┘
                │ usa
┌───────────────▼────────────────────────────────────────────────┐
│  M-05 ropero-server      HTTP: proxy, auth, streaming SSE      │
│  M-04 ropero-forge       Fabricación: fases, checkpoints,       │
│  M-03 ropero-download    Descarga: reintentos, verificación    │
│  M-02 ropero-runtime     Procesos: spawn, sonda, parada, estado │
└───────────────┬────────────────────────────────────────────────┘
                │ depende de
┌───────────────▼────────────────────────────────────────────────┐
│  M-01 ropero-core         Dominio puro: tipos, reglas,         │
│  (sin I/O)                transiciones de estado, errores      │
└────────────────────────────────────────────────────────────────┘
```

**La regla que define la frontera**: `ropero-core` no hace I/O. *«No hay
`std::process::Command` en `ropero-core`»* es una regla que se puede
comprobar con `grep` en el CI, y por tanto se comprobará (§ 8, T-01).

Qué gana cada frontera:

- **Dominio separado de E/S** ⇒ la lógica se prueba sin procesos, sin red y
  sin disco. Un test del dominio tarda microsegundos; el mismo test integrado
  tarda segundos. La batería completa tiene que poder correr en segundos, o no
  secorre.
- **Un motor por crate** ⇒ el trabajo del gguffer (que hoy son 2.338 líneas de
  bash en `cmd-gguffer.sh`) no compite por el lock con el arranque de un modelo.
- **CLI por encima de todo** ⇒ el servidor, el fabricador y el descargador se
  pueden usar como librerías desde otro programa sin la CLI.

### 3.2 M-01 · `ropero-core` — el dominio

Contiene **todo** lo que se puede decidir sin tocar el mundo exterior.

#### 3.2.1 Los tipos del modelo

```rust
// crates/ropero-core/src/model.rs

/// Un modelo del catálogo. Es DATO: aquí no se ejecuta nada (P-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSpec {
    pub model: ModelMeta,
    pub files: Files,
    pub download: Option<Download>,
    pub build: Option<Build>,
    pub levels: Option<Levels>,
    pub flags: Option<Flags>,
    pub options: Option<Options>,
    pub aliases: BTreeMap<String, String>,
    pub limits: Option<Limits>,
    pub args: Args,
    /// La ruta del fichero TOML de origen. Se conserva para los mensajes de
    /// error: un error de validación debe decir EN QUÉ FICHERO.
    pub source: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelMeta {
    pub name: String,          // kebab-case, único global
    pub engine: EngineId,      // id canónico (ver § 4.2)
    pub context: u32,
    pub alias: Option<String>,
    pub port: Option<u16>,     // puerto pineado
    pub format: ModelFormat,   // Gguf | Safetensors | Q4nx | Strata
    pub path: Option<String>,  // obligatorio en safetensors, prohibido en gguf
}
```

**El campo `source` es deliberado.** El validador actual (`modelconf.py`)
reporta `modelo X: campo Y`, no `modelo X (fichero Z)`: línea 3. En un catálogo
de 45 modelos, el mensaje sin ruta obliga a un `grep` manual. Con `source`, el
error dice `models/llama/groot.toml:12: [model].format = "safetensors" exige
[model].path`. Es la diferencia entre un error accionable y un error que hay
que interpretar.

#### 3.2.2 El vocabulario: nivel, flag, opción

El doc A fija este vocabulario (R-CLI) y es **parte del contrato público**:

| Concepto | Qué es | Dónde se declara | Cómo se pide en la CLI |
|---|---|---|---|
| **nivel** (`level`) | Un modo de razonamiento. Mutuamente excluyente. | `[levels].list` | `--<nivel>` |
| **flag** | Un interruptor booleano. Acumulable. | `[flags].list` | `--<flag>` |
| **opción** (`option`) | Un valor. Repetible con última wins. | `[options].list` | `--<opción> <valor>` |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgSelector {
    Level(Level),
    Flag(Flag),
    Option(OptionKey),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Level {
    Named(String),   // "high"
    Alias(String),   // "x" → resuelve a un Named
}
```

**Un alias de nivel es un nivel.** `[aliases] x = "high"` significa que `--x` es
exactamente `--high`, con el mismo argv y las mismas consecuencias. No es un
atajo distinto. Escribirlo de otro modo es un bug esperando: el día que un
alias apunte a un flag y no a un nivel, la comprobación de "nivel no declarado
→ exit 2" deja de aplicar.

#### 3.2.3 El montaje del argv

El orden es **fijo y normativo** (R-DATA-15, plantilla `models/_template.toml`):

```
[args].base  →  [args.level.<nivel>]  →  [args.flag.<f>]  →  [args.option.<o>]
```

```rust
// crates/ropero-core/src/args.rs

/// Monta el argv final de un motor. PURA: no lee disco, no consulta el
/// entorno, no lanza procesos. Todo lo que necesita entra por parámetro.
pub fn build_argv(
    spec: &ModelSpec,
    request: &LaunchRequest,
    ctx: &ResolveContext,
) -> Result<Vec<String>, ArgvError>;

pub struct LaunchRequest {
    pub level: Option<String>,
    pub flags: Vec<String>,     // en el orden pedido por el usuario
    pub options: Vec<(String, String)>,
}

pub struct ResolveContext {
    /// Contexto efectivo: [model].context, o el override ROPERO_<NAME>_CTX.
    pub context: u32,
    /// Rutas ya resueltas y existentes (gguf, mtp, mmproj, chat_template).
    pub files: ResolvedFiles,
    /// Variables de entorno ya capturadas.
    pub env: BTreeMap<String, String>,
}
```

**El orden de los flags es el pedido por el usuario**, no el del TOML. Es
observable y hay tests que lo comprueban (R-DATA-16).

**`replace` sustituye en el base.** `[args.option.temp] base = ["--temp",
"${value}"]` con `replace = "--temp"` elimina la pareja `--temp 0.7` del base y
pone `--temp <valor>` en su lugar. Si no, el argv tendría `--temp 0.7 … --temp
0.9`, y el motor gana el último — lo que funciona por casualidad, no por
diseño, y falla en cuanto el orden cambie.

**`requires` es una precondición de disco.** `[args.flag.sin-vision] requires =
["mmproj"]` ⇒ si el `mmproj` no está, `--sin-vision` es **error de uso**
(exit 2), no un warning. El usuario pidió algo que no se puede hacer; decírselo
claramente es lo único honesto.

#### 3.2.4 Placeholders

Nueve placeholders, resueltos en dos fases (sustitución textual y después
resolución de rutas). Los nombres exactos están en el doc A § 4.2.3; aquí la
regla que los gobierna:

> **Un placeholder que no resuelve es un error, nunca una cadena vacía.**
> `${gguf}` sin fichero ⇒ `ArgvError::MissingFile`. Nunca `"--model" ""`,
> porque un motor interpretaría eso como un modelo vacío y fallaría 30
> segundos después con un mensaje inútil.

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgvError {
    UnknownLevel { level: String, declared: Vec<String> },
    UnknownFlag  { flag: String, declared: Vec<String> },
    UnknownOption{ option: String, declared: Vec<String> },
    MissingFile  { placeholder: &'static str, key: String },
    MissingEnv   { var: String },
    NotDeclared  { selector: String },
}
```

Cada variante lleva **la lista de lo declarado**, para que el mensaje pueda
decir «--alto no es un nivel; los niveles son: low, medium, high».

#### 3.2.5 La máquina de estados del runtime

El estado de un modelo gestionado es un valor, no un conjunto de ficheros sueltos
(R-EST). La máquina de estados es explícita:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeState {
    /// No hay proceso. Estado inicial y final.
    Stopped,
    /// Hay proceso, pero todavía no responde.
    Starting,
    /// Responde. Hay PID, puerto y sello.
    Ready,
    /// El proceso murió durante el arranque.
    Failed,
    /// Se le ha pedido parar y se está esperando.
    Stopping,
    /// El proceso está vivo pero su salud se ha perdido.
    Degraded,
}
```

Transiciones permitidas — **cualquier otra es un bug de programación** y el
test de § 8 T-02 la comprueba exhaustivamente:

```
Stopped  → Starting        (start)
Starting → Ready           (sonda OK)
Starting → Failed          (sonda KO / proceso muerto)
Starting → Stopping        (cancelación)
Ready    → Stopping        (stop)
Ready    → Degraded        (la sonda posterior falla)
Degraded → Ready           (la sonda vuelve a OK)
Degraded → Stopping        (stop)
Failed   → Starting        (reintento)
Stopping → Stopped         (proceso reaped)
Stopping → Failed          (no muere: hay que matarlo)
```

`Failed` y `Degraded` se distinguen a propósito: `Failed` es «no llegó a
arrancar» (no hay a quién preguntar) y `Degraded` es «arrancó y dejó de
funcionar» (hay que pararlo). Confundirlos produce el síntoma de «ropero dice
que el modelo está vivo y no lo está».

#### 3.2.6 Los errores

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("modelo desconocido: {name}")]
    UnknownModel { name: String, known: Vec<String> },

    #[error("{path}:{line}: {message}")]
    Config { path: PathBuf, line: usize, message: String },

    #[error("el motor {engine} no es compatible con esta máquina: {reason}")]
    IncompatibleEngine { engine: EngineId, reason: String },

    #[error("puerto {port} ocupado por un proceso ajeno (pid {pid})")]
    PortTakenByForeign { port: u16, pid: u32 },

    #[error("transición de estado inválida: {from} → {to}")]
    IllegalTransition { from: RuntimeState, to: RuntimeState },
}
```

**Regla**: un `CoreError` **no contiene texto de presentación**. El texto en
español con tips lo compone la capa CLI. Esto permite que un test compruebe
`matches!(err, CoreError::UnknownModel { .. })` en vez de comparar cadenas, y
que el mensaje cambie sin romper el test.

### 3.3 M-02 · `ropero-runtime` — procesos, puertos y estado

#### 3.3.1 El contrato de motor, en Rust

El doc A define ocho hooks en bash (R-ENG-1). En Rust, un motor es **un
manifiesto de datos** (§ 4.3) más, opcionalmente, un módulo con lógica
propia. Los hooks que hoy son funciones pasan a ser **campos del manifiesto**:

```rust
// crates/ropero-core/src/engine.rs

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineManifest {
    pub id: EngineId,
    pub label: String,
    /// Condiciones de compatibilidad (SO, hardware). Ver § 4.3.
    pub compat: Vec<CompatRule>,
    /// Comandos externos de los que depende.
    pub deps: Vec<String>,
    /// Cómo se instala. `Manual` significa que ropero no lo instala.
    pub install: InstallSpec,
    /// Puerto por defecto de este motor.
    pub port: u16,
    /// Si el motor habla el protocolo OpenAI natively.
    pub native_auth: bool,
    /// Cómo se construye la línea de órdenes. `Template` para el 90 % de los
    /// casos; `Custom` (un módulo Rust) para Strata y FLM.
    pub launch: LaunchSpec,
    /// Estado de madurez declarado (R-CTX-3).
    pub maturity: Maturity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Maturity {
    /// Probado en la máquina del dueño.
    Proven,
    /// Integrado pero nunca arrancado.
    Integrated,
    /// A medio integrar.
    Partial,
}
```

**`Maturity` es un requisito, no un comentario.** El doc A mide seis motores
probados y dos sin probar. Que esa distinción sea un campo del manifiesto
permite que `ropero install engines` lo muestre y que `ropero doctor` avise.
Es el mecanismo que convierte R-CTX-3 en algo verificable en lugar de una
frase en la documentación.

#### 3.3.2 Compatibilidad

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CompatRule {
    Os { any: Vec<Os> },
    /// Requiere GPU NVIDIA con `nvidia-smi` presente.
    NvidiaSmi,
    /// Requiere Apple Silicon con Metal.
    Metal,
    /// Requiere NPU AMD XDNA.
    AmdXnpu,
    /// Requiere que el motor esté ya instalado.
    Installed,
    /// Requiere un mínimo de RAM (GB).
    MinRam { gb: u32 },
}
```

Las reglas se evalúan **todas** y se acumulan: un motor incompatible explica
**todas** las razones, no la primera. El motivo es que el usuario que está en
Linux y quiere instalar TensorRT-LLM necesita saber que le faltan *dos* cosas
(a NVIDIA y a 32 GB de RAM), no una por intercourse.

#### 3.3.3 Identidad de proceso: `pid + start_time`

Ésta es la pieza que impide matar procesos ajenos (P-3).

```rust
// crates/ropero-runtime/src/process.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcIdentity {
    pub pid: u32,
    /// Segundos desde epoch del inicio del proceso. En Linux, campo 22 de
    /// /proc/<pid>/stat. En macOS, el `start time` de sysctl KERN_PROC_PID.
    pub start_time: u64,
    /// El hash del binario ejecutado, si se pudo resolver.
    pub exe_digest: Option<[u8; 32]>,
}
```

El **PID no basta**: los PIDs se reciclan. Un guard de puerto que sólo compruebe
el PID puede, en el peor caso, matar un proceso ajeno que heredó ese número.
Con `start_time`, la coincidencia es real.

`exe_digest` (SHA-256 del binario) es la tercera capa: aunque el PID y el
`start_time` coincidan —que es prácticamente imposible—, el binario tiene que
ser el nuestro.

**El orden de las comprobaciones al parar** (R-EST-8):

```
1. ¿Hay algo escuchando en el puerto?        → si no, no hay nada que parar.
2. ¿El PID del estado es el del puerto?      → si no, es ajeno. NO se toca.
3. ¿El start_time coincide?                  → si no, es otro proceso. NO se toca.
4. ¿El exe_digest coincide?                 → si no, es otro programa. NO se toca.
5. Ahora sí: SIGTERM al grupo de procesos.
6. Esperar hasta el deadline.
7. SIGKILL al grupo.
```

Los pasos 2-4 son la diferencia entre «ropero para lo que arrancó» y «ropero
mata lo que encuentra».

#### 3.3.4 El estado en disco

El estado **en disco es la fuente de verdad** (P-4): si el proceso se reinicia,
ropero debe saber qué había corriendo.

```
$STATE_DIR/                       # por defecto ~/.local/state/ropero
├── ports/
│   └── 9999.state                # un fichero por puerto gestionado
├── locks/
│   └── 9999.lock                 # lock de exclusión, con PID y timestamp
└── logs/
    └── 9999.log                  # log del motor
```

Formato de `ports/9999.state`: **líneas `CLAVE=valor`**, tal y como fija
R-ARQ-9 del doc A. El doc A es normativo sobre el comportamiento observable, y
el formato del fichero de estado es observable (está en disco, lo puede leer
un usuario, y hay herramientas que lo leen). **No se cambia.**

```ini
MODEL=groot
ENGINE=llama-server
PORT=9999
PID=42137
STARTTIME=812345678
EXEDIGEST=3f2a9c...
LEVEL=high
STAMP=a1b2c3d4e5f6...
TS=2026-11-12T18:45:12Z
LOG=/home/usuario/.local/state/ropero/logs/9999.log
VERSION=1
```

**Las tres claves nuevas** (`STARTTIME`, `EXEDIGEST`, `VERSION`) son
**adiciones compatibles**: las ocho claves del doc A siguen ahí, con el mismo
nombre y el mismo significado, y un lector de v1 sigue entendiendo el
fichero. La escritura sigue siendo **atómica** (fichero temporal + `rename`)
y **conserva** las claves que no vienen en la llamada (R-ARQ-9).

**`stamp` es el sello** (R-RUN-8): un hash del modelo, el nivel, el contexto y
los ficheros. Si el sello del estado **no coincide** con el sello que la
configuración actual produce, el modelo se considera **obsoleto** y hay que
reiniciarlo. Es lo que hace que cambiar `[model].context` no deje corriendo un
server con el contexto viejo. La comparación es de igualdad de strings; no
hay nada que «recalcular».

#### 3.3.5 El lock de puerto

`mkdir` es atómico en POSIX. El lock es un **directorio** `locks/9999.lock` con
el PID dentro, y **caduça** si el ropero que lo tenía muere (R-RUN-17): quien
encuentra el lock comprueba el PID; si el proceso no existe, el lock es
basura y se retira.

Adquirir el lock:

```
1. mkdir locks/9999.lock        → si falla con EEXIST: ya hay alguien.
2. escribir el PID dentro.
3. Comprobar que el PID sigue vivo (read lock, verificar, write lock).
   Si no → otro proceso lo tomó entre medias → reintentar (máx. 3).
```

#### 3.3.6 La sonda de readiness

El doc A define una jerarquía de niveles. En Rust:

```rust
pub enum Probe {
    /// El puerto acepta conexiones TCP.
    Tcp,
    /// El endpoint de salud del motor responde 200.
    Http { path: &'static str },
    /// El motor responde a una inferencia mínima.
    Inference { prompt: &'static str },
}

pub struct ProbePlan {
    /// Se prueban en orden; el primero que pasa es suficiente.
    pub levels: Vec<Probe>,
    /// Presión de cada nivel, de menor a mayor.
    pub timeout: Duration,
    /// Backoff entre intentos.
    pub backoff: Duration,
}
```

**La sonda de inferencia es la única que demuestra que el modelo está
*listo*.** La de TCP sólo demuestra que hay un socket. La de HTTP sólo que el
servidor web responde. Para un modelo de 60 GB, la diferencia entre «el puerto
está abierto» y «el modelo responde» es la diferencia entre «ropero funciona» y
«ropero dice que funciona».

### 3.4 M-03 · `ropero-download` — descargas

El doc A fija la secuencia exacta (R-MOD-16) y el motor de descarga es `hf` y
sólo `hf` (R-MOD-18). El trabajo aquí es **encapsular `hf` con las garantías
que hoy no tiene**:

```rust
pub struct DownloadRequest {
    pub repo: String,          // "owner/repo"
    pub file: String,
    pub dest: PathBuf,         // ruta final
    pub size_hint: Option<String>,
}

pub enum DownloadOutcome {
    /// Ya estaba y no está vacío.
    AlreadyPresent,
    /// Descargado ahora.
    Fetched { bytes: u64 },
    /// Reanudado desde un fichero parcial.
    Resumed { bytes: u64 },
}
```

**El fichero vacío no cuenta como descargado.** Es un detalle que cuesta una
descarga de 40 GB si se ignora: `hf` deja un fichero de 0 bytes cuando falla,
y la comprobación por *existencia* lo da por bueno.

### 3.5 M-04 · `ropero-forge` — la fabricación de GGUFs

El módulo más grande y el que tiene más deudas. Se divide en sub-módulos
claros:

```
ropero-forge/
├── src/
│   ├── lib.rs
│   ├── pipeline.rs     # las seis fases, como máquina de estados
│   ├── phases.rs       # cada fase: precondición, acción, postcondición
│   ├── checkpoint.rs   # la taxonomía de cinco clases (R-GGUF-32)
│   ├── imatrix.rs      # el importance matrix
│   ├── mtp.rs          # la cabeza MTP: el bloque que el imatrix no cubre
│   ├── engine.rs       # la PRECONDICIÓN: el motor llama.cpp (S-7, H-27)
│   ├── gguf.rs         # lectura de la cabecera GGUF (sustituye a ggufcheck.py)
│   └── provenance.rs   # la procedencia: el sidecar que hace el artefacto auditable
```

#### 3.5.1 La precondición del motor

Es el arreglo de mayor valor de todo el documento. Hoy (R-GGUF-55) la
conversión falla sin motor y el clasificador dice *«faltan ficheros en el
clon: la descarga está incompleta»* — un diagnóstico **falso** que empuja a
borrar y re-descargar 60 GB.

```rust
// crates/ropero-forge/src/engine.rs

/// Comprueba la precondición del gguffer ANTES de descargar nada.
/// Devuelve el diagnóstico correcto, no "la descarga está incompleta".
pub fn ensure_toolchain(spec: &ToolchainSpec) -> Result<Toolchain, ToolchainError>;

#[derive(Debug, thiserror::Error)]
pub enum ToolchainError {
    /// No hay ningún motor llama.cpp instalado.
    #[error("el gguffer necesita un motor llama.cpp (el oficial o tu fork).\n\
             Ninguno está instalado.\n\
             Instálalo con: ropero install engines llama-server")]
    NoEngineInstalled,

    /// Hay motor, pero está en un directorio que el gguffer no mira.
    #[error("hay un motor llama.cpp en {found}, pero el gguffer busca en {expected}.\n\
             Instala el motor con `ropero install engines llama-server` \
             (o exporta LLAMA_DIR={found})")]
    EngineInWrongPath { found: PathBuf, expected: PathBuf },

    /// Hay motor, pero le falta una de las herramientas.
    #[error("el motor en {root} no tiene {tool} (se necesita para {purpose}).\n\
             Vuelve a compilar el motor: ropero install engines llama-server --force")]
    MissingTool { root: PathBuf, tool: &'static str, purpose: &'static str },
}
```

**Cada variante lleva la acción.** Eso es P-2 aplicado al diagnóstico: el
mensaje dice qué hacer, no sólo qué falló.

Y lo que es realmente importante: **este check ocurre antes de la fase 1
(descarga)**. En el orden actual, el fallo aparece después de descargar 60 GB.

#### 3.5.2 Las seis fases

El doc A define las fases (R-GGUF-16…21). Cada una es una función con
precondición, acción y postcondición explícitas:

```rust
pub enum Phase {
    Resolve,     // 1 · resolver el repo y los ficheros
    Download,    // 2 · bajar los pesos (o el GGUF fuente)
    Convert,     // 3 · convertir a GGUF intermedio
    Imatrix,     // 4 · calcular el importance matrix
    Quantize,    // 5 · cuantizar al objetivo
    Verify,      // 6 · verificar el resultado y escribir la procedencia
}
```

**Cada fase escribe un checkpoint** antes de empezar y lo marca al terminar
(R-GGUF-32). Un fallo en la fase 5 (la cara) **no borra nada**: el clon y el
intermedio se quedan, y el error dice exactamente cómo reintentar sólo la
cuantización. Perder 60 GB por un fallo de cuantización es la herida que
R-GGUF-16 ya prohíbe y que el código incumple.

#### 3.5.3 La taxonomía de los checkpoints

Cinco clases (R-GGUF-32..36). Cada checkpoint lleva:

```rust
pub struct Checkpoint {
    pub phase: Phase,
    /// Estado dentro de la fase: si se reintenta, desde dónde.
    pub state: CheckpointState,
    /// Los datos que la fase necesita para reintentarse sola.
    pub resume: ResumeData,
    pub ts: SystemTime,
}
```

La distinción clave de la taxonomía: un checkpoint **no verificable** (no se
puede comprobar sin rehacer el trabajo) es de una clase distinta de uno
**verificable** (se puede comprobar cheaply y continuar). El intermedio GGUF
es verificable por su cabecera; el imatrix, por su tamaño y formato.

### 3.6 M-05 · `ropero-server` — la puerta única

Reemplaza `bin/server/endpoint.py` (1.598 líneas) y su test de auth que **da
verde falso** (R-TST-19, wound #1).

```rust
// crates/ropero-server/src/lib.rs

pub struct ServerConfig {
    pub bind: SocketAddr,
    /// Clave de la API. Nunca en el argv del motor (P-8).
    pub api_key: Option<String>,
    /// Hacia dónde se enruta cada petición.
    pub router: Arc<dyn Router>,
    pub upstream: UpstreamConfig,
}

/// Arranca el servidor. Devuelve cuando está listo para aceptar.
pub async fn serve(cfg: ServerConfig, shutdown: Shutdown) -> anyhow::Result<()>;
```

**El arreglo del wound #1 es estructural, no cosmético.** El test actual hace:

```bash
rc=$?          # ← el rc del bloque python se captura aquí
...            # ← y NUNCA se vuelve a mirar
exit 0         # ← inalcanzable, pero si llega, el test pasa
```

En Rust el equivalente imposible es: el test llama a la **misma función** que
usa el servidor para autenticar, y afirma sobre el **resultado**, no sobre el
código de salida de un subproceso:

```rust
#[tokio::test]
async fn rechaza_peticion_sin_clave() {
    let app = build_app(test_server()).await;
    let res = app.oneshot(Request::get("/v1/models").body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn rechaza_peticion_con_clave_incorrecta() {
    let app = build_app(test_server()).await;
    let res = app.oneshot(
        Request::get("/v1/models")
            .header("authorization", "Bearer incorrecta")
            .body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}
```

No hay forma de que ese test pase con la autenticación rota: si `build_app`
deja de comprobar la clave, los dos tests fallan. Es la diferencia entre
comprobar el **comportamiento** y comprobar el **código de salida de un
proceso**.

**La clave nunca va en el argv del motor** (P-8, wound medido: la clave es
legible por cualquier usuario del mismo host en `/proc/<pid>/cmdline`). En
Rust la clave se pasa por **entorno** o por **fd**:

```
--api-key-file /run/user/1000/ropero/key   (modo 0600)
```

La variable de entorno tiene una limitación conocida (`/proc/<pid>/environ` es
legible por el mismo usuario), pero es estrictamente mejor que el argv, que es
legible por **todos**. El fichero con modo 0600 es el único que resuelve el
problema por completo.

### 3.7 M-06 · `ropero-cli` — la presentación

```rust
// crates/ropero-cli/src/main.rs
#[derive(Parser)]
#[command(name = "ropero", version, about = "gestor de modelos locales")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Sirve un modelo (atajo de `use`).
    Use { model: String, #[command(flatten)] opts: RunOpts },
    Stop { target: Option<String> },
    Status { model: Option<String> },
    List { #[arg(long)] all: bool },
    Template,
    Args { model: String },
    Levels { model: String },
    Path { model: String },
    Install { #[command(subcommand)] action: InstallAction },
    Engines,
    Models,
    Doctor,
    Check,
    Config { engine: Option<String> },
    Gguffer { #[command(subcommand)] action: GgufferAction },
    Serve { #[arg(long)] port: Option<u16> },
    Stats { #[arg(long)] watch: bool },
    Test { model: String },
    /// Sólo para el desarrollo: emite el JSON Schema del TOML de modelo.
    Schema,
    /// Autocompletado de shell.
    Completion { shell: Shell },
}
```

**Regla de la CLI**: la lógica no vive aquí. Un `match` que hace trabajo de
negocio en `main.rs` es un bug de arquitectura; el test del CLI comprueba
**código de salida y stdout**, no lógica.

**Los códigos de salida son contrato** (R-CLI-4):

| Código | Significado |
|---|---|
| 0 | Ok |
| 1 | Error (de ejecución) |
| 2 | Error de uso (argumentos, nivel no declarado, opción desconocida) |
| 130 | Interrumpido por Ctrl-C |

El plugin de opencode y los scripts del usuario dependen de esta tabla.

---
## 4. Contratos de datos

Un contrato de datos es un formato que dos programas usan para entenderse. En
ropero hay cinco, y **los cinco están versionados**: un cambio incompatible
obliga a migrar, y la migración se hace explícita.

### 4.1 El TOML de modelo (`models/<engine>/<nombre>.toml`)

**Estabilizado.** El esquema es el del doc A § 4.2.1 y el de
`models/_template.toml`. Este documento **no lo cambia** (S-3); lo convierte en
un tipo Rust con validación.

```rust
// crates/ropero-core/src/model.rs

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]     // ← la decisión que hace la validación real
pub struct ModelSpec { /* … */ }
```

**`deny_unknown_fields` es la decisión de diseño más importante del formato.**
Hoy, una clave desconocida en `[build]` genera un **aviso** (según
`models/_template.toml`); el validador lo permite. Con `deny_unknown_fields`,
una clave mal escrita es un **error**, y el error dice cuál:

```
models/llama/groot.toml:34: clave desconocida en [build]: "cuantization"
  ¿quise decir "quant"?
```

Un typo en `cuantization` que hoy pasa desapercibido significa que la
cuantización sale con el default en vez de la pedida. El usuario lo descubre
tres días después, cuando el modelo no tiene la calidad que esperaba. El
error cuesta un segundo y evita un misterio.

**Consecuencia**: el `models/_template.toml` y los 45 TOML existentes deben
validar contra el esquema nuevo **antes** de que el validador nuevo entre en
producción. Esa es una tarea de la fase F1 (T-05).

#### 4.1.1 Reglas de validación (el contrato completo)

Son **15 reglas**. Cada una es una violación del catálogo y sale con código 1.

| Regla | Violación | Código de salida |
|---|---|---|
| `[model].name` es kebab-case y único global (activos + `fired/`) | error | 1 |
| `[model].name` **debe coincidir con el nombre del fichero** | error | 1 |
| `engine` es un id canónico o un alias legacy conocido | error | 1 |
| `format = "safetensors"` ⇒ `[model].path` **obligatorio** | error | 1 |
| `format ≠ "safetensors"` ⇒ `[model].path` **prohibido** | error | 1 |
| `format = "gguf"` ⇒ `[files].gguf` obligatorio | error | 1 |
| `[download]` presente sin `[build]` ⇒ `repo` y (`file` si gguf) obligatorios | error | 1 |
| Un `${fichero}` en `[args]` no declarado en `[files]` | error | 1 |
| `[levels].default` debe estar en `[levels].list` | error | 1 |
| `[aliases]` apunta a un nivel que no existe | error | 1 |
| `[options.default]` debe tener su clave en `[options].list` | error | 1 |
| `[args.flag.<f>].requires` referencia una clave de `[files]` inexistente | error | 1 |
| `[limits].min_ram_gb > 0` | error | 1 |
| Clave desconocida en cualquier tabla | error | 1 |
| `context` debe ser > 0 y múltiplo de 32 | error | 1 |

Esta tabla es el contrato de validación de la v2. Las cuatro comprobaciones
adicionales que hace la v1 —niveles, flags, opciones y presencia de
`[download]`/`[build]`— aún no están implementadas y están registradas como
deuda en el CHANGELOG.

**Tener `[download]` y `[build]` a la vez es un estado válido del TOML, no un
error.** El validador lo acepta; la fila que lo prohibía (con código 2) se
borró (decisión del dueño, 2026-11-14). Declarar las dos vías significa que el
modelo **se puede obtener de dos maneras**, y eso es una propiedad del modelo,
no un defecto de su ficha. Por tanto:

- El **validador** no dice nada sobre qué vía tomar: las dos son válidas y las
  dos se validan por separado.
- La **elección del camino** es contrato de `ropero install models` (F5/T-25,
  R-MOD-29): con las dos vías, se pregunta al usuario; con `--prio` se elige
  sin preguntar. El contrato completo de `--prio` está en § 4.10.7.
- El **exit 2** —error de uso— lo reservan los comandos, no el validador.

**La regla de `name` == nombre de fichero** es una de las deudas medidas (H-23):
hoy el validador **no** la comprueba. Se añade aquí.

**El error de validación debe incluir la ruta y la línea.** Es el requisito que
convierte un validador en una herramienta y no en un obstacle:

```
models/llama/groot.toml:12
  [model].format = "safetensors" requiere [model].path
```

### 4.2 Los identificadores de motor

Ocho motores, con ids **canónicos** y alias **legacy**:

| Canónico | Alias legacy aceptados |
|---|---|
| `llama-server` | `llama` |
| `llama-server-official` | — |
| `mlx-serve` | `mlx_lm` |
| `fastflowlm` | `flm` |
| `tensorrt-llm` | `tensorrt` |
| `vllm` | — |
| `airllm` | — |
| `strata` | `strata_engine` |

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EngineId {
    LlamaServer,
    LlamaServerOfficial,
    MlxServe,
    FastFlowLm,
    TensorRtLlM,
    Vllm,
    AirLlm,
    Strata,
}
```

El enum **es** la tabla de motores: añadir un motor es añadir una variante (y
el compilador señala todos los `match` que hay que tocar) y un manifiesto. Es
la respuesta directa a «integrar un engine es desarrollo»: en el modelo
actual, un motor nuevo es un fichero bash con ocho hooks; aquí es una variante
de enum + un fichero de datos, y el compilador dice dónde falta actualizar.

Un alias legacy se canonicaliza **con un aviso**, nunca en silencio:

```
AVISO: el motor 'mlx_lm' es un alias antiguo; se usa 'mlx-serve'
```

### 4.3 El manifiesto de motor (`engines/<id>.toml`)

**Nuevo en v2.** Es la embodiment del «engine como dato» (§ 3.3.1).

```toml
# engines/llama-server.toml
id = "llama-server"
label = "llama-server (fork TurboQuant)"
maturity = "proven"

compat = [
  { os = ["linux"] },
  { os = ["macos"], metal = true },
]
deps = ["cmake", "git", "ninja"]
port = 9999
native_auth = false

[install]
kind = "source"
repo = "Lorenzo-SF/ropero-llama-turboquant"
public_fallback = "https://github.com/Lorenzo-SF/ropero-llama-turboquant"
dir = "LLAMA_DIR"
build = "cmake"
patches = ["patches/nvfp4-y-pattern.patch"]

[install.build.linux]
requires = ["nvidia-smi", "nvcc", "g++"]
cmake_args = ["-DGGML_CUDA=on"]

[install.build.macos]
requires = ["cmake", "clang"]
cmake_args = ["-DGGML_METAL=on"]

[launch]
kind = "template"
binary = "${LLAMA_DIR}/build/bin/llama-server"
template = [
  "--model", "${gguf}",
  "--host", "127.0.0.1",
  "--port", "${port}",
]
auth = "none"
```

**Tres motores no encajan en `kind = "template"`** y por eso el enum tiene esa
variante:

| Motor | Por qué no es una plantilla |
|---|---|
| `strata` | No es un servidor: es un motor que necesita un *pack* propio, un *expert-profile* y un *tune*. El arranque real lo hace `bin/server/strata-run.sh`. |
| `fastflowlm` | El servidor se lanza vía una CLI propia de FLM con su propio modelo de datos y su comprobación de NPU. |
| `airllm` | El modelo vive **en el proceso del proxy**, no en un subproceso. El flujo de datos es radicalmente distinto. |

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LaunchSpec {
    /// La mayoría: una plantilla de argv.
    Template { binary: String, template: Vec<String>, auth: AuthMode },
    /// Un módulo Rust propio.
    Custom { module: String },
}
```

El `Custom` es la **válvula de escape**: cuando un motor no se puede expresar
como plantilla, se escribe Rust para él — y ese Rust vive en
`ropero-runtime/src/engines/`, no se dispersa por el código.

### 4.4 El estado en disco (§ 3.3.4)

Formato `CLAVE=valor`, un fichero por puerto (§ 3.3.4). **El formato es
estable**: un `ropero` v2 tiene que poder leer el estado que dejó un `ropero`
v1, porque el usuario puede tener los dos instalados durante la transición
(§ 9, F7).

La clave `VERSION` es lo que lo permite:

- **Sin `VERSION`** (estado escrito por v1): se migra al leer. Las claves que
  faltan (`STARTTIME`, `EXEDIGEST`) se derivan o se dejan vacías, y el
  proceso se trata como **ajeno** hasta que se verifique la identidad por
  otros medios (§ 3.3.3). Es la postura segura: sin `start_time` no se puede
  probar que el PID sea el nuestro.
- **`VERSION=1`**: estado nativo de v2.
- **`VERSION` mayor que la conocida**: **se rechaza con un mensaje claro**
  («estado escrito por una versión más nueva de ropero»), no se interpreta a
  medias. Un estado a medias es peor que no tener estado.

### 4.5 El contrato de stdout (`ropero use`)

Es contrato público: lo consume el plugin de opencode (R-OC).

```
ROPERO_PORT=9999
ROPERO_MODEL=groot
ROPERO_ENGINE=llama-server
```

**Tres reglas:**

1. **La última línea es siempre `ROPERO_PORT=<n>`** y nada más va después.
   Un consumidor puede hacer `tail -1`.
2. **Sólo esas líneas** van a stdout. Los mensajes de progreso, avisos y
   errores van a **stderr**. Esto permite `PORT=$(ropero use groot | tail -1)`.
3. En modo `--background`, las tres líneas se emiten **después** de que el
   motor esté listo, no antes. El consumidor no debe recibir un puerto que aún
   no responde.

Regla 3 es la que más se subestima y la que más falla: si `ropero use` devuelve
el puerto antes de que el motor esté listo, el consumidor se conecta y recibe
*connection refused*, y el error se atribuye al servidor HTTP y no a ropero.

### 4.6 La API del endpoint

Compatible con la de OpenAI. Los endpoints que ropero implementa:

| Método | Ruta | Qué hace |
|---|---|---|
| `GET` | `/v1/models` | Lista los modelos servidos ahora |
| `GET` | `/v1/models/{id}` | Un modelo concreto |
| `POST` | `/v1/chat/completions` | Chat (streaming SSE o no) |
| `POST` | `/v1/completions` | Completions |
| `POST` | `/v1/embeddings` | Embeddings |
| `GET` | `/health` | Salud del proxy (sin auth: no filtra información) |
| `GET` | `/ready` | Todos los upstreams listos |

**Reglas del proxy:**

- **Reescritura de `model`**: el cliente pide `groot`; el motor de destino
  puede esperar `Qwen3.8-Flash-Next-Gyro-M-TQ2_0`. El proxy reescribe en el
  cuerpo antes de reenviar y **no reescribe la respuesta** (R-API-8).
- **Streaming**: si el cliente pide `stream: true`, la respuesta se reenvía
  **token a token**, sin bufferizar. Un proxy que bufferiza el stream rompe la
  perceived-latency de un modelo local, que es su única ventaja.
- **Auth**: `Authorization: Bearer <clave>`. Sin clave ⇒ 401. Con clave
  incorrecta ⇒ 401. **El mismo error en los dos casos** (no se distingue «no
  hay clave» de «la clave está mal»: esa distinción es un oráculo para un
  atacante).
- **Errores del upstream**: se traducen a errores OpenAI-shaped con el
  `status` correcto. Un 503 del motor no puede volver como 200 con un cuerpo
  de error.

---

## 5. Patrones de diseño

Un patrón de diseño que no se nombra en el código es un patrón que nadie puede
evaluar. Los que se usan aquí están justificados por un problema concreto que
el doc A ha medido.

### 5.1 Los once patrones y dónde se aplican

| # | Patrón | Problema real que resuelve | Dónde vive |
|---|---|---|---|
| 1 | **Newtype** | `Port` y `ModelName` son `u16` y `String`: se confunden entre sí y con cualquier otro número. Un `Port(9999)` no se puede pasar donde se espera un nivel. | `core::types::{Port, ModelName, LevelName}` |
| 2 | **Enum cerrado** (en vez de `String` para el motor) | El motor es un conjunto cerrado de 8. Con `String`, un typo (`"llama-server "` con espacio) se descubre en runtime, en el motor, después de descargar 40 GB. | `core::engine::EngineId` |
| 3 | **State machine** (estados explícitos) | El estado repartido en ficheros sueltos se desincroniza. El doc A mide `stop` que mata un proceso ajeno porque el estado no distingue «mío» de «ajeno». | `core::state::RuntimeState` + `runtime::supervisor` |
| 4 | **Strategy** (por motor) | Cada motor arranca distinto. Hoy son 8 ficheros bash con 8 hooks; mañana serían 8 `match` con 8 brazos, y cada cambio toca los 8. | `runtime::engines::{trait Engine, …}` |
| 5 | **Repository** (estado en disco) | El acceso al disco se hace desde 6 sitios y cada uno reimplementa el parseo y el manejo de errores. | `runtime::state::StateStore` |
| 6 | **Ports & Adapters** (hexagonal) | El dominio tiene que ser testeable sin procesos reales ni disco. | La separación `core` / `runtime` |
| 7 | **Builder** (argv y peticiones) | El argv tiene 4 fuentes que se combinan en un orden fijo; con constructores posicionales el orden se olvida. | `core::args::ArgvBuilder` |
| 8 | **Null Object / sentinel** (probe) | La sonda tiene tres niveles y el superior es el único que prueba que el modelo responde. | `runtime::probe::Probe` |
| 9 | **Dependency Injection** (manual, por constructor) | Los tests necesitan un motor falso. Si el motor se crea dentro de la función, no hay test. | Todos los constructores de `runtime` |
| 10 | **Result type / errores tipados** | Un `String` de error no se puede comprobar en un test sin comparar texto. | `thiserror` en todo el dominio |
| 11 | **Adapter** (el que traduce HTTP↔motor) | La API de OpenAI y la de cada motor no coinciden. El adapter es donde se reconcilian. | `server::upstream::{OpenAiUpstream, …}` |

### 5.2 Los principios que extendemos al doc A

El doc A define doce principios (P-1…P-12). Estos cuatro se añaden:

| # | Principio | Formulación | Por qué |
|---|---|---|---|
| **P-13** | **Un error dice qué hacer** | Todo mensaje de error incluye la acción que resuelve el problema, o la razón por la que no hay acción. | El wound del gguffer («la descarga está incompleta») es un error que **miente** y empuja a una acción destructiva. Un error que no dice qué hacer es un error que se ignora. |
| **P-14** | **Un test que puede pasar sin comprobar nada no es un test** | Cada test debe poder fallar. Un test cuyo resultado depende de un código de salida que nadie mira es peor que no tener test. | El wound #1 (auth verde falso) es exactamente esto. |
| **P-15** | **El estado es una máquina, no un conjunto de ficheros** | El estado de un runtime se lee de un sitio y se interpreta con una máquina de estados explícita. | La separación de estados es lo que permite distinguir `Failed` de `Degraded`. |
| **P-16** | **La precondición se comprueba antes de la acción cara** | Antes de descargar 60 GB, se comprueba que el motor existe, que hay espacio, que la RAM da. | R-GGUF-55 y R-GGUF-56 son dos casos medidos de esto. |

### 5.3 Patrones que quedan **prohibidos** (y por qué)

| Patrón no usado | Por qué está prohibido aquí |
|---|---|
| `unwrap()` / `expect()` en código de producción | Un panic en un destructor o en un handler de señal deja el estado a medias. Se usa `expect()` **sólo** en `main` y en invariantes probadas por el compilador. |
| `unwrap_or_default()` sobre un valor de configuración | Un default silencioso sobre un puerto o un contexto es un fallo de producción que se manifiesta como «el modelo va lento». |
| `println!` en `core`, `runtime`, `server` | Esos crates no saben qué es una pantalla. Escriben con `tracing`. |
| `std::env::set_var` en el código | El entorno del proceso es de quien lo lanza. Se **lee**, no se modifica. Modificarlo afecta a los hijos de forma invisible. |
| Errors como `Box<dyn Error>` sin tipo | El `match` sobre el error es lo que permite reaccionar distinto a «modelo desconocido» y a «puerto ocupado». |
| `sleep` para sincronizar | Es una espera ciega. Se espera a un evento con deadline. |
| Comments en inglés / mensajes en inglés | La convención del repo: código en inglés, texto en español. |
| Flags globales (`--verbose` como variable de entorno leída en cualquier sitio) | Un `--json` de `ropero use` tiene que llegar hasta el endpoint; si se lee del entorno en tres capas, es imposible saber de dónde salió. |

---
## 6. Metodología de trabajo

### 6.1 La regla que gobierna todas las demás

> **El test primero, y el test tiene que poder fallar.**

En el repo v1 hay 16 ficheros de test y 138 aserciones, y tres heridas
medidas (R-TST-19…23). La primera —el test de auth que da verde con la
autenticación rota— no es un descuido de escritura: es un **test cuyo
resultado no depende de lo que prueba**. Por eso en v2 hay dos reglas
distintas y las dos son obligatorias:

**Regla T-1 (el test primero).** Ninguna función de producción se escribe sin
un test que ya falla. El orden observable en el commit es: primero el test
(rojo), después la implementación (verde). En el mensaje del commit, el
cuerpo describe el fallo que el test producía.

**Regla T-2 (el test puede fallar).** Antes de dar por bueno un test, se
comprueba que **falla cuando el comportamiento se rompe**. Esto se hace de
dos formas, y las dos:

- **Mutación manual**: se cambia el código a propósito (se invierte una
  condición, se cambia un código de salida) y se comprueba que el test se
  pone rojo. Se revierte.
- **Test de la sonda**: para los tests cuyo sujeto es «el sistema se
  comporta así», hay un segundo test que rompe el comportamiento a propósito
  y comprueba que el primero falla.

La regla T-2 es la que evita el wound #1. Un test que no puede fallar es
peor que no tener test: da una confianza falsa y ocupa sitio en la batería.

### 6.2 Ciclo de trabajo

Cada tarea (§ 9) sigue el mismo ciclo, y **el ciclo no se salta nada**:

```
1. LEER    Leer el requisito (doc A) y su evidencia. Anotar el verbo.
2. MODELAR Escribir el tipo (struct/enum) o la firma, con doc-comment.
           Si no se puede escribir la firma, el requisito no está
           entendido: volver a 1.
3. TEST    Escribir el test que falla. Ejecutarlo. VER QUE FALLA.
           (Un test que pasa sin haber escrito nada es un test vacío.)
4. CODE    Escribir lo mínimo que hace pasar el test.
5. VERIFICAR  Ejecutar la batería completa. Si algo falla, se arregla
           —no se baja el listón del test.
6. MUTAR   Romper a propósito. Comprobar que el test se pone rojo.
           (T-2)
7. REFACTOR  Limpiar. Los tests siguen en verde.
8. COMMIT  Un commit. Formato § 6.4.
9. DOC     Si el cambio altera un contrato, actualizar el doc A en el
           MISMO commit (§ 6.6).
```

**El paso 3 no se salta.** Es tentador escribir la implementación y luego el
test, porque el test pasa a la primera y da sensación de control. Ese es
exactamente el camino que produce el wound #1: un test escrito después del
código, que verifica lo que el código hace, no lo que el código debe hacer.

### 6.3 Commits

**Conventional Commits, en español.** El repo v1 ya los usa.

```
<tipo>(<ámbito>): <descripción en imperativo>

<cuerpo: por qué, no qué. Referencia el R-XXX-N que cierra.>
<pie: issues>
```

| Tipo | Cuándo |
|---|---|
| `feat` | Capacidad nueva |
| `fix` | Corrección de un comportamiento |
| `refactor` | Reestructuración **sin** cambio de comportamiento |
| `test` | Sólo tests |
| `docs` | Sólo documentación |
| `build` | Dependencias, toolchain, CI |
| `chore` | Lo demás |

**Un commit = un cambio verificable.** Si el mensaje necesita una «y», son
dos commits.

**El cuerpo explica el porqué.** Ejemplo real:

```
fix(forge): comprueba la precondición del motor antes de descargar

El gguffer buscaba el motor sólo en $LLAMA_DIR. Con el motor oficial
instalado ($LLAMA_OFFICIAL_DIR) no lo encontraba, y el clasificador de
errores atribuía el fallo a "la descarga incompleta", lo que empujaba a
borrar y re-descargar 60 GB para resolver un problema de entorno.

Ahora ensure_toolchain() se ejecuta antes de la fase 1 y cada causa tiene
su propia variante de error, con la acción que la resuelve.

Closes: R-GGUF-54, R-GGUF-55, R-GGUF-56, R-GGUF-57, R-GGUF-58
Refs: H-27, H-28, H-29
```

### 6.4 Ramas y pull requests

- Rama por tarea: `v2/<area>-<descripción-corta>`. Ejemplo: `v2/forge-precondicion-motor`.
- **Nunca se commitea directamente en `main`.**
- El PR describe: qué requisito cierra, qué tests lo cubren, y **qué se ha
  medido** (si el PR contiene cifras, van medidas, no estimadas).
- El merge lo hace el dueño, con squash. Es la convención del repo y no se
  cambia.

### 6.5 Qué se toca y qué no

| Se puede tocar en una tarea | No se puede tocar |
|---|---|
| El crate y los tests de la tarea | `main` |
| El fichero de test correspondiente | La CLI pública (salvo que la tarea lo diga) |
| El doc A, si la tarea cambia un comportamiento (y sólo entonces) | El esquema de los TOML de modelo |
| `Cargo.toml` (añadiendo dependencias, con ADR) | El `Cargo.lock` a mano (se genera) |

### 6.6 El doc A se actualiza con el código

**Regla dura.** Si una tarea cambia un comportamiento, el doc A se actualiza
**en el mismo commit**, con el mismo número de requisito y la evidencia
nueva. Un doc que describe un código que ya no existe es peor que no tener
doc: la especificación es el oráculo del golden master, y un oráculo
desalineado hace que los tests validen contra una fantasía.

**El formato de evidencia**: `fichero:línea` o `fichero:línea-línea`. Con la
línea, no el fichero solo: la evidencia tiene que ser localizable.

### 6.7 Cuando algo no está definido

Una IA que construye esto va a encontrar huecos. La regla es explícita para
que no improvise en silencio:

| Situación | Qué hacer |
|---|---|
| El doc A define el comportamiento | Implementarlo tal cual, y citar el R-XXX-N en el commit. |
| El doc A marca el requisito 🔶 | Implementarlo como estado objetivo, y marcarlo como tal en el código y en el CHANGELOG. |
| El doc A define un requisito contradictorio | **Parar y documentarlo.** Un commit que dice «implemento R-X-3 y R-X-7, que se contradicen» no se puede hacer. |
| El doc A no dice nada y el hueco es **menor** (un mensaje, un nombre, un default) | Elegir, escribir la decisión en el ADR (§ 6.8) y seguir. Un hueco menor no justifica parar. |
| El doc A no dice nada y el hueco es **mayor** (un formato nuevo, una API nueva, un cambio de comportamiento observable) | **Parar y preguntar.** Escribir el hueco en el issue y esperar. |
| El hueco afecta a la **seguridad** (identidad de proceso, clave, permisos) | **Parar y preguntar.** Siempre. |
| El hueco afecta al **formato del TOML de modelo** | **Parar y preguntar.** Es un contrato público. |

**Nunca** se inventa un comportamiento y se da por hecho. Un requisito
inventado que funciona es un requisito que nadie pidió y que nadie va a
mantener.

### 6.8 Los ADR

Un ADR (Architecture Decision Record) es un fichero de 10-20 líneas que
registra una decisión y su motivo. Vive en `docs/adr/`.

**Cuándo se escribe un ADR:**

- Se añade una dependencia nueva (no trivial).
- Se cambia una regla ya escrita (incluido este documento).
- Se rechaza una alternativa que alguien va a volver a proponer.
- Se toma una decisión con un coste que no se puede deshacer.

**Plantilla:**

```markdown
# ADR-NN · <título>

**Estado**: aceptada | rechazada | sustituida por ADR-NN
**Fecha**: 2026-11-12

## Contexto
<qué problema obliga a decidir; con cifras si las hay>

## Decisión
<qué se decide, en una frase>

## Alternativas consideradas
- <alternativa> — <por qué no>

## Consecuencias
<qué se gana; qué se pierde; qué queda atado>
```

**Los ADR son la memoria del proyecto.** El doc A explica *qué* hace ropero; un
ADR explica *por qué* se hizo así. Sin ADR, dentro de un año alguien
pregunta «¿por qué el estado es `CLAVE=valor` y no TOML?» y la respuesta es
«porque siempre fue así», que no es una respuesta.

### 6.9 Qué sigue siendo bash

La regla por defecto es: **nada**. Pero hay tres excepciones justificadas, y
cada una tiene su motivo y su fecha de revisión:

| Qué | Por qué sigue en bash | Cuándo se revisa |
|---|---|---|
| `install.sh` | Es el **primer** contacto del usuario con ropero, y tiene que funcionar en una máquina donde no hay Rust, ni compilador, ni red. Un instalador que necesita un compilador no puede instalar nada. | Cuando el repositorio distribuya binarios firmados, se puede sustituir por un `install.sh` de 20 líneas que descarga el binario. |
| `opencode-plugin/` | Es TypeScript, porque opencode lo carga como plugin de Node. No es una elección: es el formato que opencode consume. | Nunca (mientras opencode use Node). |
| `test/fakes/` | Los dobles (`hf` falso, `llama-quantize` falso) son scripts que **deben** parecer los binarios reales. En Rust serían binarios aparte, más pesados. | Cuando se tenga un `ropero-fakes` como crate. |

**Todo lo demás es Rust.** Un `bin/*.sh` nuevo requiere un ADR que explique
por qué.

### 6.10 Qué se entrega al final de cada fase

Cada fase entrega::

1. **Código que compila** y con la batería en verde.
2. **Tests nuevos** que fallaban antes (demostrado en el PR).
3. **Doc A actualizado** si cambió algún comportamiento.
4. **ADR** si hubo una decisión no trivial.
5. **CHANGELOG** con la entrada correspondiente.
6. **Un PR** con la descripción y las cifras medidas.

Una fase sin batería en verde **no está terminada**, por mucho que el código
«funcione en mi máquina». P-2 y P-14 aplicados a la entrega.

---

---

## 7. Definition of Done por requisito

### 7.1 Qué es una Definition of Done aquí

Una puerta de aceptación **no** es «está implementado». Es una frase que dice
**qué se comprueba y dónde**, y que un revisor (humano o IA) puede ejecutar
sin preguntar nada.

Las cuatro propiedades que toda fila de § 7.4 cumple:

1. **Nombra un artefacto.** Un fichero, un módulo o un test concreto. Nada de
   «en el sitio adecuado».
2. **Nombra una prueba que puede fallar.** Si el test no puede ponerse rojo,
   la puerta no vale (P-14).
3. **Es localizable.** La evidencia del doc A (`fichero:línea`) va como
   *semilla v1*: es el punto de partida del golden master (§ 8.1).
4. **Distingue lo ya implementado de lo objetivo.** Los requisitos 🔶 llevan
   su marca y su nota de cambio de arquitectura.

### 7.2 La puerta global

Una tarea está terminada cuando **todo** esto es cierto:

| # | Puerta | Cómo se comprueba |
|---|---|---|
| G-1 | El código compila sin warnings | `cargo clippy --all-targets -- -D warnings` |
| G-2 | El formato es el de `rustfmt` | `cargo fmt --check` |
| G-3 | La batería completa pasa | `cargo nextest run --all-features` |
| G-4 | Cada test nuevo fallaba antes | Evidencia en el PR (el fallo pegado) |
| G-5 | Cada test nuevo puede volver a fallar | Mutación deliberada (§ 6.1, regla T-2) |
| G-6 | La cobertura no baja | `cargo llvm-cov` ≥ umbral de § 9.4 |
| G-7 | Sin dependencias nuevas sin ADR | `cargo deny check` + revisión |
| G-8 | El doc A está actualizado si cambió un comportamiento | Revisión del diff |
| G-9 | El CHANGELOG tiene la entrada | Revisión del diff |
| G-10 | El mensaje del commit dice **por qué** | Revisión del diff |
| G-11 | La CLI no cambió su contrato | `cargo test --test cli_contract` |
| G-12 | No hay `unwrap()` en producción | `grep -rn '\.unwrap()' crates/ --include='*.rs'` salvo `main.rs` |

**G-5 es la puerta que el repo v1 no tenía**, y es la que habría detectado el
wound #1 automáticamente.

### 7.3 Puertas por familia

Cada familia de requisitos tiene un **tipo de puerta** y un **riesgo
característico** que la puerta debe cubrir:

| Familia | Tipo de puerta | Riesgo que cubre |
|---|---|---|
| R-ARQ | Estructural: el grafo de módulos y el tamaño del CLI | Que la lógica se disperse y el sistema deje de entenderse |
| R-DATA | De contrato: un TOML de entrada, un error de salida con `fichero:línea` | Que un dato mal formado pase silenciosamente |
| R-ENG | De enumeración: el motor es un caso de un `enum` | Que un motor nuevo se meio a medias |
| R-CLI | De interfaz: código de salida + stdout + stderr, byte a byte | Que el contrato con el usuario y con opencode se rompa |
| R-RUN | De proceso: hay un proceso real y se comprueba su estado | Que «arrancado» signifique «vivo y respondiendo» |
| R-EST | De no-destrucción: un proceso ajeno sobrevive a la operación | Que ropero mate lo que no es suyo |
| R-API | De protocolo: petición real contra el servidor, status + cuerpo | Que el proxy dé respuestas que no son OpenAI-shaped |
| R-ROUTE | De selección: la misma entrada va al destino correcto | Que el enrutado degrade la calidad en silencio |
| R-INST | De efecto: se comprueba el **disco**, no la salida | Que el instalador toque lo que no debe |
| R-MOD | De descarga: con un `hf` falso que registra las llamadas | Que se descargue lo que no se pidió, o dos veces |
| R-TST | Meta: el test puede fallar, y hay una sonda de la propia batería | Que la batería dé verde falso |
| R-OC | De contrato con opencode: el plugin recibe lo que espera | Que la integración se rompa en el punto de uso |
| R-STRATA | De artefacto: el pack y el profile generados son los correctos | Que se sirva el modelo equivocado con el nombre del bueno |
| R-GGUF | De artefacto: el GGUF final existe, es legible y trazable | Que se pierda trabajo caro sin explicación |

### 7.4 La tabla

Las 317 filas, ordenadas por la sección del doc A de la que proceden.

### 3.2 Motores soportados

*Módulo v2*: `crates/ropero-core/src/engine.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/engines.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-CTX-1** | `install engines` sólo ofrece los motores compatibles con el SO y el hardware de la máquina: en Linux no aparece Metal; | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/03-engines.sh:5-17, bin/lib/cmd-install.sh:495-499. | Test `r-ctx_01_install_engines_solo_ofrece_los_motores` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-CTX-2** | `install models` sólo ofrece los modelos de los engines que están instalados: si el catálogo tiene 80 modelos pero sólo hay un engine instalado, se ofrecen los 15 de ese engine. | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:136-145, :602-767. | Test `r-ctx_02_install_models_solo_ofrece_los_modelos` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-CTX-3** 🔶 | Los motores y los modelos no están todos en el mismo grado de madurez, y la especificación no debe fingir lo contrario (+ tabla en el doc A) | `crates/ropero-core/src/engine.rs`<br>*Cambio de arquitectura*: El campo `maturity` es nuevo; el doc A lo exige como estado objetivo. | `ropero install engines` y `ropero doctor` muestran la madurez de cada motor leída del campo `maturity` del manifiesto, y `ropero list --all` marca los modelos con `maturity != proven`. |

### 4.1 Arquitectura y módulos

*Módulo v2*: `crates/ropero-*/` · *Zona de prueba*: `crates/ropero-core/tests/architecture.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-ARQ-1** | El sistema se compone de un dispatcher fino (`bin/ropero`, ~200 líneas) y módulos con carga diferida. | `crates/ropero-*/`<br>*Cambio de arquitectura*: Sustituye al dispatcher de 200 líneas con un `clap` derive. | `ropero-cli` no contiene lógica de negocio: su código mide menos de 400 líneas y todo `match` sobre el subcomando delega en una llamada a otro crate. |
| **R-ARQ-2** | La tabla de ruteo es completa desde el primer día y no se toca: cada fase posterior sólo añade `cmd-*.sh` y su función `rp_cmd_<comando>`. | `crates/ropero-core/tests/architecture.rs` | Test `r-arq_02_tabla_ruteo_completa_desde_primer_dia` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-3** | Un comando no portado responde `aún no portado (fase N)` y sale con 2; | `crates/ropero-core/tests/architecture.rs` <br>*Semilla v1*: bin/ropero:252-263. | Test `r-arq_03_comando_portado_responde_aun_portado_fase` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-4** | `ROPERO_ROOT` se resuelve siguiendo la cadena de symlinks de `$0` con un bucle de `readlink` + `cd/pwd`, sin `readlink -f` (no existe en el bash 3.2 de macOS). | `crates/ropero-core/tests/architecture.rs` <br>*Semilla v1*: bin/ropero:66-99. | Test `r-arq_04_roperoroot_resuelve_siguiendo_cadena_symlinks_con` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-5** | Los módulos son: `00-utils` (utilidades y presentación), `01-env` (entorno), `02-modelconf` (wrapper del catálogo), `03-engines` (cargador de motores), `04-launch` (runtime),… | `crates/ropero-core/tests/architecture.rs` | Test `r-arq_05_los_modulos_son_utils_utilidades_presentacion` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-6** | El catálogo tiene una única fuente de verdad: el parser en Python (`bin/python/modelconf.py`). | `crates/ropero-core/tests/architecture.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:5-7, 176-184. | Test `r-arq_06_catalogo_tiene_una_unica_fuente_verdad` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-7** | El parser es autocontenido: trae su propio lector de TOML (`parse_toml`, `bin/python/modelconf.py:478`) y sólo usa `tomllib` si está disponible (`_tomllib()`, línea 519). | `crates/ropero-core/tests/architecture.rs` | Test `r-arq_07_parser_autocontenido_trae_propio_lector_toml` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-8** | El interprete de Python se prueba, no se supone: se recorren los candidatos y se ejecuta `python -c 'import sys'`; | `crates/ropero-core/tests/architecture.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:60-90. | Test `r-arq_08_interprete_python_prueba_supone_recorren_los` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-9** | El estado runtime vive en `$STATE_DIR/ports/<puerto>`, un fichero de líneas `CLAVE=valor` con `MODEL=`, `ENGINE=`, `PID=`, `PORT=`, `LEVEL=`, `TS=`, `LOG=`, `STAMP=`. | `crates/ropero-core/tests/architecture.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:249-302. | Test `r-arq_09_estado_runtime_vive_statedir_ports_puerto` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-10** | El directorio de modelos tiene dos niveles: `models/<engine>/` (activos) y `models/<engine>/fired/` (retirados). | `crates/ropero-core/tests/architecture.rs` <br>*Semilla v1*: models/_template.toml:5-7. | Test `r-arq_10_directorio_modelos_tiene_dos_niveles_models` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |
| **R-ARQ-11** | El estado de la herramienta se puede reconstruir desde el disco: los puertos gestionados se derivan de `$PORT`, `$CPU_PORT`, los `[model].port` de los TOML activos y los ficheros que… | `crates/ropero-core/tests/architecture.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:304-334. | Test `r-arq_11_estado_herramienta_puede_reconstruir_desde_disco` en `crates/ropero-core/tests/architecture.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-*/`. El test se pone rojo si el comportamiento se altera. |

### 4.2.1 Tablas y reglas

*Módulo v2*: `crates/ropero-core/src/model.rs` · *Zona de prueba*: `crates/ropero-core/tests/model.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-DATA-1** | `[model]` es obligatoria. | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1474-1477. - El id legacy mlx_lm se acepta y se canóniza a mlx-serve con aviso. Evidencia: models/_templat | Test `r-data_01_model_obligatoria` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-2** | `[files]` es obligatoria. | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1479-1494. | Test `r-data_02_files_obligatoria` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-3** | `[download]` es obligatoria salvo que el modelo se fabrique con `[build]`. | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1496-1529, models/_template.toml:47-58. | Test `r-data_03_download_obligatoria_salvo_que_modelo_fabrique` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-4** | `[download.aux.<tipo>]` describe auxiliares; | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: models/_template.toml:60-68. | Test `r-data_04_download_aux_tipo_describe_auxiliares` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-5** | `[build]` declara cómo se fabrica el GGUF: `repo`, `gguf`, `gguf_file`, `quant`, `imatrix`, `imatrix_repo`, `imatrix_file`, `out`, `out_principal`, `from_subdir`, `include`,… | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: models/_template.toml:71-93. | Test `r-data_05_build_declara_como_fabrica_gguf_repo` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-6** | Si un TOML tiene `[download]` y `[build]`, `ropero install models <modelo>` exige `--download` o `--build` para elegir camino; | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: README.md:72-73, models/_template.toml:72-73. | Test `r-data_06_toml_tiene_download_build_ropero_install` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-7** | `[levels]`, `[flags]` y `[options]` declaran lo que el modelo acepta por línea de comandos. | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: models/_template.toml:96-98, bin/lib/cmd-runtime.sh:143, cmd-catalog.sh:272. | Test `r-data_07_levels_flags_options_declaran_que_modelo` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-8** | `[levels].default` tiene que estar en `[levels].list`; | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1531-1555. | Test `r-data_08_levels_default_tiene_que_estar_levels` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-9** | `[limits].min_ram_gb` es un número > 0. | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1557-1565, bin/lib/04-launch.sh:598-617. | Test `r-data_09_limits_minramgb_numero` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-10** | `[args]` es obligatoria para `engine = llama-server` y debe traer `base` como lista. | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1567-1582. | Test `r-data_10_args_obligatoria_para_engine_llama_server` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-11** | Un `${fichero}` usado en `[args]` pero no declarado en `[files]` es error. | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1625-1644. | Test `r-data_11_fichero_usado_args_pero_declarado_files` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-12** | La carpeta del TOML tiene que ser un `engine_dir` conocido (`llama`, `llama-official`, `mlx`, `airllm`, `flm`, `tensorrt`, `vllm`, `strata`). | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1646-1649. | Test `r-data_12_carpeta_del_toml_tiene_que_ser` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.2.2 Montaje del argv

*Módulo v2*: `crates/ropero-core/src/args.rs` · *Zona de prueba*: `crates/ropero-core/tests/args.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-DATA-13** | Un modelo no se ejecuta: sus `[args]` son una lista de *tokens*, y el argv final se monta en este orden fijo: | `crates/ropero-core/tests/args.rs` <br>*Semilla v1*: models/_template.toml:144-146. | Test `r-data_13_modelo_ejecuta_sus_args_son_una` en `crates/ropero-core/tests/args.rs` (prueba unit): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/args.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-14** | Un elemento de la lista = un argumento. | `crates/ropero-core/tests/args.rs` | Test `r-data_14_elemento_lista_argumento` en `crates/ropero-core/tests/args.rs` (prueba unit): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/args.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-15** | `[args.level.<n>]` admite tres formas equivalentes de expresar el nivel: `base` (args literales), `reasoning_effort` (atajo que se traduce a `--chat-template-kwargs <json>`) y… | `crates/ropero-core/tests/args.rs` <br>*Semilla v1*: models/_template.toml:118-122. | Test `r-data_15_args_level_admite_tres_formas_equivalentes` en `crates/ropero-core/tests/args.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/args.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-16** | `[args.flag.<f>]` puede declarar `requires = [<claves de [files]>]`: si el fichero no está en disco, el flag es error. | `crates/ropero-core/tests/args.rs` <br>*Semilla v1*: models/_template.toml:124-129, bin/python/modelconf.py:1608-1623. | Test `r-data_16_args_flag_puede_declarar_requires_claves` en `crates/ropero-core/tests/args.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/args.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.2.3 Placeholders

*Módulo v2*: `crates/ropero-core/src/placeholders.rs` · *Zona de prueba*: `crates/ropero-core/tests/placeholders.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-DATA-17** | Los placeholders válidos en cualquier `base[]` son: `${gguf}`, `${mtp}`, `${mmproj}`, `${chat_template}`, `${ctx}`, `${value}` (sólo en `[args.option]`), `${gguf_dir}`,… | `crates/ropero-core/tests/placeholders.rs` <br>*Semilla v1*: models/_template.toml:131-142. | Test `r-data_17_los_placeholders_validos_cualquier_base_son` en `crates/ropero-core/tests/placeholders.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/placeholders.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-18** | Se pueden sobrescribir sin tocar el TOML con variables de entorno: `ROPERO_<NOMBRE>_CTX` (contexto efectivo), `ROPERO_<NOMBRE>_GGUF` (apunta `${gguf}` a otra cuantización del mismo… | `crates/ropero-core/tests/placeholders.rs` <br>*Semilla v1*: models/_template.toml:148-151, bin/lib/04-launch.sh:895. | Test `r-data_18_pueden_sobrescribir_sin_tocar_toml_con` en `crates/ropero-core/tests/placeholders.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/placeholders.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.2.4 Formatos

*Módulo v2*: `crates/ropero-core/src/format.rs` · *Zona de prueba*: `crates/ropero-core/tests/format.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-DATA-19** | Los cuatro formatos de artefacto son: `gguf` (fichero local), `safetensors` (checkpoint bajo `$SAFETENSORS_DIR`), `q4nx` (el almacén de FLM: `[model].path` es un TAG) y `strata` (el… | `crates/ropero-core/tests/format.rs` <br>*Semilla v1*: models/_template.toml:29-32. | Test `r-data_19_los_cuatro_formatos_artefacto_son_gguf` en `crates/ropero-core/tests/format.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/format.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-20** | Un modelo `format = gguf` sin `[download]` y sin `[build]` es un modelo sólo-local: su GGUF tiene que estar ya en disco. | `crates/ropero-core/tests/format.rs` <br>*Semilla v1*: bin/python/modelconf.py:1521-1529. | Test `r-data_20_modelo_format_gguf_sin_download_sin` en `crates/ropero-core/tests/format.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/format.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-21** 🔶 | `[model].name` DEBE ser igual al nombre del fichero TOML, sin extensión, y ese valor es el alias con el que se arranca y se para el modelo. | `crates/ropero-core/src/format.rs`<br>*Cambio de arquitectura*: Se implementa como § 4.1. | El esquema Rust (`serde`, `deny_unknown_fields`) acepta los 45 TOML del repo sin cambios, y `ropero schema` emite el JSON Schema que los describe. |
| **R-DATA-22** | El vocabulario de lo que un modelo acepta del CLI tiene tres clases, y cada una tiene su sección en el TOML (+ tabla en el doc A) | `crates/ropero-core/tests/format.rs` <br>*Semilla v1*: bin/python/modelconf.py:1487-1529, :1549-1554; models/_template.toml (ejemplo comentado, líneas 230-289). | Test `r-data_22_vocabulario_que_modelo_acepta_del_cli` en `crates/ropero-core/tests/format.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/format.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-DATA-23** | Un modelo puede tener distintos ficheros para la vía download y para la vía build, y cada uno puede estar en un repositorio distinto: el chat template, el MTP, el mmproj (la parte de… | `crates/ropero-core/tests/format.rs` <br>*Semilla v1*: models/_template.toml:205-224; bin/lib/cmd-models.sh:321-336. | Test `r-data_23_modelo_puede_tener_distintos_ficheros_para` en `crates/ropero-core/tests/format.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/format.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.3 El contrato de motor (engine)

*Módulo v2*: `crates/ropero-core/src/engine.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/engines.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-ENG-1** | Un motor es un fichero `bin/lib/engines/<id>.sh` que define ocho hooks obligatorios (+ tabla en el doc A) | `crates/ropero-core/src/engine.rs`<br>*Cambio de arquitectura*: § 3.3.1 y § 4.2-§ 4.3. <br>*Semilla v1*: bin/lib/03-engines.sh:6-25. | Un motor es un manifiesto TOML con los ocho campos del contrato; los hooks que hoy son funciones pasan a ser campos, y `EngineId` es un enum cerrado. |
| **R-ENG-2** | El cargador verifica los ocho y, si falta uno, error nombrándolo: `engine 'x': falta el hook rp_eng_x_label`. | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/03-engines.sh:93-105. | Test `r-eng_02_cargador_verifica_los_ocho_falta_uno` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ENG-3** | Los ids de motor llevan guiones y no se traducen: son identificadores bash legales (`rp_eng_llama-server_label`). | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/03-engines.sh:26-33, 54-64. | Test `r-eng_03_los_ids_motor_llevan_guiones_traducen` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ENG-4** | Añadir un motor = añadir un fichero (y, si tiene modelos, `models/<dir>/`). | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: README.md:179-180. | Test `r-eng_04_anadir_motor_anadir_fichero_tiene_modelos` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ENG-5** | Un motor roto no tumba el install: `_rp_engine_deps_ok` devuelve 1 con la lista de lo que falta y quien llama decide. | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/03-engines.sh:186-195. | Test `r-eng_05_motor_roto_tumba_install_rpenginedepsok_devuelve` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ENG-6** | El probe mira el puerto de ropero, no el interno del motor: lo que tiene que contestar es el endpoint de ropero con su `/health`. | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:143-156. | Test `r-eng_06_probe_mira_puerto_ropero_interno_del` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ENG-7** | Un motor que no tiene auth nativa (`native_auth = no`) arranca sin clave y lo pone el proxy de ropero delante. | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:158-167. | Test `r-eng_07_motor_que_tiene_auth_nativa_nativeauth` en `crates/ropero-runtime/tests/engines.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/engine.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.4 La línea de comandos

*Módulo v2*: `crates/ropero-cli/src/` · *Zona de prueba*: `crates/ropero-cli/tests/cli.rs` · *Tipo*: cli

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-CLI-1** | La superficie pública es: | `crates/ropero-cli/src/`<br>*Cambio de arquitectura*: S-2: la CLI no cambia. <br>*Semilla v1*: bin/ropero:8-11. | La superficie pública es la del doc A, comando por comando; `ropero --help` lista los mismos 19 comandos. |
| **R-CLI-2** | Los comandos son: `use`, `stop`, `status`, `list`, `template`, `args`, `levels`, `path`, `models`, `install` (con `setup`/`engines`/`link`/ `opencode-plugin`), `engines`,… | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/ropero:150-168, tabla de ruteo 160-179. | Test `r-cli_02_los_comandos_son_use_stop_status` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-14** 🔶 | La superficie pública de `install` son dos comandos: `ropero install engines` y `ropero install models`. | `crates/ropero-cli/src/`<br>*Cambio de arquitectura*: El doc A lo marca como objetivo: hoy no existe. | `ropero stats [--watch]` existe, mide CPU/RAM/GPU con `sysinfo` y su salida por defecto es texto legible; `--json` emite una línea JSON por muestra. |
| **R-CLI-15** | `ropero install` a secas ejecuta el proceso completo: preflight del entorno → engines → models, en ese orden. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:117-130 (bare: setup → engines → models), :463-467. | Test `r-cli_15_ropero_install_secas_ejecuta_proceso_completo` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-16** | `ropero install engines <id>...` y `ropero install models <alias>...` aceptan una lista explícita y son el modo desatendido: sin TTY no hay menú, se exige la lista. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:91-96, :464-467; bin/lib/cmd-models.sh:641-645. | Test `r-cli_16_ropero_install_engines_ropero_install_models` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-17** 🔶 | `ropero stats [--watch]` es un comando de estado del sistema: CPU, RAM y GPU, con refresco automático en `--watch`. | `crates/ropero-cli/src/`<br>*Cambio de arquitectura*: Evita duplicar la fuente de verdad de las métricas. | `ropero stats` y `ropero doctor` comparten la misma capa de muestreo, de modo que las cifras de uno y otro no pueden divergir. |
| **R-CLI-3** | Códigos de salida: `0` éxito · `1` error de ejecución · `2` error de uso (incluido «comando aún no portado») · `130` Ctrl-C en primer plano. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/00-utils.sh:170-182, bin/ropero:252-263, bin/lib/04-launch.sh:1159-1161. | Test `r-cli_03_codigos_salida_exito_error_ejecucion_error` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-4** | Un error de uso imprime el error y un puntero a la ayuda (`ropero <cmd> --help → uso de <cmd>`), no un volcado de veinte líneas. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/00-utils.sh:170-182, bin/ropero:236-241. | Test `r-cli_04_error_uso_imprime_error_puntero_ayuda` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-5** | La ayuda general está en línea en el dispatcher: tiene que funcionar aunque sólo existan `00-utils.sh`/`01-env.sh`, e incluso sin `alaja` y sin ANSI. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/ropero:20-22, 199-213. | Test `r-cli_05_ayuda_general_esta_linea_dispatcher_tiene` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-6** | Un comando no conocido que no empieza por `-` se interpreta como arranque directo de modelo (no como error). | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/ropero:265-278. | Test `r-cli_06_comando_conocido_que_empieza_por_interpreta` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-7** | `ropero list` muestra los modelos activos; | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: README.md:37, bin/lib/04-launch.sh:848-852. | Test `r-cli_07_ropero_list_muestra_los_modelos_activos` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-8** | `ropero args <modelo>` imprime el argv final, un argumento por línea. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-catalog.sh:221, 299, 346. | Test `r-cli_08_ropero_args_modelo_imprime_argv_final` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-9** | Los niveles, flags y opciones se validan contra el TOML: un nivel no declarado, un flag no declarado o una opción no declarada son error de uso, con el mensaje que enumera lo que sí… | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-runtime.sh:143, bin/lib/cmd-catalog.sh:258, 272. | Test `r-cli_09_los_niveles_flags_opciones_validan_contra` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-10** | La gramática de los argumentos tras el nombre del modelo: el primer argumento que no es `k=v` es el nivel; | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:269-307. | Test `r-cli_10_gramatica_los_argumentos_tras_nombre_del` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-11** | `ropero status <modelo>` devuelve 0 sirviéndose · 1 no · 2 desconocido. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: README.md:43. | Test `r-cli_11_ropero_status_modelo_devuelve_sirviendose_desconocido` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-12** | `--cpu` sólo existe en Linux (en macOS es error de uso) y sólo funciona con `engine = llama-server` (es `--n-gpu-layers 0`, un flag suyo). | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-runtime.sh:85-92, bin/lib/04-launch.sh:865-870. | Test `r-cli_12_cpu_solo_existe_linux_macos_error` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-CLI-13** | `--port` repetido: gana el último, con aviso. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-runtime.sh:94-113, bin/lib/04-launch.sh:142-145. | Test `r-cli_13_port_repetido_gana_ultimo_con_aviso` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/`. El test se pone rojo si el comportamiento se altera. |

### 4.5.1 Preflight

*Módulo v2*: `crates/ropero-runtime/src/preflight.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/preflight.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-RUN-1** | El preflight ocurre antes de tocar el puerto: un lanzamiento condenado (artefacto ausente, argv inválido, motor sin instalar) no mata lo que está corriendo. | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:857-972. | Test `r-run_01_preflight_ocurre_antes_tocar_puerto_lanzamiento` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que no toca lo que no es suyo lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-2** | El chat template se comprueba en disco antes de arrancar: llama-server, si no lo encuentra, aborta con el muro de sus plantillas built-in y el error real queda enterrado arriba del log. | `crates/ropero-runtime/tests/preflight.rs` | Test `r-run_02_chat_template_comprueba_disco_antes_arrancar` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-3** | El contexto del modelo tiene que llegar al motor: se inyecta `-c <contexto>` (o `--ctx-size`) antes de los args del TOML, para que un TOML con su propio `-c` gane. | `crates/ropero-runtime/tests/preflight.rs` | Test `r-run_03_contexto_del_modelo_tiene_que_llegar` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-4** | Para `engine = llama-server`, ropero añade `--model <ruta-del-gguf>` al argv: el TOML declara flags, no la ruta; | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:928-933, 957-963. | Test `r-run_04_para_engine_llama_server_ropero_anade` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-5** | El override `ROPERO_<NOMBRE>_GGUF` se resuelve en el preflight, con el mismo fichero que acabará en `${gguf}`: si no, el preflight daría por bueno un cuantizado distinto del que se… | `crates/ropero-runtime/src/preflight.rs`<br>*Cambio de arquitectura*: P-6: idempotencia por defecto. <br>*Semilla v1*: bin/lib/04-launch.sh:888-900. | `ropero use` sobre un modelo ya listo y con el mismo sello **no** reinicia nada: el PID antes y después es el mismo, y el comando sale en < 2 s. |

### 4.5.2 Readiness

*Módulo v2*: `crates/ropero-runtime/src/probe.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/probe.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-RUN-6** | La espera es al modelo, no sólo al puerto: `rp_wait_ready` hace poll del *probe* del motor y, cuando responde, exige que `/v1/models` publique el modelo pedido (por nombre, por GGUF o… | `crates/ropero-runtime/tests/probe.rs` | Test `r-run_06_espera_modelo_solo_puerto_rpwaitready_hace` en `crates/ropero-runtime/tests/probe.rs` (prueba integration): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/probe.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-7** | Si el proceso muere antes de escuchar, el fallo es inmediato: no se esperan los N segundos. | `crates/ropero-runtime/tests/probe.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:704-709. | Test `r-run_07_proceso_muere_antes_escuchar_fallo_inmediato` en `crates/ropero-runtime/tests/probe.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/probe.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-8** | El timeout por defecto es `ROPERO_START_TIMEOUT=300 s`, con poll cada 2 s. | `crates/ropero-runtime/tests/probe.rs` <br>*Semilla v1*: bin/lib/01-env.sh:63, bin/lib/04-launch.sh:698, docs/strata.md:67-72. | Test `r-run_08_timeout_por_defecto_roperostarttimeout_300_con` en `crates/ropero-runtime/tests/probe.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/probe.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-9** | `/v1/models` se consulta con la API key: llama-server exige la clave hasta para listar. | `crates/ropero-runtime/tests/probe.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:338-351. | Test `r-run_09_models_consulta_con_api_key_llama` en `crates/ropero-runtime/tests/probe.rs` (prueba integration): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/probe.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-10** | Un arranque fallido mata el proceso que arrancó y limpia el estado; | `crates/ropero-runtime/tests/probe.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:650-682. | Test `r-run_10_arranque_fallido_mata_proceso_que_arranco` en `crates/ropero-runtime/tests/probe.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/probe.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-11** | El PID se captura por fichero temporal, nunca por sustitución de comandos: con `$( )` el proceso lo crea el *subshell*, el engine queda huérfano y `wait` responde «no es un proceso… | `crates/ropero-runtime/tests/probe.rs` | Test `r-run_11_pid_captura_por_fichero_temporal_nunca` en `crates/ropero-runtime/tests/probe.rs` (prueba integration): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/probe.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.5.3 El sello (STAMP)

*Módulo v2*: `crates/ropero-core/src/stamp.rs` · *Zona de prueba*: `crates/ropero-core/tests/stamp.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-RUN-12** | El *sello* de la configuración de arranque es el `cksum` de los args (no `sha256sum`, que macOS no trae). | `crates/ropero-core/tests/stamp.rs` | Test `r-run_12_sello_configuracion_arranque_cksum_los_args` en `crates/ropero-core/tests/stamp.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/stamp.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-13** | Si el modelo ya corre en el puerto destino: - con sello idéntico → no se toca nada («ya está corriendo»), salida 0; | `crates/ropero-core/tests/stamp.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:986-1010. | Test `r-run_13_modelo_corre_puerto_destino_con_sello` en `crates/ropero-core/tests/stamp.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/stamp.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.5.4 Primer plano y Ctrl-C

*Módulo v2*: `crates/ropero-runtime/src/supervisor.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/supervisor.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-RUN-14** | En primer plano, el engine siempre arranca en background con salida al logfile, y lo que se ve en la terminal es el log en vivo coloreado desde el offset del arranque actual (para no… | `crates/ropero-runtime/tests/supervisor.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:1040-1048, 1113-1123. | Test `r-run_14_primer_plano_engine_siempre_arranca_background` en `crates/ropero-runtime/tests/supervisor.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/supervisor.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-15** | La espera en primer plano es un bucle `kill -0`, no un `wait`: bash aplaza el trap INT hasta que el comando esperado acaba, y el comando esperado es el engine, que ignora SIGINT (nohup… | `crates/ropero-runtime/tests/supervisor.rs` | Test `r-run_15_espera_primer_plano_bucle_kill_wait` en `crates/ropero-runtime/tests/supervisor.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/supervisor.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-16** | Ctrl-C sale con 130, y el 130 se reconoce por el código del proceso, no por el trap (que puede no llegar a correr). | `crates/ropero-runtime/tests/supervisor.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:1127-1128, 1159-1161, bin/lib/06-styles.sh:12-20. | Test `r-run_16_ctrl_sale_con_130_130_reconoce` en `crates/ropero-runtime/tests/supervisor.rs` (prueba integration): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/supervisor.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-RUN-17** | El arranque se bloquea por puerto mientras dura: un directorio `.lock` con el PID dentro (`mkdir` es atómico). | `crates/ropero-runtime/src/supervisor.rs`<br>*Cambio de arquitectura*: El lock caduca; no bloquea para siempre. <br>*Semilla v1*: bin/lib/04-launch.sh:216-247. | Un lock de puerto cuyo PID ya no existe se retira solo en el siguiente intento; un lock con PID vivo bloquea con un mensaje que nombra al dueño. |

### 4.6 Puertos, estado y parada

*Módulo v2*: `crates/ropero-runtime/src/state.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/state.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-EST-1** | La precedencia del puerto destino es: `--port N` > `[model].port` pineado > slot CPU (`:9998`, sólo Linux) > slot base del engine. | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:147-182. | Test `r-est_01_precedencia_del_puerto_destino_port_model` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-2** | Los puertos gestionados son la lista canónica de `status`, `stop all` y la idempotencia de `use`: `$PORT` + `$CPU_PORT` (sólo Linux) + los `[model].port` de los TOML activos + los… | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:304-334. | Test `r-est_02_los_puertos_gestionados_son_lista_canonica` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-3** | Para preguntar «¿quién escucha en este puerto?» hay cuatro métodos en cascada: `lsof` → `ss` → `fuser` → `/proc/net/tcp` + `/proc/*/fd` (inodo del socket). | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:52-123. | Test `r-est_03_para_preguntar_quien_escucha_este_puerto` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-4** | La identidad del modelo en un puerto tiene una jerarquía de cuatro niveles: (1) id de `/v1/models` mapeado al catálogo; | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:391-426. | Test `r-est_04_identidad_del_modelo_puerto_tiene_una` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-5** | Un id publicado casa con el modelo si es el nombre, el GGUF (con o sin extensión) o el path del checkpoint (basename o ruta completa). | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:353-372. | Test `r-est_05_publicado_casa_con_modelo_nombre_gguf` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-6** | `stop` mata por puerto, nunca con `pkill -f` global (que mataría los slots paralelos). | `crates/ropero-runtime/src/state.rs`<br>*Cambio de arquitectura*: P-3 aplicado al caso masivo. <br>*Semilla v1*: bin/lib/04-launch.sh:450-500. | `ropero stop` sin argumentos para **todo** sólo detiene los puertos que ropero gestiona (los que tienen fichero de estado), verificado con un puerto ajeno abierto durante el test. |
| **R-EST-7** | El PID del estado se mata aunque el puerto no escuche todavía: un server que está cargando el modelo todavía no escucha, y antes quedaba vivo y se ponía a servir peticiones al terminar. | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:458-465. | Test `r-est_07_pid_del_estado_mata_aunque_puerto` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-8** | Antes de matar, se verifica que el PID no esté reciclado: el SO recicla PIDs y un estado viejo apuntaría a un navegador. | `crates/ropero-runtime/src/state.rs`<br>*Cambio de arquitectura*: P-3: nunca se mata nada ajeno. <br>*Semilla v1*: bin/lib/04-launch.sh:430-448, 474-484. | `ropero stop` sólo mata un proceso si coinciden **pid + start_time + digest del binario**. Un test levanta un proceso ajeno en el puerto, con PID vivo y start_time distinto, y comprueba que sobrevive. |
| **R-EST-9** | La parada es escalonada: `SIGTERM` a todo el árbol → hasta 10 s de margen (mirando el puerto y los PIDs) → `SIGKILL` de red de seguridad → borrado del estado. | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:508-536. | Test `r-est_09_parada_escalonada_sigterm_todo_arbol_hasta` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-10** | `stop` de un puerto libre devuelve 1 (no había nada que parar), no 0. | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:502-506. | Test `r-est_10_stop_puerto_libre_devuelve_habia_nada` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-11** | La API key es única para todos los motores y se resuelve por precedencia: variable de entorno `LLAMA_API_KEY` > `$STATE_DIR/api_key` > generación `sk-ropero-<hex>` (openssl, fallback… | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:184-214. | Test `r-est_11_api_key_unica_para_todos_los` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-12** | El estado es la fuente de verdad y se escribe de forma atómica (tmp + `mv`), conservando las claves no presentes en la llamada. | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/04-launch.sh:268-296. | Test `r-est_12_estado_fuente_verdad_escribe_forma_atomica` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que es atómico lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-13** | `ropero use <modelo>` es idempotente por (modelo, nivel) y emite `ROPERO_PORT=<n>` como última línea de stdout. | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/cmd-runtime.sh:265-292, opencode-plugin/README.md:161-162. | Test `r-est_13_ropero_use_modelo_idempotente_por_modelo` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que es idempotente lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-14** | `ropero stop` sin argumentos para todos los puertos gestionados por ropero; | `crates/ropero-runtime/tests/state.rs` <br>*Semilla v1*: bin/lib/cmd-runtime.sh:314-324 (destino por defecto all), :325-360; bin/lib/04-launch.sh:304-334 (puertos gestionados). | Test `r-est_14_ropero_stop_sin_argumentos_para_todos` en `crates/ropero-runtime/tests/state.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/state.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-EST-15** 🔶 | Detectar los motores ajenos es una pregunta abierta. La capacidad de informar («estos procesos están usando la GPU y no son míos») es distinta de la de pararlos, y ropero hoy no tiene… | `crates/ropero-runtime/src/state.rs`<br>*Cambio de arquitectura*: Contrato de datos § 4.4. | Todo estado escrito en disco lleva `version = 1`; un estado con `version` desconocida se rechaza con un mensaje que nombra la versión leída. |

### 4.7 La puerta única: el endpoint OpenAI-compatible

*Módulo v2*: `crates/ropero-server/src/` · *Zona de prueba*: `crates/ropero-server/tests/` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-API-1** | `bin/server/endpoint.py` es un servidor FastAPI/uvicorn que expone una API OpenAI-compatible con autenticación propia y hace de proxy inverso (o genera en proceso) para los motores que… | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:13-20. | Test `r-api_01_bin_server_endpoint_servidor_fastapi_uvicorn` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-2** | Las rutas son exactamente cinco (+ tabla en el doc A) | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:1326-1429. | Test `r-api_02_las_rutas_son_exactamente_cinco` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-3** | `/health` siempre devuelve 200, incluso con el modelo sin cargar o el backend muerto: el estado real va en `backend.ready` y `backend.error`. | `crates/ropero-server/tests/` | Test `r-api_03_health_siempre_devuelve_200_incluso_con` en `crates/ropero-server/tests/` (prueba integration): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-4** | `/v1/models` devuelve exactamente un modelo, con `id` = el nombre de ropero (no el del backend). | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:1362-1378, bin/server/README.md:17-18. | Test `r-api_04_models_devuelve_exactamente_modelo_con_nombre` en `crates/ropero-server/tests/` (prueba integration): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-5** | El campo `model` del body se reescribe siempre al que el backend conoce: a `model_id` para `tensorrt` y `strata`, y al TAG para `flm` (FLM valida el campo: reescribirlo al id de ropero… | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:886-903, 1532-1540. | Test `r-api_05_campo_model_del_body_reescribe_siempre` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-6** | `/v1/completions` se traduce a `/v1/chat/completions` hacia el backend (FLM no implementa la primera), de modo que los cuatro backends tienen la misma semántica. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:1408-1423. | Test `r-api_06_completions_traduce_chat_completions_hacia_backend` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-7** | Los errores usan el formato OpenAI: `{"error":{"message":…,"type":…,"code":…}}`, con `type` ∈ `invalid_request_error` \| `server_error` \| `upstream_unavailable` y `code` ∈… | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:198-204, tabla en § 8.1 de la nota de análisis. | Test `r-api_07_los_errores_usan_formato_openai_error` en `crates/ropero-server/tests/` (prueba integration): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-8** | Un body que no es un objeto JSON, o un JSON inválido, es 400 `invalid_json` — nunca un 500. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:228-243, 1419-1422. | Test `r-api_08_body_que_objeto_json_json_invalido` en `crates/ropero-server/tests/` (prueba integration): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-9** | El `content` de un mensaje puede ser una lista de partes VLM: `{"type":"text"}` pasa su texto y `{"type":"image_url"}` se sustituye por el literal `"[imagen]"`. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:158-175. | Test `r-api_09_content_mensaje_puede_ser_una_lista` en `crates/ropero-server/tests/` (prueba integration): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-10** | `airllm` no implementa tool calling y el endpoint lo dice con un 400 explícito en vez de fingir soporte (el cliente colgaría). | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:556-563. | Test `r-api_10_airllm_implementa_tool_calling_endpoint_dice` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-11** | La autenticación acepta dos cabeceras: `Authorization: Bearer <clave>` y `X-Api-Key: <clave>`. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:207-225, test/test-endpoint-auth.sh:66-70. | Test `r-api_11_autenticacion_acepta_dos_cabeceras_authorization_bearer` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-12** | Sin clave configurada el endpoint queda abierto, y lo dice en el log. | `crates/ropero-server/src/`<br>*Cambio de arquitectura*: Distinguirlos sería un oráculo para un atacante. <br>*Semilla v1*: bin/server/endpoint.py:213-214, 1542-1544. | El proxy responde 401 con el **mismo cuerpo** si falta la clave y si es incorrecta. |
| **R-API-13** | La clave nunca se reenvía al backend: `authorization` está en el set `HOP_BY_HOP` y se elimina de las cabeceras hacia el upstream. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:127-129, 141, 864-884. | Test `r-api_13_clave_nunca_reenvia_backend_authorization_esta` en `crates/ropero-server/tests/` (prueba integration): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-14** | La clave jamás aparece en un log, y el endpoint loguea siempre a stderr (stdout lo consume ropero). | `crates/ropero-server/src/`<br>*Cambio de arquitectura*: Wound #3 cerrado en el proxy. <br>*Semilla v1*: bin/server/endpoint.py:145-152. | La clave llega al upstream por fichero 0600 o por variable de entorno, nunca por argv; un test lo comprueba sobre el proceso real. |
| **R-API-15** | El sub-proceso del backend siempre escucha en `0.0.0.0.1`: el único proceso expuesto a la red es el proxy. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:44-45, 1226, 1251, 1313. | Test `r-api_15_sub_proceso_del_backend_siempre_escucha` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-16** | El proxy puede vivir sin backend: el puerto externo abre antes de que el modelo esté listo y responde `503 model_loading` en vez de colgarse. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:332-337, 549-554. | Test `r-api_16_proxy_puede_vivir_sin_backend_puerto` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-17** | Un solo worker de uvicorn: con backend proxied el modelo vive en el sub-proceso, y con airllm vive en el proceso del proxy y no se puede compartir entre workers. | `crates/ropero-server/src/`<br>*Cambio de arquitectura*: Se conserva la restricción del doc A. <br>*Semilla v1*: bin/server/endpoint.py:1576-1583. | El servidor usa un único worker: con backend proxied el modelo vive en el subproceso, y con airllm en el proceso del proxy. |
| **R-API-18** | `airllm` serializa las generaciones con un lock global: mueve el modelo entero capa a capa, y dos generaciones a la vez no es «más rápido», es el mismo throughput repartido y el doble… | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:326-329, 531. | Test `r-api_18_airllm_serializa_las_generaciones_con_lock` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-19** | Si el binario del backend rechaza un flag (`No such option`), el proxy lo quita del argv, lo cachea y se relanza. | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:678-717, 773-813. | Test `r-api_19_binario_del_backend_rechaza_flag_such` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-20** | El log del motor y el del proxy comparten fichero (`$LOG_DIR/<modelo>.log`); | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:742-769, bin/lib/04-launch.sh:1035. | Test `r-api_20_log_del_motor_del_proxy_comparten` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-21** | El error de un 4xx/5xx del backend se reenvía tal cual (mismo status, mismos bytes); | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:922-936, 996-1006. | Test `r-api_21_error_4xx_5xx_del_backend_reenvia` en `crates/ropero-server/tests/` (prueba integration): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-22** | En streaming SSE, el proxy emite `data: [DONE]` en el `finally` aunque el upstream se corte a mitad (airllm no tiene servidor que lo haga). | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/server/endpoint.py:628-632. | Test `r-api_22_streaming_sse_proxy_emite_data_done` en `crates/ropero-server/tests/` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-API-23** 🔶 | El proxy no se decide globalmente: lo decide el engine. Cada engine sabe si necesita el proxy, y su hook `launch` monta el comando con el que arranca —con o sin proxy—. | `crates/ropero-server/src/`<br>*Cambio de arquitectura*: Invierte la regla actual (R-API-23 del doc A). <br>*Semilla v1*: bin/lib/engines/airllm.sh:121, strata.sh:278, tensorrt-llm.sh:169 (los tres lanzan endpoint.py); mlx-serve.sh:5 y fastflowlm.sh:29 | La decisión de enrutar por el proxy la toma el motor, no ropero: el manifiesto de cada motor declara `launch.auth` y `launch.proxy`, y el proxy aplica lo que el motor declara. |
| **R-API-24** 🔶 | Qué engines pasan por el proxy hoy (medido) (+ tabla en el doc A) | `crates/ropero-server/src/`<br>*Cambio de arquitectura*: Cierra el wound medido de la clave legible. | El proxy nunca coloca la clave en el argv del motor: se pasa por fichero con modo 0600, y un test comprueba que `/proc/<pid>/cmdline` no contiene la clave. |
| **R-API-25** 🔶 | El endpoint está sin probar. El dueño lo dice explícitamente: «no sé si es sólo AirLLM o TensorRT porque no los he probado». | `crates/ropero-server/src/`<br>*Cambio de arquitectura*: El endpoint actual no tiene rate-limiting. | El proxy aplica `limits` por cliente (peticiones/minuto y peticiones concurrentes) y responde 429 con `Retry-After` al superarlos. |

### 4.8 El enrutado

*Módulo v2*: `crates/ropero-server/src/router.rs` · *Zona de prueba*: `crates/ropero-server/tests/router.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-ROUTE-1** | El enrutado real de ropero es por puerto y por estado en disco, no por reglas de petición. | `crates/ropero-server/tests/router.rs` | Test `r-route_01_enrutado_real_ropero_por_puerto_por` en `crates/ropero-server/tests/router.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/router.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ROUTE-2** | `bin/router.sh` no enruta peticiones HTTP: es un selector semántico de modelo. | `crates/ropero-server/tests/router.rs` <br>*Semilla v1*: bin/router.sh:9-38. | Test `r-route_02_bin_router_enruta_peticiones_http_selector` en `crates/ropero-server/tests/router.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/router.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ROUTE-3** | El fallback de `router.sh` es `minimax/m3.1-flash`, que no corresponde a ningún modelo del catálogo de ropero. | `crates/ropero-server/tests/router.rs` <br>*Semilla v1*: bin/router.sh:31-38 frente a models/. | Test `r-route_03_fallback_router_minimax_flash_que_corresponde` en `crates/ropero-server/tests/router.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/router.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ROUTE-4** | `router.sh` no tiene consumidor en el repo y su cabecera promete consultar `MODELS.md`, un fichero que no existe en este repositorio. | `crates/ropero-server/tests/router.rs` <br>*Semilla v1*: bin/router.sh:6, grep sin resultados. | Test `r-route_04_router_tiene_consumidor_repo_cabecera_promete` en `crates/ropero-server/tests/router.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/router.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-ROUTE-5** | El cuerpo de la petición al clasificador se construye por interpolación sin escapar: una descripción con comillas dobles rompe el JSON y la petición cae al fallback sin mensaje. | `crates/ropero-server/tests/router.rs` <br>*Semilla v1*: bin/router.sh:27-35. | Test `r-route_05_cuerpo_peticion_clasificador_construye_por_interpolacion` en `crates/ropero-server/tests/router.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-server/src/router.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.9.1 La tesis: symlink-only

*Módulo v2*: `crates/ropero-cli/src/install.rs` · *Zona de prueba*: `test/install_test.sh` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-INST-1** | El instalador sólo crea symlinks: no copia, no descarga, no compila, no genera *farms* de entradas, no borra el repo. | `crates/ropero-cli/src/install.rs`<br>*Cambio de arquitectura*: El cambio de bash a binario único no obliga a cambiar la tesis de la instalación. | El instalador sigue siendo symlink-only (S-4): crea enlaces, no copia; y `ropero` pasa a ser un binario único, así que el enlace apunta al binario. |
| **R-INST-2** | La instalación deja exactamente dos symlinks: | `test/install_test.sh` <br>*Semilla v1*: install.sh:5-20, :367-392; idéntico contrato en bin/lib/cmd-install.sh:530-537. | Test `r-inst_02_instalacion_deja_exactamente_dos_symlinks` en `test/install_test.sh` (prueba e2e): comprueba que escribe lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-3** | El repo es la fuente de verdad: `git pull` actualiza el código instalado de inmediato, sin reinstalar. | `test/install_test.sh` <br>*Semilla v1*: install.sh:25-28 (regla R3). | Test `r-inst_03_repo_fuente_verdad_git_pull_actualiza` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-4** | El instalador nunca escribe dentro de `<repo>/`. | `test/install_test.sh` <br>*Semilla v1*: install.sh:45-47 (R11), bin/lib/cmd-install.sh:538-549. | Test `r-inst_04_instalador_nunca_escribe_dentro_repo` en `test/install_test.sh` (prueba e2e): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-5** | Sin red: no hay modo red, ni `--tarball`, ni bootstrap por stdin. | `test/install_test.sh` <br>*Semilla v1*: install.sh:35-36 (R6), :556-557. | Test `r-inst_05_sin_red_hay_modo_red_tarball` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-6** | El PATH del usuario no se modifica nunca: si `~/.local/bin` no está en el PATH, ropero avisa e imprime el `export` que falta, pero no edita ficheros de shell. | `test/install_test.sh` <br>*Semilla v1*: install.sh:408-415 (R13). | Test `r-inst_06_path_del_usuario_modifica_nunca_local` en `test/install_test.sh` (prueba e2e): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-7** | El instalador sólo crea `$BIN_DIR` y `dirname "$DIR"`. | `test/install_test.sh` <br>*Semilla v1*: install.sh:363-364, bin/lib/01-env.sh:75-78. | Test `r-inst_07_instalador_solo_crea_bindir_dirname_dir` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.9.2 Los flags y sus códigos

*Módulo v2*: `crates/ropero-cli/src/install.rs` · *Zona de prueba*: `test/install_test.sh` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-INST-8** | Los flags de `install.sh` son un conjunto cerrado (+ tabla en el doc A) | `test/install_test.sh` <br>*Semilla v1*: install.sh:69-75, :535-562. | Test `r-inst_08_los_flags_install_son_conjunto_cerrado` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-9** | `--local` junto a `--check` → gana `--check`: la comprobación se evalúa antes de migrar o enlazar. | `test/install_test.sh` <br>*Semilla v1*: install.sh:563, :567-570. | Test `r-inst_09_local_junto_check_gana_check_comprobacion` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-10** | El instalador usa sólo `pipefail`: sin `errexit` ni `nounset`, para que un fallo implícito no aborte la instalación a medias sin mensaje. | `test/install_test.sh` <br>*Semilla v1*: install.sh:61-66. | Test `r-inst_10_instalador_usa_solo_pipefail_sin_errexit` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-11** | La salida va a stderr con marcadores `[✓] [!] [✗] [-]`, en español, con recuento final «N ok · M avisos · K fallos»; | `test/install_test.sh` <br>*Semilla v1*: install.sh:41-42, :86-108. | Test `r-inst_11_salida_stderr_con_marcadores_espanol_con` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-12** | El instalador verifica su propio resultado: tras enlar, ejecuta `bash <repo>/bin/ropero --help` y si no sale 0, es un fallo (instalación exit 1), no un aviso. | `test/install_test.sh` <br>*Semilla v1*: install.sh:423-434 (R9). | Test `r-inst_12_instalador_verifica_propio_resultado_tras_enlar` en `test/install_test.sh` (prueba e2e): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.9.3 La migración del layout viejo y el rescate

*Módulo v2*: `crates/ropero-cli/src/install.rs` · *Zona de prueba*: `test/install_test.sh` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-INST-13** | `fase_migrar` decide por el estado de `$DIR` (+ tabla en el doc A) | `test/install_test.sh` <br>*Semilla v1*: install.sh:275-356. | Test `r-inst_13_fasemigrar_decide_por_estado_dir` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-14** | Los hooks de rescate se ejecutan antes de borrar nada: `_rescatar_engines` (por cada binario de motor que sea symlink dentro del farm viejo) y `_rescatar_venv` (el venv del gguffer a… | `test/install_test.sh` <br>*Semilla v1*: install.sh:191-258. | Test `r-inst_14_los_hooks_rescate_ejecutan_antes_borrar` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-15** | El rescate de un motor no lo hace si `command -v <nombre>` ya resuelve: no debe sombrear un binario de brew o del sistema que el usuario ya tiene funcionando. | `test/install_test.sh` <br>*Semilla v1*: install.sh:196-214 (F2). | Test `r-inst_15_rescate_motor_hace_command_nombre_resuelve` en `test/install_test.sh` (prueba e2e): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-16** | El rescate no pisa nada: si el destino ya existe (fichero o symlink), o el enlace está colgante o no ejecutable, no se rescata. | `test/install_test.sh` <br>*Semilla v1*: install.sh:215-223. | Test `r-inst_16_rescate_pisa_nada_destino_existe_fichero` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-17** | El enlace rescatado apunta al binario real (symlinks resueltos), para que no se rompa si `$BIN_DIR` se vacía y se rellena. | `test/install_test.sh` <br>*Semilla v1*: install.sh:219-228. | Test `r-inst_17_enlace_rescatado_apunta_binario_real_symlinks` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-18** | El venv rescatado que colisiona con uno existente se aparta como `venv.farm.$$`: no se pisa nada. | `test/install_test.sh` <br>*Semilla v1*: install.sh:236-258. | Test `r-inst_18_venv_rescatado_que_colisiona_con_uno` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.9.4 Lo que el instalador nunca toca

*Módulo v2*: `crates/ropero-cli/src/install.rs` · *Zona de prueba*: `test/install_test.sh` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-INST-19** | La lista de intocables: el repo; | `test/install_test.sh` <br>*Semilla v1*: install.sh:369-373, :404-406, :352, :324-326, bin/lib/cmd-install.sh:601-604. | Test `r-inst_19_lista_intocables_repo` en `test/install_test.sh` (prueba e2e): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-20** | No existe `--uninstall`: la única desinstalación es borrar los dos symlinks a mano. | `test/install_test.sh` <br>*Semilla v1*: grep de «uninstall» sobre install.sh → 0 resultados. | Test `r-inst_20_existe_uninstall_unica_desinstalacion_borrar_los` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.9.5 `--check`

*Módulo v2*: `crates/ropero-cli/src/install.rs` · *Zona de prueba*: `test/install_test.sh` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-INST-21** | `--check` es read-only y hace cinco cosas: (1) el entrypoint debe ser symlink y resolver a un ejecutable existente; | `test/install_test.sh` <br>*Semilla v1*: install.sh:445-530, :501-512. | Test `r-inst_21_check_read_only_hace_cinco_cosas` en `test/install_test.sh` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/install.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.9.6 Los motores: tres estados que se confunden

*Módulo v2*: `crates/ropero-runtime/src/preflight.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/preflight.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-INST-22** | Hay tres estados distintos de un motor, y el doctor los distingue (+ tabla en el doc A) | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:506-515, :593-613; bin/lib/cmd-doctor.sh:286-301. | Test `r-inst_22_hay_tres_estados_distintos_motor_doctor` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-23** | Un motor cuyo instalador dice OK pero que no queda utilizable es un fallo, no un éxito maquillado (`rc=1`). | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:506-515. | Test `r-inst_23_motor_cuyo_instalador_dice_pero_que` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-24** | La precedencia de `resolve` es PATH → `$BIN_DIR` → rutas largas, en los ocho motores. | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/engines/llama-server.sh:314-327, llama-server-official.sh:228-241, vllm.sh:73-84, mlx-serve.sh:76-138, tensorrt-llm.sh:103 | Test `r-inst_24_precedencia_resolve_path_bindir_rutas_largas` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-25** | Los nombres de los enlaces son el `basename` del binario resuelto: `llama-server`, `llama-server-official`, `vllm`, `mlx-serve`, `trtllm-serve`, `flm`, `strata`. | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:564-565, :599-600. | Test `r-inst_25_los_nombres_los_enlaces_son_basename` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-26** | `ropero install engines` exige TTY: sin TTY es error de uso (exit 2), porque un menú que se traga un EOF acabaría eligiendo por su cuenta. | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:375-448, :464-467, :495-505. | Test `r-inst_26_ropero_install_engines_exige_tty_sin` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-INST-27** | Un engine roto no tumba el install: `_rp_engine_deps_ok` devuelve 1 con la lista de dependencias que faltan, sin `exit`. | `crates/ropero-runtime/tests/preflight.rs` <br>*Semilla v1*: bin/lib/03-engines.sh:171-184. | Test `r-inst_27_engine_roto_tumba_install_rpenginedepsok_devuelve` en `crates/ropero-runtime/tests/preflight.rs` (prueba integration): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/preflight.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.10.1 El TOML es el dato

*Módulo v2*: `crates/ropero-core/src/model.rs` · *Zona de prueba*: `crates/ropero-core/tests/model.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-MOD-1** | El catálogo son los TOML: la raíz es `<repo>/models`; | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:630-658, README.md:58-60. | Test `r-mod_01_catalogo_son_los_toml_raiz_repo` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-2** | `bin/python/modelconf.py` es la fuente de verdad del esquema (el shell no lo duplica). | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:4-6. | Test `r-mod_02_bin_python_modelconf_fuente_verdad_del` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-3** | El shell consume el catálogo mediante un único dump por proceso (idempotente), con separador `0x1F` y 31 campos por registro, y un guard de versión de formato: si la cabecera no es… | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:6-7, :33-34, :159-185. | Test `r-mod_03_shell_consume_catalogo_mediante_unico_dump` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-4** | Un TOML roto no tumba el catálogo: se avisa por stderr y se salta; | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:1176-1186. | Test `r-mod_04_toml_roto_tumba_catalogo_avisa_por` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-5** | Nombres duplicados entre activo y `fired` no son error: las dos filas se quedan y la primera gana (los activos van antes). | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/python/modelconf.py:137-140, bin/lib/02-modelconf.sh:220-229. | Test `r-mod_05_nombres_duplicados_entre_activo_fired_son` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-6** | El formato declara dónde vive el artefacto (+ tabla en el doc A) | `crates/ropero-core/tests/model.rs` <br>*Semilla v1*: bin/lib/cmd-catalog.sh:389-410. | Test `r-mod_06_formato_declara_donde_vive_artefacto` en `crates/ropero-core/tests/model.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/model.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.10.2 Los alias

*Módulo v2*: `crates/ropero-core/src/catalog.rs` · *Zona de prueba*: `crates/ropero-core/tests/catalog.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-MOD-7** | El alias se declara en `[model].alias`; | `crates/ropero-core/tests/catalog.rs` <br>*Semilla v1*: models/_template.toml (sección [aliases]), bin/python/modelconf.py:1549-1554. | Test `r-mod_07_alias_declara_model_alias` en `crates/ropero-core/tests/catalog.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/catalog.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-8** | En memoria, un alias se materializa como una fila extra en las arrays paralelas, detrás de las reales, copiando sus campos con `MODEL_ISALIAS=1` y `MODEL_REAL=<real>`. | `crates/ropero-core/tests/catalog.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:36-38, :193-214. | Test `r-mod_08_memoria_alias_materializa_como_una_fila` en `crates/ropero-core/tests/catalog.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/catalog.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-9** | El choque de alias es un error que nombra el conflicto; | `crates/ropero-core/tests/catalog.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:202-206. | Test `r-mod_09_choque_alias_error_que_nombra_conflicto` en `crates/ropero-core/tests/catalog.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/catalog.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-10** | Un alias responde igual que el nombre: `rp_model_exists` acepta ambos, `rp_model_argv` traduce alias → real, y `ropero path` usa siempre el nombre real para el directorio. | `crates/ropero-core/tests/catalog.rs` <br>*Semilla v1*: bin/lib/02-modelconf.sh:231-235, :284-288; bin/lib/cmd-catalog.sh:364-372. | Test `r-mod_10_alias_responde_igual_que_nombre_rpmodelexists` en `crates/ropero-core/tests/catalog.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/src/catalog.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.10.3 El contrato de stdout

*Módulo v2*: `crates/ropero-cli/src/run.rs` · *Zona de prueba*: `crates/ropero-cli/tests/cli.rs` · *Tipo*: cli

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-MOD-11** | stdout = datos; | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-catalog.sh:11-15, :24-27, :91-92; bin/lib/00-utils.sh:5-8. | Test `r-mod_11_stdout_datos` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/run.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-12** | `ropero levels` no imprime cabeceras, porque cualquier otra cosa por stdout rompe el autocompletado; | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-catalog.sh:284-287. | Test `r-mod_12_ropero_levels_imprime_cabeceras_porque_cualquier` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/run.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-13** | `ropero args` imprime un argumento por línea y valida contra el TOML: un nivel, flag u opción no declarado es exit 2. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-catalog.sh:221-282. | Test `r-mod_13_ropero_args_imprime_argumento_por_linea` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/run.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.10.4 La descarga: `hf` y sólo `hf`

*Módulo v2*: `crates/ropero-download/src/` · *Zona de prueba*: `crates/ropero-download/tests/download.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-MOD-14** | El único downloader es `hf` (HuggingFace CLI): no hay `curl` de reserva, porque `ROPERO_DOWNLOADER` era «una vía de fuga de los tests». | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:9-13. | Test `r-mod_14_unico_downloader_huggingface_cli_hay_curl` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-15** | La descarga de un GGUF usa staging propio: `mktemp -d "$dest_dir/.rp-dl.XXXXXX"` — dentro del destino y en el mismo filesystem, para que el `mv` sea atómico; | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:95-99, :158-162. | Test `r-mod_15_descarga_gguf_usa_staging_propio_mktemp` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que es atómico lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-16** | La secuencia de una descarga de GGUF es exactamente: (1) si el nombre final existe y no está vacío → «ya descargado», return 0; | `crates/ropero-download/src/`<br>*Cambio de arquitectura*: El detalle del fichero vacío cuesta una descarga de 40 GB si se ignora. <br>*Semilla v1*: bin/lib/05-download.sh:123-199. | La secuencia de descarga es exactamente la del doc A, y un fichero de **0 bytes** cuenta como no descargado. |
| **R-MOD-17** | La descarga de un checkpoint safetensors NO usa staging, a propósito: `hf` es reanudable con `--local-dir` fijo, y un staging mataría precisamente esa reanudación (una descarga de 12… | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:203-209. | Test `r-mod_17_descarga_checkpoint_safetensors_usa_staging_proposito` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-18** | La verificación de una descarga es por contenido real, no por marcadores: - GGUF: fichero final presente y no vacío (`[[ -f && -s ]]`). | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:20-22, :38-59, :126, :228-235. | Test `r-mod_18_verificacion_una_descarga_por_contenido_real` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-19** | Aunque `hf` salga con 0, se re-verifica; | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:285-296. | Test `r-mod_19_aunque_salga_con_verifica` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-20** | No hay verificación por hash ni por checksum en ninguna parte. | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:91, :151; bin/lib/cmd-models.sh:478, :487. | Test `r-mod_20_hay_verificacion_por_hash_por_checksum` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-21** | El estado de una descarga se reconstruye desde el disco: GGUF → presencia del fichero final; | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:21-22, :250-256, :286-298. | Test `r-mod_21_estado_una_descarga_reconstruye_desde_disco` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-22** | La salida de `hf` va a stderr, para que un install con selección no «parezca escupir basura en vez de un modelo». | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/05-download.sh:277-279. | Test `r-mod_22_salida_stderr_para_que_install_con` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-23** | Los auxiliares `[download.aux.*]` se bajan al mismo directorio con filtro `use=run` (los que el arranque referencia: `${mtp}`, `${mmproj}`, `${chat_template}`). | `crates/ropero-download/tests/download.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:321-336, :515-517. | Test `r-mod_23_los_auxiliares_download_aux_bajan_mismo` en `crates/ropero-download/tests/download.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/`. El test se pone rojo si el comportamiento se altera. |

### 4.10.5 Reutilización entre modelos

*Módulo v2*: `crates/ropero-download/src/reuse.rs` · *Zona de prueba*: `crates/ropero-download/tests/reuse.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-MOD-24** | Si dos TOMLs piden el mismo repo + el mismo nombre de fichero, el segundo modelo se enlaza al directorio del primero en vez de re-descargar. | `crates/ropero-download/tests/reuse.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:338-435, :346-350. | Test `r-mod_24_dos_tomls_piden_mismo_repo_mismo` en `crates/ropero-download/tests/reuse.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/reuse.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-25** | Un symlink ya hecho a mano se respeta (no se toca); | `crates/ropero-download/tests/reuse.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:396-426. | Test `r-mod_25_symlink_hecho_mano_respeta_toca` en `crates/ropero-download/tests/reuse.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/reuse.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.10.6 La decisión de descargar

*Módulo v2*: `crates/ropero-download/src/plan.rs` · *Zona de prueba*: `crates/ropero-download/tests/plan.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-MOD-26** | `ropero models` exige TTY sin selección explícita; | `crates/ropero-download/tests/plan.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:641-662, :688-690, :730-737. | Test `r-mod_26_ropero_models_exige_tty_sin_seleccion` en `crates/ropero-download/tests/plan.rs` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/plan.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-27** | Elegir un modelo ES consentir su descarga: dentro del proceso se fija `RP_YES=1`, y la pregunta de tamaño no se repite. | `crates/ropero-download/tests/plan.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:19-20, :500. | Test `r-mod_27_elegir_modelo_consentir_descarga_dentro_del` en `crates/ropero-download/tests/plan.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/plan.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-29** | Un modelo puede declarar las dos vías (`[download]` y `[build]`) a la vez; | `crates/ropero-download/tests/plan.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:696-723, :730-737. | Test `r-mod_29_modelo_puede_declarar_las_dos_vias` en `crates/ropero-download/tests/plan.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/plan.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-30** | En la vía build, si el repo de safetensors ya viene cuantizado, la fase de cuantización se salta: se genera el GGUF directamente. | `crates/ropero-download/tests/plan.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:439-483 (plan de cuantización). | Test `r-mod_30_via_build_repo_safetensors_viene_cuantizado` en `crates/ropero-download/tests/plan.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/plan.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-31** | `ropero gguffer` es el equivalente manual de `ropero install models --prio build`: baja el repo de safetensors, crea el GGUF y lo cuantiza. | `crates/ropero-download/tests/plan.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1-15, :1271-1327. | Test `r-mod_31_ropero_gguffer_equivalente_manual_ropero_install` en `crates/ropero-download/tests/plan.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/plan.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-MOD-28** | Los motores con almacén propio (FLM, Strata) deciden ellos: si el engine define el hook opcional `install_model`, manda él y se salta el camino download/build. | `crates/ropero-download/tests/plan.rs` <br>*Semilla v1*: bin/lib/cmd-models.sh:437-455, bin/lib/engines/fastflowlm.sh:188-259, bin/lib/engines/strata.sh:188-237. | Test `r-mod_28_los_motores_con_almacen_propio_flm` en `crates/ropero-download/tests/plan.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-download/src/plan.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.10.7 El contrato de `--prio`

Un modelo **puede tener `[download]`, `[build]` o los dos**: los dos es un estado
válido del TOML (§ 4.1.1). La elección del camino ocurre en
`ropero install models`, y `--prio` es la forma de **no preguntar**.

Este apartado es el **contrato normativo** de `--prio`: el doc A (R-MOD-29) y
el doc del CLI (§ 9.3) remiten aquí.

**Valor**: `--prio=download` o `--prio=build`. Sólo esos dos valores. Cualquier
otro valor (p. ej. `--prio=fast`) es **error de uso (exit 2)** y el mensaje
**incluye el valor recibido**.

**Ámbito**: `--prio` puede ser **global** (toda la ejecución de ese
`ropero install models`) **o por modelo**.

**Sintaxis global**: el `--prio` global va **antes** de la lista de modelos:

```
ropero install models --prio=build nemotron groot
```

**Sintaxis por modelo**: el `--prio` va **detrás** del modelo al que se aplica
y su ámbito **termina en el siguiente modelo**. El ejemplo canónico es:

```
ropero install models nemotron groot --prio=build qwen --prio=download
```

que significa exactamente:

| Modelo | `--prio` | Consecuencia |
|---|---|---|
| `nemotron` | (ninguno) | se procesa **con lo que tenga**: si declara las dos vías, se pregunta al usuario; si sólo tiene una, se usa esa |
| `groot` | `build` | se usa su camino `build` |
| `qwen` | `download` | se usa su camino `download` |

**Precedencia**: si hay un `--prio` **global** y otro **por modelo**, **manda
el por modelo** para ese modelo; el global sólo aplica a los modelos que no
tengan `--prio` propio. En

```
ropero install models --prio=build nemotron groot --prio=download
```

`nemotron` va por `build` (el global) y `groot` por `download` (el suyo).

**Cuándo se ignora el `--prio`** (no es un error, no aborta la ejecución):

- Si el modelo **no tiene** ese camino (p. ej. `--prio=build` sobre un modelo
  que sólo declara `[download]`), el `--prio` **se ignora**.
- Si el modelo **sólo tiene un camino** y se indica ese `--prio`, **se ignora**:
  ya es su comportamiento por defecto, así que no cambia nada.

Un `--prio` ignorado **no** es un error y **no** aborta: los **demás** modelos
de la lista se instalan igual y el exit code es el del resultado de la
instalación completa.

**Sin `--prio` y con las dos vías**: se **pregunta al usuario** qué camino
tomar. Si no hay TTY, es un **error de uso (exit 2)** que corresponde a los
comandos, no al validador, y el mensaje dice **qué hacer**: pasar `--prio`, o
dar una lista de modelos en la que ninguno tenga las dos vías.

**Lo que la v1 hacía y lo que la v2 cambia** (para que F5 no lo copie tal cual):

- El `--prio` de la v1 era **sólo global**: un único valor para toda la
  invocación (`ropero-code/bin/lib/cmd-models.sh:603`, `:623-628`). La v2
  añade el ámbito **por modelo** y la **precedencia** entre ambos.
- La v1 validaba el valor de `--prio` **al parsear**
  (`ropero-code/bin/lib/cmd-models.sh:623-628`), igual que la v2, pero su
  mensaje no incluía el valor recibido.
- La v1 tenía además los flags `--download`/`--build`, que forzaban un camino
  y sí eran **error de uso** si contradecían al TOML
  (`ropero-code/bin/lib/cmd-models.sh:730-737`): son **otra cosa** que
  `--prio`, que se ignora.
- La rama de «la prioridad no está disponible» de la v1
  (`ropero-code/bin/lib/cmd-models.sh:711`) **no llegaba a ejecutarse**: con un
  valor válido, un modelo de un solo camino caía siempre en ese camino
  (`ropero-code/bin/lib/cmd-models.sh:725-728`). La v2 lo dice sin ambigüedad:
  **se ignora**.

> **Contrato pendiente de implementación.** La v2 actual no tiene todavía
> `ropero install models` (llega en F5/T-25). Este apartado fija el
> comportamiento que tendrá; lo verifica
> `r-mod_29_modelo_puede_declarar_las_dos_vias` en
> `crates/ropero-download/tests/plan.rs`.

### 4.11.1 Qué es

*Módulo v2*: `test/` · *Zona de prueba*: `test/` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-TST-1** | La batería es una especificación ejecutable en dos niveles: | `test/` <br>*Semilla v1*: recuento de call-sites en test/test-*.sh; el desglose por fichero está en § 5.2. | Test `r-tst_01_bateria_una_especificacion_ejecutable_dos_niveles` en `test/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-2** | El exit code de los wrappers sólo distingue 0 (las 5 baterías ok) de 1 (alguna falló) de 2 (no se puede ni empezar). | `test/` <br>*Semilla v1*: test/lib/test-common.sh:499-565, docs/juez.md. | Test `r-tst_02_exit_code_los_wrappers_solo_distingue` en `test/` (prueba e2e): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `test/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-3** | El contenido de las respuestas del modelo no se evalúa automáticamente en ningún test: no hay ni un `assert` sobre la calidad de la respuesta, sólo sobre el artefacto (JSON no vacío). | `test/` <br>*Semilla v1*: test/lib/test-common.sh:401-437. | Test `r-tst_03_contenido_las_respuestas_del_modelo_evalua` en `test/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/`. El test se pone rojo si el comportamiento se altera. |

### 4.11.2 El runner común

*Módulo v2*: `test/lib/` · *Zona de prueba*: `test/lib/` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-TST-4** | El runner resuelve sus rutas desde la ubicación del propio script, no desde el `cwd` ni desde `ROPERO_ROOT`: el test debe ser lanzable desde cualquier sitio. | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:37-38. | Test `r-tst_04_runner_resuelve_sus_rutas_desde_ubicacion` en `test/lib/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-5** | El runner monta un provider hermético propio (`ropero-local`, openai-compatible) apuntando a `127.0.0.1:<puerto>/v1` y no lee ni toca la config global de opencode del usuario; | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:20-27, :147-183, :288-297. | Test `r-tst_05_runner_monta_provider_hermetico_propio_ropero` en `test/lib/` (prueba e2e): comprueba que devuelve lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-6** | La API key se resuelve después del arranque del modelo, en este orden: `LLAMA_API_KEY` > `$STATE_DIR/api_key` > `OPENAI_API_KEY` > `sk-local-dev-key`. | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:85-100, :83, :181, :459. | Test `r-tst_06_api_key_resuelve_despues_del_arranque` en `test/lib/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-7** | El contexto y el `output` del modelo bajo prueba se derivan del `context` del TOML: `output = context − 1024`, saturado a `[4096, 65536]`, con contexto por defecto 32768. | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:114-128 (medido: groot → context 65536 / output 64512). | Test `r-tst_07_contexto_output_del_modelo_bajo_prueba` en `test/lib/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-8** | El runner hace probe del engine con `GET /v1/models` + `Authorization: Bearer <key>` antes de lanzar opencode, y aborta con mensaje claro si no responde; | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:189-203, :394-398. | Test `r-tst_08_runner_hace_probe_del_engine_con` en `test/lib/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-9** | El runner invoca exactamente `opencode run <prompt> --agent build --model ropero-local/<modelo> --format json --standalone`, con stdout al JSON y stderr a su propio fichero, nunca… | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:401-403 (argv verificado en ejecución). | Test `r-tst_09_runner_invoca_exactamente_opencode_run_prompt` en `test/lib/` (prueba e2e): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-10** | Un fallo en una batería no aborta las demás: se anota y se sigue; | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:541-565, :213, :216. | Test `r-tst_10_fallo_una_bateria_aborta_las_demas` en `test/lib/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-11** | El runner produce un `recopilacion-completa.md` con el CONTENIDO de cada artefacto (log del engine, JSON, sesión exportada, stderr), no sólo sus rutas. | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:312-332, :445-492. | Test `r-tst_11_runner_produce_recopilacion_completa_con_contenido` en `test/lib/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-12** | La negativa a empezar es exit 2, no 1: falta el modelo, falta `jq`/`opencode`/`ropero`, no se puede crear el directorio de salida o no se puede preparar el entorno aislado. | `test/lib/` <br>*Semilla v1*: test/lib/test-common.sh:501-528, :530-539. | Test `r-tst_12_negativa_empezar_exit_falta_modelo_falta` en `test/lib/` (prueba e2e): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `test/lib/`. El test se pone rojo si el comportamiento se altera. |

### 4.11.3 Los dobles

*Módulo v2*: `test/fakes/` · *Zona de prueba*: `test/fakes/` · *Tipo*: e2e

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-TST-13** | Los dobles de `test/fakes/` no son un adorno: son la forma de verificar el contrato sin GPU, sin red y sin tocar el estado del usuario. | `test/fakes/` <br>*Semilla v1*: test/fakes/{pkill,curl,ropero,opencode}. | Test `r-tst_13_los_dobles_test_fakes_son_adorno` en `test/fakes/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/fakes/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-14** | El doble `llama-quantize` impone la restricción real del motor: re-cuantizar falla con el mensaje literal `requantizing from type X is disabled`. | `test/fakes/` <br>*Semilla v1*: test/fakes/gguffer/llama-quantize:121-129. | Test `r-tst_14_doble_llama_quantize_impone_restriccion_real` en `test/fakes/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/fakes/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-15** | El GGUF sintético (`gguf_fake.py`) permite que el resultado del cuantizador falso sea verificable con el `ggufcheck` real, no con otro doble: eso es lo que da credibilidad a la… | `test/fakes/` <br>*Semilla v1*: test/fakes/gguffer/gguf_fake.py, test/test-gguffer.sh:178. | Test `r-tst_15_gguf_sintetico_gguffake_permite_que_resultado` en `test/fakes/` (prueba e2e): comprueba que se cumple lo que el requisito afirma, contra la implementación de `test/fakes/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-16** | La suite offline, ejecutada con `HOME` temporal y los fakes en el PATH, corre 5/5 baterías ok en ~2 s, exit 0, sin GPU, sin red y sin matar nada. | `test/fakes/` <br>*Semilla v1*: test/test-groot.sh + test/fakes/ en el PATH. | Test `r-tst_16_suite_offline_ejecutada_con_home_temporal` en `test/fakes/` (prueba e2e): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `test/fakes/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-17** | La baja de procesos es una lista cerrada y explícita: `llama-server`, `llama-server-official`, `opencode`, `mcode`, `claude`. | `test/fakes/` <br>*Semilla v1*: test/lib/test-common.sh:206-219, test/fakes/pkill:6-10. | Test `r-tst_17_baja_procesos_una_lista_cerrada_explicita` en `test/fakes/` (prueba e2e): comprueba que devuelve lo que el requisito afirma, contra la implementación de `test/fakes/`. El test se pone rojo si el comportamiento se altera. |

### 4.11.4 Lo que la batería NO garantiza — tres heridas medidas

*Módulo v2*: `crates/ropero-core/tests/` · *Zona de prueba*: `crates/ropero-server/tests/` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-TST-18** | 🔴 El guard de auth del endpoint pasa en verde con la auth rota. Verificado empíricamente: al romper `authed()` en una copia para que devolviera `True` siempre, el bloque python reportó… | `crates/ropero-server/tests/` <br>*Semilla v1*: test/test-endpoint-auth.sh:31-106, :204-208. | Test `r-tst_18_guard_auth_del_endpoint_pasa_verde` en `crates/ropero-server/tests/` (prueba unit): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-19** | 🟠 El test de gguffer no es hermético: depende del `transformers` del host, porque hay un pre-check real antes de invocar el conversor falso. | `crates/ropero-core/tests/`<br>*Cambio de arquitectura*: Wound #1: verde falso. Es el motivo de reescribir el endpoint en Rust. <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1961-1980, test/test-gguffer.sh:16-17. | El test de auth llama a la **misma función** que usa el servidor para autenticar y afirma sobre el resultado; no captura el `rc` de un subproceso. Sacar la comprobación de la clave hace fallar el test. |
| **R-TST-20** | 🟠 No hay runner de CI ni comando documentado que ejecute la suite completa: hay que lanzar 16 scripts a mano. | `crates/ropero-server/tests/` <br>*Semilla v1*: README.md:174, :192, CHANGELOG.md:205. | Test `r-tst_20_hay_runner_comando_documentado_que_ejecute` en `crates/ropero-server/tests/` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-21** | 🟠 Los logs de los dobles se escriben pero nunca se assertan. Los cinco dobles raíz registran su argv en `$HOME/.fake-*.log` y ningún test los lee (grep de `FAKE_LOG`/`.fake-` → 0… | `crates/ropero-server/tests/` <br>*Semilla v1*: test/fakes/README.md:50-51. | Test `r-tst_21_los_logs_los_dobles_escriben_pero` en `crates/ropero-server/tests/` (prueba unit): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-22** | 🟠 La resolución de niveles (`[levels]`) no tiene un solo test. El catálogo define niveles por modelo y `rp_model_argv <modelo> [nivel]` los resuelve, pero ningún test verifica que el… | `crates/ropero-core/tests/`<br>*Cambio de arquitectura*: Wound #2: la resolución de niveles no tenía un solo test. <br>*Semilla v1*: models/llama/groot.toml ([levels]), bin/lib/02-modelconf.sh:269-312. | Existe un test que verifica que el nivel pedido llega al argv, que un nivel no declarado falla con exit 2, y que el nivel por defecto se aplica cuando no se pide ninguno. |
| **R-TST-23** | 🟠 21 ficheros de código no son tocados por ningún test, entre ellos `cmd-catalog.sh`, `cmd-doctor.sh`, `cmd-install.sh`, `cmd-runtime.sh`, `cmd-gguffer.sh`, `router.sh`, los seis… | `crates/ropero-core/tests/`<br>*Cambio de arquitectura*: Wound #3: la clave en el argv, legible por cualquier usuario del host. <br>*Semilla v1*: grep sobre test/*.sh → 0 coincidencias. | Existe un test que verifica que la clave **no** aparece en el argv del motor, examinando `/proc/<pid>/cmdline` del proceso real. |
| **R-TST-24** | 🟡 El CHANGELOG describe un contrato caducado que el test contradice: afirma que el engine FLM le pasa `--api-key` al endpoint y «se niega a arrancar sin `LLAMA_API_KEY`»; | `crates/ropero-server/tests/` <br>*Semilla v1*: CHANGELOG.md:98-102 vs. test/test-endpoint-auth.sh:116-122, :199-202. | Test `r-tst_24_changelog_describe_contrato_caducado_que_test` en `crates/ropero-server/tests/` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-25** | 🟡 Dos scripts dependent del entorno más de lo que parece: `test-flm-decision.sh` y `test-models-symlink.sh` abortan con `LLAMA_API_KEY: unbound variable` si no está exportada (por `set… | `crates/ropero-server/tests/` <br>*Semilla v1*: bin/lib/01-env.sh:60, test/lib/test-common.sh:522-528. | Test `r-tst_25_dos_scripts_dependent_del_entorno_mas` en `crates/ropero-server/tests/` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-26** | 🟡 La compatibilidad con bash 3.2 es una declaración, no una verificación: el README y las cabeceras de los 11 scripts la afirman, pero la batería no ejecuta `bash 3.2` en ningún sitio… | `crates/ropero-server/tests/` <br>*Semilla v1*: README.md:200-202, test/bench-motor.sh:411. | Test `r-tst_26_compatibilidad_con_bash_una_declaracion_una` en `crates/ropero-server/tests/` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-27** | 🟡 `shellcheck` no está en la batería, pese a que el README lo documenta como requisito de desarrollo. | `crates/ropero-server/tests/` <br>*Semilla v1*: README.md:194. | Test `r-tst_27_shellcheck_esta_bateria_pese_que_readme` en `crates/ropero-server/tests/` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |
| **R-TST-28** | 🟡 Dos scripts devuelven códigos no estándar: `test-strata.sh` sale con el número de fallos (3 fallos → exit 3, no 1) y `test-endpoint-auth.sh` sale 0/1 pero sin propagar el fallo del… | `crates/ropero-server/tests/` <br>*Semilla v1*: test/test-strata.sh:218-223, test/test-endpoint-auth.sh:204-208. | Test `r-tst_28_dos_scripts_devuelven_codigos_estandar_test` en `crates/ropero-server/tests/` (prueba unit): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-core/tests/`. El test se pone rojo si el comportamiento se altera. |

### 4.12 La integración con opencode

*Módulo v2*: `opencode-plugin/` · *Zona de prueba*: `crates/ropero-cli/tests/opencode.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-OC-1** | El plugin es `opencode-plugin/index.ts`, un plugin de TypeScript (`@opencode/plugin`) con `id: "ropero-modelo"`. | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:4-40, :146-147. | Test `r-oc_01_plugin_opencode_plugin_index_plugin_typescript` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-2** | El plugin se instala como copia, no como symlink — al revés que todo lo demás de ropero, y con razón explícita: el watcher de opencode (inotify) no sigue symlinks, así que los cambios… | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:688-691, :710-712, :722-732. | Test `r-oc_02_plugin_instala_como_copia_como_symlink` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-3** | El instalador no registra el plugin: al final comprueba si la config de opencode declara `"ropero-modelo"` y, si no, avisa e imprime el snippet. | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: bin/lib/cmd-install.sh:741-753, opencode-plugin/README.md:14. | Test `r-oc_03_instalador_registra_plugin_final_comprueba_config` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-4** | Las tres opciones del plugin (+ tabla en el doc A) | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:42-57, :151, :164, :271-279. | Test `r-oc_04_las_tres_opciones_del_plugin` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-5** | Los tres hooks usados y cuándo (+ tabla en el doc A) | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:215-281, opencode-plugin/README.md:96-103. | Test `r-oc_05_los_tres_hooks_usados_cuando` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-6** | El plugin filtra: consulta `ropero list` (cacheado) y no hace nada si el `modelID` no es un modelo de ropero — «el server ya está sirviendo lo que sea, y meter a ropero por medio sólo… | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:76-101, :179-191. | Test `r-oc_06_plugin_filtra_consulta_ropero_list_cacheado` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-7** | Si `ropero use` falla, el hook lanza: la petición de opencode falla. | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:167-172, opencode-plugin/README.md:60-62. | Test `r-oc_07_ropero_use_falla_hook_lanza_peticion` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-8** | El `variant` de opencode se pasa como `--nivel` sólo si coincide exactamente con un nivel declarado por el modelo (`ropero levels <modelo>`). | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:230-232. | Test `r-oc_08_variant_opencode_pasa_como_nivel_solo` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-9** | La deduplicación usa `servido` + `enCurso` para no lanzar dos `ropero use` por dos peticiones seguidas; | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:153-160, :192-210. | Test `r-oc_09_deduplicacion_usa_servido_encurso_para_lanzar` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-10** | El contrato estable con la CLI es de tres cosas: `list` (dos espacios + nombre), `levels` (uno por línea) y `use` (última línea `ROPERO_PORT=<n>`). | `crates/ropero-cli/tests/opencode.rs` | Test `r-oc_10_contrato_estable_con_cli_tres_cosas` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |
| **R-OC-11** | El `provider` por defecto del plugin es `"marichocho"`, un provider que no existe en la configuración actual de opencode del dueño. | `crates/ropero-cli/tests/opencode.rs` <br>*Semilla v1*: opencode-plugin/index.ts:151. | Test `r-oc_11_provider_por_defecto_del_plugin_marichocho` en `crates/ropero-cli/tests/opencode.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `opencode-plugin/`. El test se pone rojo si el comportamiento se altera. |

### 4.13.1 Qué es Strata, y por qué ropero lo trata aparte

*Módulo v2*: `crates/ropero-runtime/src/engines/strata.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/strata.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-STRATA-1** | Strata no es un servidor de modelos: es un *terminal* que baja un motor, prepara los artefactos y deja el modelo en disco. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: test/test-strata.sh:5-16, :102-109. | Test `r-strata_01_strata_servidor_modelos_terminal_que_baja` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que escribe lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-2** | Un modelo de Strata en el catálogo es un puntero al catálogo de Strata: `[model].path = "<familia>/<paquete>"` (p. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:160-176; models/strata/gyro-m.toml:22-25. | Test `r-strata_02_modelo_strata_catalogo_puntero_catalogo_strata` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-3** | Los ficheros no los baja ropero: los baja Strata al arrancar. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:188-237; docs/strata.md. | Test `r-strata_03_los_ficheros_los_baja_ropero_los` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-4** | La instalación de Strata es la oficial: se descarga el instalador a un fichero temporal (nunca `curl \| sh`) y se ejecuta con `env STRATA_BIN_DIR=$BIN_DIR sh $tmp`. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:87-111. | Test `r-strata_04_instalacion_strata_oficial_descarga_instalador_fichero` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-5** | La precedencia de `resolve` es PATH → `$BIN_DIR` → `~/.local/bin/strata` → `~/.local/share/strata/terminal/current/bin/strata` — el *launcher activo*, no la versión más nueva que haya… | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:124-139. | Test `r-strata_05_precedencia_resolve_path_bindir_local_bin` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-6** | El probe distingue tres estados: `missing` (Strata ausente), `down` (Strata presente, puerto sin servicio) y `ok`. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:141-154. | Test `r-strata_06_probe_distingue_tres_estados_missing_strata` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-7** | Strata no hace auth (`native_auth` → `no`): la clave la pone el proxy de ropero. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/lib/engines/strata.sh:157-163. | Test `r-strata_07_strata_hace_auth_nativeauth_clave_pone` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-8** | El `launch` de ropero devuelve un PID, y el proceso supervisor es otro: `strata launch --no-tui` devuelve cuando el server ya sirve. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: test/test-strata.sh:160-161; bin/server/strata-run.sh:14-30. | Test `r-strata_08_launch_ropero_devuelve_pid_proceso_supervisor` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-9** | El puente `strata-run.sh` (1) lanza Strata y espera a que termine —la descarga puede ser de 70 GB—, (2) se queda vivo mientras el modelo sirve, y (3) al recibir SIGTERM/SIGINT para lo… | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/server/strata-run.sh:14-30, :100-160. | Test `r-strata_09_puente_strata_run_lanza_strata_espera` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-10** | Para poder parar sólo lo suyo, el launch se guarda con `--save-preset <nombre>` y al parar se usa `strata launch --preset <nombre> --stop`: `strata launch --stop` a secas para todos… | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/server/strata-run.sh:36-48, :100-110. | Test `r-strata_10_para_poder_parar_solo_suyo_launch` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que escribe lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-11** | El puente comprueba cada 15 s que el launch sigue corriendo (`strata launch --status --output json`): si Strata lo paró o murió el motor, el wrapper sale con error para que ropero… | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: bin/server/strata-run.sh:130-160. | Test `r-strata_11_puente_comprueba_cada_que_launch_sigue` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-12** | El `launch` de ropero pasa `--backend strata`, `--backend-bin <binario>`, `--model <familia/paquete>`, el id publicado de ropero (`strata-qwen38-q2`), el contexto del modelo y el… | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: test/test-strata.sh:169-177. | Test `r-strata_12_launch_ropero_pasa_backend_strata_backend` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-13** | El prefijo `--set` es el mecanismo con el que se pasan opciones con valor a Strata: `ropero use strata-qwen36-q4 --vram-reserve 1344` se traduce en `--set vram_reserve_mib=1344`. | `crates/ropero-runtime/tests/strata.rs` <br>*Semilla v1*: docs/strata.md:132-137; models/strata/gyro-m.toml:56-60. | Test `r-strata_13_prefijo_set_mecanismo_con_que_pasan` en `crates/ropero-runtime/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/strata.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.13.2 El hueco: `strata launch` no puede servir estos modelos

*Módulo v2*: `crates/ropero-forge/src/strata.rs` · *Zona de prueba*: `crates/ropero-forge/tests/strata.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-STRATA-14** | 🔴 El motor de ropero lanza Strata con `strata launch`, que vuelve a pasar por `setup.py`. | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:15-18, :184-205. | Test `r-strata_14_motor_ropero_lanza_strata_con_strata` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-15** | 🔴 Cada GGUF necesita su propio pack, no el del modelo ya instalado. | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:260-271. | Test `r-strata_15_cada_gguf_necesita_propio_pack_del` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-16** | El profile de expertos propio se genera sintético y neutro (no calibrado con activaciones), con ranking round-robin por capa: capa 0 exp0, capa 1 exp0, … capa 47 exp0, capa 0 exp1, …… | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:184-205, :206-258. | Test `r-strata_16_profile_expertos_propio_genera_sintetico_neutro` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-17** | El script no se limita a lanzar: el ciclo completo es `preparar` → `afinar` → `lanzar`, y cada uno tiene su razón: - `preparar`: carpetas + descarga + verificación (`strata models… | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:5-11, :363-399, :402-413, :449-473. | Test `r-strata_17_script_limita_lanzar_ciclo_completo_preparar` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-18** | El `setup.py` de Strata se invoca directamente con `STRATA_CONTROL_INSTALL=1` y un `STRATA_INSTALL_DIR` privado por modelo, porque `setup.py` usa `CONTROL` para dirigir todas las… | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:160-182. | Test `r-strata_18_setup_strata_invoca_directamente_con_stratacontrolinstall` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-19** | El parche de args reconstruye la lista conservando las opciones de tuning actuales y sustituyendo sólo pack, profile y reserva (incluidos duplicados). | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:301-354. | Test `r-strata_19_parche_args_reconstruye_lista_conservando_las` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-20** | El `afinar` exige GPU libre y verifica el resultado del tune por JSON, no por el exit code: exige `ok is True`, `outcome` ∈ {`improved`, `default_confirmed`} y un `preset.id` presente. | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:363-399. | Test `r-strata_20_afinar_exige_gpu_libre_verifica_resultado` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-21** | La comprobación de GPU libre es estricta a propósito: tolera usos triviales (≤500 MiB por proceso —el escritorio puede aparecer como proceso CUDA con unos pocos MiB—) pero cualquier… | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:100-125. | Test `r-strata_21_comprobacion_gpu_libre_estricta_proposito_tolera` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-22** | El `parar` del script se niega a parar procesos: «este script no registra propiedad de motores; | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:96-98. | Test `r-strata_22_parar_del_script_niega_parar_procesos` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.13.3 Los requisitos que la integración impone

*Módulo v2*: `crates/ropero-forge/src/strata.rs` · *Zona de prueba*: `crates/ropero-forge/tests/strata.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-STRATA-23** | 🔴 El engine de Strata necesita un segundo modo de lanzamiento: `strata serve --profile <run-config>` además del actual `strata launch --family … --model …`. | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_23_engine_strata_necesita_segundo_modo_lanzamiento` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-24** | 🔴 `[model].path` necesita un segundo significado para Strata: hoy significa `<familia>/<paquete>` del catálogo de Strata; | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_24_model_path_necesita_segundo_significado_para` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-25** | 🔴 El artefacto preparado necesita un sitio en el catálogo de ropero (+ tabla en el doc A) | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_25_artefacto_preparado_necesita_sitio_catalogo_ropero` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-26** | El nombre público del GGUF es una decisión con consecuencias: el script renombra ambos modelos a `Qwen3.8-Flash-Next-Gyro-M-TQ2_0.gguf` porque «setup.py sólo encuentra ese nombre». | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_26_nombre_publico_del_gguf_una_decision` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-27** | El `tune` debe entrar en ropero como una operación de primer nivel: `ropero tune <modelo> [--mode quick\|thorough]` que mide la máquina y guarda el preset, con la misma verificación… | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_27_tune_debe_entrar_ropero_como_una` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que escribe lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-28** | La opción `vram-reserve` está declarada pero muerta: aparece en `[options].list` y su bloque `[args.option.vram-reserve]` está comentado en los dos TOML de Strata. | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: models/strata/gyro-m.toml:29, :56-60; docs/strata.md:132-137; groot-strata.sh:343-344. | Test `r-strata_28_opcion_vram_reserve_esta_declarada_pero` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-29** | La reserva de VRAM (1344 MiB) es un valor medido en una máquina concreta (MSI Stealth AI+ A16, RTX 5080 Laptop). | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_29_reserva_vram_1344_mib_valor_medido` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-30** | La preparación del pack necesita `llama.cpp` con `gguf-py` ya instalado en el árbol de Strata: el script no lo descarga automáticamente («Instálalo previamente y vuelve a ejecutar… | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_30_preparacion_del_pack_necesita_llama_cpp` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-31** | El aislamiento del `setup.py` exige que el directorio privado de instalación no sea un symlink y no coincida con `INSTALL_DIR` compartido. | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:168-170. | Test `r-strata_31_aislamiento_del_setup_exige_que_directorio` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-32** | El estado del script es su propio disco, no el de ropero: `$RAIZ/<id>/tune.json`, `$RAIZ/<id>/tune.log`, `$RAIZ/<id>/prepare-pack.log` y el run config en… | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_32_estado_del_script_propio_disco_ropero` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-33** | La comprobación de GPU libre del script es más estricta que la de ropero, y ambas son razonables en su contexto: ropero es idempotente (si el modelo ya corre en el puerto, no lo toca —… | `crates/ropero-forge/tests/strata.rs` | Test `r-strata_33_comprobacion_gpu_libre_del_script_mas` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se rechaza lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-34** | El script no es portable a macOS por tres razones concretas, y ropero tiene una restricción dura equivalente: `readlink -f` (ropero tiene `real_dir`/`real_path`/`_rp_link_real`… | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:48, :140, :288; install.sh:61-66; bin/lib/00-utils.sh. | Test `r-strata_34_script_portable_macos_por_tres_razones` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-STRATA-35** | El `estado` del script usa `mktemp` + fichero temporal para el `strata models list --installed --output json`, con un motivo concreto y medido: en tubería, si el lector de Python se… | `crates/ropero-forge/tests/strata.rs` <br>*Semilla v1*: groot-strata.sh:422-428. | Test `r-strata_35_estado_del_script_usa_mktemp_fichero` en `crates/ropero-forge/tests/strata.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/strata.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.13.5 El estado objetivo

*Módulo v2*: `crates/ropero-forge/src/strata.rs` · *Zona de prueba*: `crates/ropero-forge/tests/strata.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-STRATA-36** 🔶 | Strata es un motor de primera clase. El objetivo declarado es que `ropero <modelo>` lo sirva como a los demás, sin que el usuario tenga que saber que Strata es especial. | `crates/ropero-forge/src/strata.rs`<br>*Cambio de arquitectura*: El doc A lo marca como objetivo. | `ropero install engines strata` deja Strata como motor de primera clase: su manifiesto declara `maturity`, su `install` y su `launch.kind = "custom"`. |
| **R-STRATA-37** 🔶 | La preparación es la parte que falta, y son tres cosas que hoy hace el script a mano: el pack propio del GGUF (R-STRATA-15), el expert-profile propio (R-STRATA-16) y el tune medido… | `crates/ropero-forge/src/strata.rs`<br>*Cambio de arquitectura*: Cierra el hueco medido: `strata launch` reescribe el run config con el profile equivocado. | El arranque de Strata genera el pack y el expert-profile propios del modelo, no reutiliza los de Flash-Next; el test verifica que el profile generado tiene la forma 48×256 del modelo Gyro-M. |
| **R-STRATA-38** 🔶 | El script es el camino manual, no el camino integrado. Su valor en la especificación es el de especificar el contrato de lo que ropero deberá hacer: el `profile` por GGUF, el pack por… | `crates/ropero-forge/src/strata.rs`<br>*Cambio de arquitectura*: `groot-strata.sh` es el camino manual que especifica el contrato. | Las capacidades del script `groot-strata.sh` quedan cubiertas por el mismo contrato: el test compara el argv y los ficheros generados por `ropero` contra los que produce el script, para los dos modelos. |

### 4.14.1 Qué es

*Módulo v2*: `crates/ropero-forge/src/lib.rs` · *Zona de prueba*: `crates/ropero-forge/tests/forge.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-1** | `ropero gguffer` es la única implementación de fabricación de GGUFs (descarga → conversión → imatrix → cuantización → verificación → model card). | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1-15. | Test `r-gguf_01_ropero_gguffer_unica_implementacion_fabricacion_ggufs` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/lib.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-2** | El gguffer no ejecuta el TOML: lee sus campos y monta el argv, igual que el resto del sistema. | `crates/ropero-forge/tests/forge.rs` | Test `r-gguf_02_gguffer_ejecuta_toml_lee_sus_campos` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/lib.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-59** 🔶 | El gguffer tiene una precondición que ningún otro comando tiene: necesita un motor llama.cpp disponible (el oficial o el fork), porque es el que convierte los pesos, cuantiza y une… | `crates/ropero-forge/src/lib.rs`<br>*Cambio de arquitectura*: Principio: una precondición no documentada es una precondición que se descubre fallando. | El gguffer es el único comando con precondición de entorno, y esa precondición está declarada en el propio comando (`ropero gguffer --help` la dice). |

### 4.14.2 El contrato de la CLI

*Módulo v2*: `crates/ropero-cli/src/gguffer.rs` · *Zona de prueba*: `crates/ropero-cli/tests/cli.rs` · *Tipo*: cli

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-3** | La fuente es una de tres, y son excluyentes: `--path <dir>` (un repo ya clonado), `--repo <owner/name>` (a clonar) o `--gguf <fichero\|owner/repo>` (un GGUF existente). | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1418. | Test `r-gguf_03_fuente_una_tres_son_excluyentes_path` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-4** | Las opciones de salida son dos y distintas: `--out NOMBRE\|RUTA` (nombre/carpeta/ruta) y `--out-principal RUTA` (la ruta exacta del primer quant de la tirada). | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:27-32, :90-121, :1466-1485. | Test `r-gguf_04_las_opciones_salida_son_dos_distintas` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-5** | Con varios quants, `--out` no puede ser la ruta exacta de un fichero: debe ser un nombre o un directorio. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1466-1479. | Test `r-gguf_05_con_varios_quants_out_puede_ser` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-6** | Las reglas del imatrix son duras y comprobadas antes de nada: - `--imatrix` (fichero local) es excluyente con el par `--imatrix-repo`/`--imatrix-file`; | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:45-48, :1350-1371, test/test-gguffer.sh:191-214, :242-246. | Test `r-gguf_06_las_reglas_del_imatrix_son_duras` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-7** | `--quant` es repetible y admite coma: `--quant IQ2_M --quant IQ3_XS` o `--quant IQ2_M,IQ3_XS`. | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:27, :54-58, :1288-1295; test/test-gguffer.sh:152-156. | Test `r-gguf_07_quant_repetible_admite_coma_quant_iq2m` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.3 El ahorro que da forma al diseño

*Módulo v2*: `crates/ropero-forge/src/pipeline.rs` · *Zona de prueba*: `crates/ropero-forge/tests/pipeline.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-8** | 🔴 La comprobación de viability ocurre ANTES de descargar los pesos. | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1619-1685; test/fakes/gguffer/llama-quantize:121-129. | Test `r-gguf_08_comprobacion_viability_ocurre_antes_descargar_los` en `crates/ropero-forge/tests/pipeline.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/pipeline.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-9** | 🔴 Un repo ya cuantizado (NVFP4) se conserva, no se cuantiza: el GGUF sale con `general.file_type = 39` (NVFP4) y el mensaje lo dice. | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: test/test-gguffer.sh:144-161, :219-223. | Test `r-gguf_09_repo_cuantizado_nvfp4_conserva_cuantiza_gguf` en `crates/ropero-forge/tests/pipeline.rs` (prueba unit): comprueba que termina con el código de salida pactado lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/pipeline.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-10** | Un repo en FP8 sí puede cuantizarse desde su F16 dequantizado, y el gguffer avisa de que dequantiza (se pierde la ventaja del quant de fábrica). | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: test/test-gguffer.sh:235-240. | Test `r-gguf_10_repo_fp8_puede_cuantizarse_desde_f16` en `crates/ropero-forge/tests/pipeline.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/pipeline.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.4 La cabeza MTP: el bloque que el imatrix no cubre

*Módulo v2*: `crates/ropero-forge/src/mtp.rs` · *Zona de prueba*: `crates/ropero-forge/tests/mtp.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-11** | El imatrix se calcula sobre el modelo base (64 capas), pero el GGUF lleva la cabeza MTP como un bloque más (el 65). | `crates/ropero-forge/tests/mtp.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:123-140. | Test `r-gguf_11_imatrix_calcula_sobre_modelo_base_capas` en `crates/ropero-forge/tests/mtp.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/mtp.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-12** | La cobertura del imatrix se comprueba por bloque entero, no tensor a tensor: los tensores que el imatrix no tiene son normalmente las normas f32, que no se cuantizan y no exigen… | `crates/ropero-forge/tests/mtp.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:123-140. | Test `r-gguf_12_cobertura_del_imatrix_comprueba_por_bloque` en `crates/ropero-forge/tests/mtp.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/mtp.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-13** | El bloque MTP se localiza por metadata, no por el nombre del tensor: los tensores de la MTP se llaman como los de cualquier otro bloque (`blk.64.attn_k.weight`), sin «nextn» en el… | `crates/ropero-forge/tests/mtp.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:151-176. | Test `r-gguf_13_bloque_mtp_localiza_por_metadata_por` en `crates/ropero-forge/tests/mtp.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/mtp.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-14** | Cuando el error es de imatrix, el gguffer traduce el mensaje opaco del cuantizador («Missing importance matrix for tensor blk.64.attn_k.weight») al lugar del que sale: «es el bloque… | `crates/ropero-forge/tests/mtp.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (fase 3, cola del error de cuantización). | Test `r-gguf_14_cuando_error_imatrix_gguffer_traduce_mensaje` en `crates/ropero-forge/tests/mtp.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/mtp.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.5 Las seis fases

*Módulo v2*: `crates/ropero-forge/src/phases.rs` · *Zona de prueba*: `crates/ropero-forge/tests/pipeline.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-15** | El gguffer tiene seis fases en orden fijo: 1. | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (fases 1–6, comentadas). | Test `r-gguf_15_gguffer_tiene_seis_fases_orden_fijo` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-16** | 🔴 Al fallar la cuantización NO se borra nada: el clon de los safetensors y el GGUF intermedio se quedan, y el error dice exactamente cómo reintentar sólo la cuantización (que es la… | `crates/ropero-forge/src/phases.rs`<br>*Cambio de arquitectura*: P-16: la precondición/limpieza se comprueba antes de la acción cara. <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (fase 3, conservo los pesos en: $work). | Un fallo en la fase de cuantización deja el clon y el intermedio **en disco**, y el error dice cómo reintentar sólo esa fase. |
| **R-GGUF-17** | 🔴 Los pesos sólo se borran si el GGUF supera la verificación: «NO borro los pesos: están en $work». | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (fases 4 y 5). | Test `r-gguf_17_los_pesos_solo_borran_gguf_supera` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-18** | El intermedio (F16) se borra después de todos los quants, no en cada uno: con varios quants en una tirada, el siguiente necesita el F16. | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (fase 5). | Test `r-gguf_18_intermedio_f16_borra_despues_todos_los` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.6 La procedencia: por qué no es opcional

*Módulo v2*: `crates/ropero-forge/src/provenance.rs` · *Zona de prueba*: `crates/ropero-forge/tests/provenance.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-19** | 🔴 Todo GGUF producido deja un `<out>.fuente.json` con de dónde salió el modelo, con qué quant, con qué importance matrix, y si la cabeza MTP quedó en un quant aparte. | `crates/ropero-forge/tests/provenance.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (fase 6); test/test-gguffer.sh:179-182. | Test `r-gguf_19_todo_gguf_producido_deja_out_fuente` en `crates/ropero-forge/tests/provenance.rs` (prueba unit): comprueba que escribe lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/provenance.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-20** | El gguffer no miente sobre el resultado: si los pesos venían en NVFP4 y no se cuantizó nada, el mensaje final dice «NO he cuantizado nada: el GGUF se ha quedado en NVFP4» —el quant… | `crates/ropero-forge/tests/provenance.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (cola de la fase 6). | Test `r-gguf_20_gguffer_miente_sobre_resultado_los_pesos` en `crates/ropero-forge/tests/provenance.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/provenance.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.7 `ggufcheck`: el que sólo lee la cabecera

*Módulo v2*: `crates/ropero-forge/src/gguf.rs` · *Zona de prueba*: `crates/ropero-forge/tests/gguf.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-21** | `bin/python/ggufcheck.py` lee sólo la cabecera en todos los modos: ningún `read()` puede pasar de 100 MiB aunque el fichero sea de 300 MiB (fichero disperso). | `crates/ropero-forge/tests/gguf.rs` <br>*Semilla v1*: test/test-ggufcheck.sh:38-47, :51-75. | Test `r-gguf_21_bin_python_ggufcheck_lee_solo_cabecera` en `crates/ropero-forge/tests/gguf.rs` (prueba unit): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/gguf.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-54** 🔶 | El gguffer exige un motor llama.cpp disponible —el oficial o el fork— y esa es una precondición, no un detalle: sin ella no hay quien convierta los pesos, ni quien cuantice, ni quien… | `crates/ropero-forge/src/gguf.rs`<br>*Cambio de arquitectura*: S-7 / H-27. Es el arreglo de mayor valor del documento. <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1376-1392, :840-846; bin/lib/01-env.sh:33-34. | `ropero gguffer` comprueba la precondición del motor **antes** de la fase 1: sin motor, el error nombra la causa y la acción, y no se descarga nada. |
| **R-GGUF-55** 🔶 | El conversor es el punto que no admite sustituto: sin `$LLAMA_DIR/convert_hf_to_gguf.py` no hay conversión, y por tanto no hay GGUF — con o sin quant. | `crates/ropero-forge/src/gguf.rs`<br>*Cambio de arquitectura*: El conversor no admite sustituto. <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1377, :1992. | El conversor `convert_hf_to_gguf.py` se busca en `$LLAMA_DIR` y en el PATH, y la ausencia produce `ToolchainError::MissingTool` con la ruta buscada y la acción. |
| **R-GGUF-56** 🔶 | La precondición se comprueba ANTES de descargar los pesos, con un mensaje que nombre el motor que falta y la variable que lo apunta. | `crates/ropero-forge/src/gguf.rs`<br>*Cambio de arquitectura*: Cierra el diagnóstico falso que empuja a borrar 60 GB. <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1957 (el único -f sobre el motor es para el parche), :2035-2041 (el clasificador). | El diagnóstico de un fallo de precondición nunca menciona la descarga: `ToolchainError` tiene una variante por causa y ninguna dice «descarga incompleta». |
| **R-GGUF-57** 🔶 | Los dos motores no dejan el clon en el mismo sitio, y eso es una costura del gguffer (+ tabla en el doc A) | `crates/ropero-forge/src/gguf.rs`<br>*Cambio de arquitectura*: H-28: el oficial vive en `$LLAMA_OFFICIAL_DIR` y el gguffer no lo veía. <br>*Semilla v1*: bin/lib/01-env.sh:33-34; bin/lib/engines/llama-server.sh:78, :155; bin/lib/engines/llama-server-official.sh:60, :77; bin/lib/cmd-g | Los motores `llama-server` y `llama-server-official` registran su directorio de clon en el estado de la instalación, y el gguffer lo consulta en lugar de asumir `$LLAMA_DIR`. |
| **R-GGUF-58** 🔶 | Ni `llama-quantize` ni `llama-gguf-split` se enlazan en `$BIN_DIR` al instalar un engine: los engines instalan `llama-server` y `llama-server-official`, nada más. | `crates/ropero-forge/src/gguf.rs`<br>*Cambio de arquitectura*: H-29: el PATH alternativo no lo activaba ningún instalador. <br>*Semilla v1*: bin/lib/cmd-install.sh:599-613 (el enlace es el basename del binario resuelto), bin/lib/engines/llama-server.sh (enlaza llama-serv | `ropero install engines` enlaza `llama-quantize` y `llama-gguf-split` en `$BIN_DIR`, o registra su ruta en el estado; el gguffer los encuentra sin exportar nada a mano. |
| **R-GGUF-22** | Los modos son `--meta`, `--tipo-predominante`, `--file-type`, `--nombres`, `--bloque`, `--info`, y la validación por defecto, y cada uno tiene una salida exacta esperada. | `crates/ropero-forge/tests/gguf.rs` <br>*Semilla v1*: test/test-ggufcheck.sh:38-47. | Test `r-gguf_22_los_modos_son_meta_tipo_predominante` en `crates/ropero-forge/tests/gguf.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/gguf.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-23** | Un GGUF inválido falla con rc=1, mensaje claro y sin traceback: un índice de tensores incompleto (declara 3, tiene 2) informa del recuento real; | `crates/ropero-forge/tests/gguf.rs` <br>*Semilla v1*: test/test-ggufcheck.sh:115-134. | Test `r-gguf_23_gguf_invalido_falla_con_mensaje_claro` en `crates/ropero-forge/tests/gguf.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/gguf.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-24** | El parámetro deprecado `solo_cabecera=True` se sigue aceptando (y se ignora): compatibilidad hacia atrás. | `crates/ropero-forge/tests/gguf.rs` <br>*Semilla v1*: test/test-ggufcheck.sh:79-93. | Test `r-gguf_24_parametro_deprecado_solocabecera_true_sigue_aceptando` en `crates/ropero-forge/tests/gguf.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/gguf.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-25** | `ggufcheck` se puede ejecutar contra cualquier GGUF, no sólo los de ropero: acepta `--lib <ruta>` para la biblioteca de ggml. | `crates/ropero-forge/tests/gguf.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh (fase 4); bin/python/ggufcheck.py. | Test `r-gguf_25_ggufcheck_puede_ejecutar_contra_cualquier_gguf` en `crates/ropero-forge/tests/gguf.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/gguf.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.8 La taxonomía del checkpoint: cinco clases

*Módulo v2*: `crates/ropero-forge/src/checkpoint.rs` · *Zona de prueba*: `crates/ropero-forge/tests/checkpoint.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-26** | El preflight clasifica el checkpoint antes de bajar nada, leyendo `config.json` y `hf_quant_config.json` (del repo por HTTP con `curl -fsS`, o del directorio local) y devolviendo una… | `crates/ropero-forge/tests/checkpoint.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:288-378, :1647-1673. | Test `r-gguf_26_preflight_clasifica_checkpoint_antes_bajar_nada` en `crates/ropero-forge/tests/checkpoint.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/checkpoint.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-27** | Las reglas de parseo tienen tres trampas resueltas: el `quant_algo` va anidado en `quantization`, no en la raíz; | `crates/ropero-forge/tests/checkpoint.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:223-245, :279-283, :305-306. | Test `r-gguf_27_las_reglas_parseo_tienen_tres_trampas` en `crates/ropero-forge/tests/checkpoint.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/checkpoint.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-28** | El plan de cuantización decide por cada quant pedido en cuatro situaciones: (1) el GGUF ya está en ese quant → no se cuantiza; | `crates/ropero-forge/tests/checkpoint.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:426-483. | Test `r-gguf_28_plan_cuantizacion_decide_por_cada_quant` en `crates/ropero-forge/tests/checkpoint.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/checkpoint.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-29** | «Cuantizado» significa: todo `file_type` salvo `""`, `0` (F32), `1` (F16) y `32` (BF16). | `crates/ropero-forge/tests/checkpoint.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:389-394. | Test `r-gguf_29_cuantizado_significa_todo_filetype_salvo_f32` en `crates/ropero-forge/tests/checkpoint.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/checkpoint.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-30** | Un quant desconocido aborta con rc 1 antes de descargar: la lista se parsea del `llama-quantize --help` del motor, en dos versiones (con alias y canónica, `grep -v "alias for"`), y el… | `crates/ropero-forge/tests/checkpoint.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1440-1465. | Test `r-gguf_30_quant_desconocido_aborta_con_antes_descargar` en `crates/ropero-forge/tests/checkpoint.rs` (prueba unit): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/checkpoint.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.9 La superficie completa de la CLI

*Módulo v2*: `crates/ropero-cli/src/gguffer.rs` · *Zona de prueba*: `crates/ropero-cli/tests/cli.rs` · *Tipo*: cli

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-31** | `ropero gguffer` no tiene subcomandos: es un comando con flags, con dos modos activados por flag: `--list-quants` (o `--quant` sin valor) lista los quants del motor y los publicados… | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1324-1326, :1391-1415, :2060-2064. | Test `r-gguf_31_ropero_gguffer_tiene_subcomandos_comando_con` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-32** | Los flags completos, con su obligatoriedad (+ tabla en el doc A) | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1271-1327. | Test `r-gguf_32_los_flags_completos_con_obligatoriedad` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-33** | Las variables de entorno que gobiernan el comportamiento y no salen en `--help`: `GGUF_DIR` (`$HOME/models/gguf`), `GGUF_WORK_DIR` (`$HOME/.cache/ropero/gguffer`), `LLAMA_DIR`… | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:975, :1271, :1376, :1476-1477, :1801. | Test `r-gguf_33_las_variables_entorno_que_gobiernan_comportamiento` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-34** | La ruta `[build]` del TOML se traduce al argv del gguffer por una tabla explícita en `modelconf.py`: `repo→--repo`, `gguf→--gguf`, `gguf_file→--gguf-file`, `imatrix→--imatrix`,… | `crates/ropero-cli/tests/cli.rs` <br>*Semilla v1*: bin/python/modelconf.py:1248-1263, :1326-1334; bin/lib/02-modelconf.sh:323-336; bin/quant/ACRITO.md:23-26. | Test `r-gguf_34_ruta_build_del_toml_traduce_argv` en `crates/ropero-cli/tests/cli.rs` (prueba cli): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-cli/src/gguffer.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.10 La conversión: tres arreglos que existen por casos reales

*Módulo v2*: `crates/ropero-forge/src/phases.rs` · *Zona de prueba*: `crates/ropero-forge/tests/pipeline.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-35** | El nombre del intermedio describe lo que lleva dentro: si el checkpoint es NVFP4 el intermedio es `-NVFP4`, si es MXFP4 `-MXFP4_MOE`, si no `-F16`/`-BF16` según `--intermedio`. | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1630-1635, :1802-1826. | Test `r-gguf_35_nombre_del_intermedio_describe_que_lleva` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-36** | `--no-mtp` sólo se pasa si el conversor lo entiende: se comprueba con `grep -qE "no-mtp\|no_nextn"` sobre el conversor. | `crates/ropero-forge/src/phases.rs`<br>*Cambio de arquitectura*: La procedencia no es opcional (R-GGUF-18). <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1919-1925; :187-200. | Un GGUF sin sidecar de procedencia se rechaza en `ropero install models` y en `ropero doctor`. |
| **R-GGUF-37** | El tokenizer se arregla antes de convertir, en dos pasos: (i) se inyecta el `chat_template.jinja` del repo dentro del `tokenizer_config.json` — el conversor de llama.cpp nunca lee el… | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1070-1141, :1984-1989. | Test `r-gguf_37_tokenizer_arregla_antes_convertir_dos_pasos` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-38** | La conversión tiene dos reintentos automáticos: (1) `Tokenizer class … does not exist` ⇒ se arregla el tokenizer y se reintenta; | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1992-2051. | Test `r-gguf_38_conversion_tiene_dos_reintentos_automaticos_tokenizer` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-39** | Las flags del cuantizador van antes de los posicionales: `llama-quantize` sólo parsea opciones mientras el argumento empiece por `--`; | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:2213-2219. | Test `r-gguf_39_las_flags_del_cuantizador_van_antes` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-40** | El bloque MTP se localiza por dos vías que se unen: la metadata (`bloque_mtp = block_count - nextn_predict_layers`) y la comparación bloque a bloque contra el imatrix. | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:2115-2166, :878-906. | Test `r-gguf_40_bloque_mtp_localiza_por_dos_vias` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-41** | El `general.file_type` (llama_ftype: NVFP4=39, MXFP4_MOE=38, BF16=32, F16=1) nunca se compara con el `ggml_type` (NVFP4=40, MXFP4=39): son espacios de números distintos y «nunca… | `crates/ropero-forge/tests/pipeline.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1435-1438, :2072-2098; bin/quant/quant-tipos.sh:9-11. | Test `r-gguf_41_general_filetype_llamaftype_nvfp4_mxfp4moe_bf16` en `crates/ropero-forge/tests/pipeline.rs` (prueba integration): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/phases.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.11 Los parches del motor

*Módulo v2*: `crates/ropero-runtime/src/engines/llama.rs` · *Zona de prueba*: `crates/ropero-runtime/tests/engines.rs` · *Tipo*: integration

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-42** | El motor de ropero es un clon privado (`Lorenzo-SF/ropero-llama-turboquant`) con fallback al fork público, y recibe parches que se aplican con `patch -p1` y exigen recompilar después… | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/quant/aplicar-parche-motor.sh:103-118, :153-158, :199-203, :206-220; bin/lib/engines/llama-server.sh:77-117. | Test `r-gguf_42_motor_ropero_clon_privado_lorenzo_ropero` en `crates/ropero-runtime/tests/engines.rs` (prueba integration): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/llama.rs`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-43** | La aplicación del parche mixed-precision en el gguffer es fail-open (`patch -p1 … \|\| true`): si el parche no entra, se sigue y la conversión fallará después con un error menos claro. | `crates/ropero-runtime/tests/engines.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1959. | Test `r-gguf_43_aplicacion_del_parche_mixed_precision_gguffer` en `crates/ropero-runtime/tests/engines.rs` (prueba integration): comprueba que produce un error lo que el requisito afirma, contra la implementación de `crates/ropero-runtime/src/engines/llama.rs`. El test se pone rojo si el comportamiento se altera. |

### 4.14.12 La capa de scripts (`bin/quant/`)

*Módulo v2*: `crates/ropero-forge/src/` · *Zona de prueba*: `crates/ropero-forge/tests/forge.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-44** | `bin/quant/` es una capa de guiones sobre el gguffer, y la tabla `modelos.tsv` (7 columnas: `n pesos q imr imf flags fuente`) es su fuente de verdad —los guiones no llevan la lista… | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/cuantizar-todos.sh:5-6, :92-102, :113, :227; bin/quant/README.md:71-72, :78-81. | Test `r-gguf_44_bin_quant_una_capa_guiones_sobre` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que devuelve lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-45** | El nombre del destino nunca es mentiroso: si el nombre de la ruta del TOML no contiene el nombre del quant principal, el GGUF va a la carpeta con su nombre propio en vez de llevar el… | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/cuantizar-todos.sh:146-186; bin/lib/cmd-gguffer.sh:105-118, :1853-1858. | Test `r-gguf_45_nombre_del_destino_nunca_mentiroso_nombre` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que no debe ocurrir nunca lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-46** | Un fallo de un modelo no para la tirada (salvo `--parar`): «un modelo que no se deja fabricar no puede impedir que los demás sí». | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/cuantizar-todos.sh:35-37, :204-212, :244-272. | Test `r-gguf_46_fallo_modelo_para_tirada_salvo_parar` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que se rechaza lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-47** | `bin/quant/todo.sh` es el guion completo y su orden es significantivo: cuantizar → verificar → subir. | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/todo.sh:47-92, :66-82. | Test `r-gguf_47_bin_quant_todo_guion_completo_orden` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-48** | `verificar-gguf.sh` es la verificación posterior sin GPU: exige ≥ 100 MiB, metadata legible, `arch` presente, tensores > 0 y `block_count`; | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/verificar-gguf.sh:28-96. | Test `r-gguf_48_verificar_gguf_verificacion_posterior_sin_gpu` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que es obligatorio lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-49** | La subida a HF (`subir-hf.sh`) crea el repo `<HF_USER>/<nombre-modelo>-<QUANT>-GGUF` (el nombre se deriva del fichero y del `general.file_type` real), sube el GGUF, el imatrix si está… | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/subir-hf.sh:46, :105-129, :184, :200-300. | Test `r-gguf_49_subida_subir_crea_repo_hfuser_nombre` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que escribe lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-50** | El model card sale del propio GGUF, no de una tabla, y debe declarar que el modelo no es del cuantizador; | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/python/model_card.py:21-27, :186-188, :272, :305-309; bin/quant/subir-hf.sh:230-252. | Test `r-gguf_50_model_card_sale_del_propio_gguf` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |

### 4.14.13 Tres discrepancias del gguffer, medidas

*Módulo v2*: `crates/ropero-forge/src/` · *Zona de prueba*: `crates/ropero-forge/tests/forge.rs` · *Tipo*: unit

| Requisito | Qué exige | Dónde se comprueba | Puerta de aceptación |
|---|---|---|---|
| **R-GGUF-51** | 🟠 `subir-hf.sh --subir` NO verifica el GGUF que sube: sólo mira el marcador `.hf-uploaded`. | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/subir-hf.sh:203-206, :278; bin/quant/README.md:82-83. | Test `r-gguf_51_subir_subir_verifica_gguf_que_sube` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-52** | 🟠 La escritura de la procedencia es fail-open y su código de retorno se ignora: si `_gguffer_escribe_fuente` falla, el GGUF queda sin `.fuente.json` y la tirada sale 0. | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/lib/cmd-gguffer.sh:1006, :2315. | Test `r-gguf_52_escritura_procedencia_fail_open_codigo_retorno` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |
| **R-GGUF-53** | 🟡 El comentario de `cuantizar-todos.sh` está desfasado respecto al código: dice que el gguffer borra los safetensors «tanto si todo va bien como si algo falla»; | `crates/ropero-forge/tests/forge.rs` <br>*Semilla v1*: bin/quant/cuantizar-todos.sh:30-33 vs. bin/lib/cmd-gguffer.sh:2040-2047; bin/quant/patches/; bin/python/model_card.py vs. bin/quan | Test `r-gguf_53_comentario_cuantizar_todos_esta_desfasado_respecto` en `crates/ropero-forge/tests/forge.rs` (prueba unit): comprueba que se cumple lo que el requisito afirma, contra la implementación de `crates/ropero-forge/src/`. El test se pone rojo si el comportamiento se altera. |


---

## 8. Plan de construcción

### 8.1 El golden master: cómo se conserva lo que ya funciona

La reimplementación en Rust amenaza con perder los detalles que costaron años
(el sello, la jerarquía de sondas, el manejo del MTP, la taxonomía de
checkpoints). El mecanismo que lo evita:

> **Antes de reescribir una función, se congela su comportamiento actual como
> un golden test.**

**Procedimiento (por tarea):**

1. Localizar la evidencia del requisito en el doc A (`fichero:línea`).
2. Ejecutar el comportamiento v1 y **capturar su salida** (stdout, stderr,
   código de salida, ficheros escritos).
3. Escribir el golden test en Rust con esa salida **como valor esperado**.
4. Implementar en Rust hasta que coincida.
5. Si la salida v1 es incorrecta (un bug), **no se congela el bug**: se
   congela la salida *correcta* y el cambio de comportamiento se documenta
   como `fix` en el doc A, con su propio PR.

El punto 5 es el que distingue un golden master de unaXFONDA de congelar
errores. Congelar un bug convierte el bug en un requisito, y eso es exactamente
lo que un golden master mal hecho hace.

**Dónde vive el golden master**: `crates/ropero-core/tests/golden/` con
ficheros `.snap` (`insta`), uno por comportamiento. El test compara la salida
**estructurada**, no el texto: si el orden de dos líneas de un log cambia, el
test no falla, porque el log no es un contrato.

### 8.2 Las diez fases

```
F0 ──► F1 ──► F2 ──► F3 ──► F4 ──► F5 ──► F6 ──► F7 ──► F8 ──► F9
base   datos  motor  runtime  estado  catálogo  forge   puerta  convi-
                                                       (serv.) (v2/v1)
```

Cada fase es **entregable, verificable y útil por sí sola**. El sistema es
utilizable desde F3: un `ropero` v2 que arranca modelos vale más que un
`ropero` v2 con un catálogo perfecto y ningún motor.

| Fase | Nombre | Entrega | Cierra |
|---|---|---|---|
| **F0** | Cimientos | Workspace, toolchain, CI, crates vacíos, test de arquitectura | — |
| **F1** | Modelo de datos | Tipos del TOML, validador, catálogo, alias | R-DATA, R-ARQ, R-CTX |
| **F2** | Motores | Manifiestos, `EngineId`, compatibilidad,成熟 | R-ENG, R-CTX |
| **F3** | Runtime | Preflight, sonda, arranque, sello, primer plano | R-RUN, R-ARQ |
| **F4** | Estado y puertos | Estado en disco, identidad, parada, locks | R-EST, R-ARQ |
| **F5** | Catálogo y descargas | `ropero list`, `install models`, `hf` | R-MOD, R-CLI |
| **F6** | Forja | Las seis fases, checkpoints, MTP, procedencia | R-GGUF |
| **F7** | La puerta | Endpoint OpenAI-compatible, auth, streaming, enrutado | R-API, R-ROUTE |
| **F8** | Instalación y opencode | `install.sh` v2, plugin, `doctor`, `stats` | R-INST, R-OC, R-TST |
| **F9** | Convivencia | v1 y v2 conviviendo, migración, retirada de v1 | — |

### 8.3 Las tareas

Cada tarea nombra: lo que hace, los requisitos que cierra, sus dependencias y
su puerta. **Una tarea = un PR.**

#### F0 · Cimientos

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-01** | Workspace con los 6 crates, `rust-toolchain.toml` (1.100.0), `rustfmt.toml`, `clippy.toml`, `deny.toml`, `Cargo.lock` commiteado, perfil release | — | — | `cargo build` verde; grafo de dependencias verificado por test |
| **T-02** | Test de arquitectura: `ropero-core` no usa `std::process`, `std::fs`, `tokio` ni `reqwest`; el grafo no tiene ciclos; `ropero-cli` < 400 líneas | R-ARQ-1 | T-01 | El test falla si se añade una dependencia prohibida |
| **T-03** | CI: clippy, fmt, nextest, llvm-cov, cargo-deny, matriz macOS+Linux | — | T-01 | CI verde en local y en GitHub |
| **T-04** | ADR-001: «por qué Rust», ADR-002: «por qué extracción y no reescritura» y ADR-003: «`ropero-code` es referencia, no dependencia» | — | — | Los tres ADR existen con su plantilla |
| **T-05** | Portar `models/_template.toml` y los 45 TOML al esquema tipado, **sin cambiar su contenido**; el validador nuevo acepta los 45 | R-DATA | T-01 | `ropero validate` acepta los 45 y rechaza un TOML con clave desconocida |

#### F1 · Modelo de datos

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-06** | `ModelSpec` y todos sus tipos con `serde` + `deny_unknown_fields` | R-DATA-1…16 | T-05 | Cada tabla del doc A tiene su tipo y su test |
| **T-07** | Validador: las 15 reglas de § 4.1.1, cada una con un test que falla | R-DATA-17…23 | T-06 | 15 tests, uno por regla, todos rojos antes |
| **T-08** | `ArgvBuilder`: base → level → flags → options, `replace`, `requires`, placeholders | R-DATA-16…21 | T-06 | Golden tests contra el argv real de los TOML |
| **T-09** | Catálogo: descubrimiento de `models/`, `fired/`, unicidad de nombres, alias | R-MOD-1…8 | T-06 | El catálogo de 45 (17 activos + 28 fired) se carga |
| **T-10** | `ropero validate`, `ropero list`, `ropero template`, `ropero args`, `ropero levels`, `ropero path` | R-CLI-6…13 | T-09 | Los seis comandos con salida esperada |

#### F2 · Motores

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-11** | `EngineId` enum + alias legacy + canonicalización con aviso | R-ENG-1, R-DATA-13 | T-06 | Test de cada alias y de cada id canónico |
| **T-12** | Manifiesto de motor en TOML + los 8 manifiestos | R-ENG-2…7 | T-11 | Los 8 cargan y validan |
| **T-13** | Evaluación de compatibilidad: SO, NVIDIA, Metal, NPU, RAM; **acumula todas las razones** | R-CTX-1, R-ENG-3 | T-12 | Test en Linux y en macOS (condicional) |
| **T-14** | `maturity` en el manifiesto, visible en `install engines` y `doctor` | R-CTX-3 🔶 | T-12 | Los 6 motores probados y los 2 no probados se distinguen |

#### F3 · Runtime

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-15** | Preflight: SO, motor instalado, ficheros presentes, RAM, espacio en disco | R-RUN-1…4 | T-13 | Cada precondición tiene su test de fallo |
| **T-16** | Sonda de readiness de tres niveles con backoff y deadline | R-RUN-5…7 | T-15 | Un motor real arranca y la sonda pasa; un motor falso no |
| **T-17** | El sello: hash de modelo+contexto+nivel+ficheros | R-RUN-8 | T-08 | Cambiar el contexto cambia el sello |
| **T-18** | Arranque en primer plano y en background; `SIGINT` propaga al grupo | R-RUN-9…17 | T-16 | Ctrl-C deja el proceso muerto y el estado limpio |
| **T-19** | Máquina de estados con las 11 transiciones y el test exhaustivo | R-EST-1 | T-18 | Toda transición no permitida panic en test |

#### F4 · Estado y puertos

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-20** | `StateStore`: `CLAVE=valor`, escritura atómica, conserva claves | R-ARQ-9, R-EST-2 | T-19 | Un estado v1 se lee y se migra |
| **T-21** | Identidad `pid + start_time + exe_digest`; `ProcIdentity` | R-EST-8 | T-20 | Un PID reciclado **no** se confunde |
| **T-22** | `stop`: la comprobación de 4 pasos y la escalada SIGTERM→SIGKILL | R-EST-3…14 | T-21 | **Un proceso ajeno sobrevive** (test dedicado) |
| **T-23** | Lock de puerto con caducidad | R-RUN-17 | T-20 | Un lock huérfano se retira; uno vivo bloquea |
| **T-24** | `status`: reconstrucción del estado desde el disco | R-ARQ-11, R-EST-2 | T-20 | El estado tras un reinicio de ropero es el mismo |

#### F5 · Catálogo y descargas

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-25** | `ropero install models`: sólo los modelos de los engines instalados; sin TTY exige lista; y el contrato de `--prio` de § 4.10.7 (global o por modelo; se ignora si el modelo no tiene ese camino o si ya es su defecto; con las dos vías y sin `--prio`, se pregunta) | R-CTX-2, R-CLI-16, R-MOD-29 | T-09 | Con un engine instalado, sólo se ofrecen sus modelos; y `ropero install models nemotron groot --prio=build qwen --prio=download` resuelve nemotron por pregunta, groot por `build` y qwen por `download` |
| **T-26** | Descarga con `hf`: la secuencia exacta de R-MOD-16, reanudación, fichero vacío | R-MOD-9…20 | T-25 | Con `hf` falso: se llama con los argumentos exactos |
| **T-27** | Reutilización entre modelos y decisión de descargar | R-MOD-21…28 | T-26 | Dos modelos del mismo repo descargan una vez |
| **T-28** | Contrato de stdout: `ROPERO_PORT=` al final, resto a stderr | R-OC-1…5, R-CLI-17 | T-18 | El plugin lee las tres líneas |

#### F6 · Forja

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-29** | **`ensure_toolchain()`: la precondición del motor, antes de la fase 1** | R-GGUF-54…59 🔶 | T-26 | Sin motor: error correcto y **cero bytes descargados** |
| **T-30** | Las seis fases como máquina de estados, con checkpoint por fase | R-GGUF-16…21 | T-29 | Un fallo en la fase 5 **conserva** clon e intermedio |
| **T-31** | La taxonomía de cinco clases de checkpoint y el reintento por fase | R-GGUF-32…36 | T-30 | Se reintenta sólo la fase cara |
| **T-32** | El bloque MTP: la cabeza que el imatrix no cubre | R-GGUF-22…31 | T-30 | El MTP se cuantiza con su propia regla |
| **T-33** | Lectura de la cabecera GGUF (sustituye a `ggufcheck.py`) | R-GGUF-37…46 | T-30 | La cabecera se lee sin cargar el modelo |
| **T-34** | La procedencia: el sidecar que hace el artefacto auditable | R-GGUF-47…51 | T-33 | Un GGUF sin sidecar se rechaza |
| **T-35** | `ropero gguffer`: la CLI completa, con las 6 fases y sus flags | R-GGUF-52…58 | T-34 | La CLI coincide con la del doc A |

#### F7 · La puerta

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-36** | `ropero serve`: axum, `/v1/models`, `/v1/chat/completions`, streaming SSE | R-API-1…11, R-API-19…22 | T-16 | El stream llega token a token, sin bufferizar |
| **T-37** | **Auth: la clave por fichero 0600, nunca en el argv** | R-API-12…14, R-TST-23 | T-36 | `/proc/<pid>/cmdline` no contiene la clave |
| **T-38** | El test de auth **sobre la función**, no sobre el rc de un subproceso | R-TST-19 | T-37 | Romper la auth **pone el test en rojo** (T-2) |
| **T-39** | El enrutado (D3): la selección semántica y su degradación | R-ROUTE-1…5 | T-36 | La misma entrada va siempre al mismo destino |
| **T-40** | Límites por cliente y CORS | R-API-23…25 🔶 | T-36 | 429 con `Retry-After` |

#### F8 · Instalación y opencode

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-41** | `install.sh` v2: symlink-only, idempotente, `--check`, migración del layout viejo | R-INST-1…27 | T-35 | Reglas R1–R13 comprobadas por test |
| **T-42** | `ropero doctor` y `ropero check`: el diagnóstico real | R-INST-13…20 | T-41 | Cada aviso dice **qué hacer** |
| **T-43** | `ropero stats`: CPU/RAM/GPU, con `--watch` y `--json` | R-CLI-14, R-CLI-17 🔶 | T-42 | `doctor` y `stats` comparten la capa de muestreo |
| **T-44** | El plugin de opencode: consume el contrato de stdout | R-OC-1…11 | T-28 | El plugin arranca un modelo y obtiene el puerto |
| **T-45** | La batería de integración en Rust, con los dobles | R-TST-1…28 | T-44 | La batería corre en < 60 s sin GPU |

#### F9 · Convivencia

| Tarea | Qué hace | Cierra | Depende de | Puerta |
|---|---|---|---|---|
| **T-46** | v1 y v2 leen y escriben el mismo estado en disco | — | T-24 | Un estado escrito por v1 lo lee v2 y al revés |
| **T-47** | La guía de migración y la retirada de v1 | — | T-46 | El CHANGELOG dice qué cambió y qué no |
| **T-48** | El informe final: **qué se ha medido**, qué queda pendiente, qué deuda se cierra | — | T-47 | Todas las H-1…H-29 del doc A están cerradas o documentadas |

### 8.4 El grafo de dependencias

```
T-01 ──► T-02, T-03, T-04, T-05
T-05 ──► T-06 ──► T-07, T-08, T-09, T-11
T-09 ──► T-10
T-11 ──► T-12 ──► T-13 ──► T-15
T-12 ──► T-14
T-15 ──► T-16 ──► T-18 ──► T-19 ──► T-20 ──► T-21 ──► T-22
T-20 ──► T-23, T-24
T-08 ──► T-17
T-17 ──► T-18
T-18 ──► T-28
T-09 ──► T-25 ──► T-26 ──► T-27
T-26 ──► T-29 ──► T-30 ──► T-31, T-32, T-33 ──► T-34 ──► T-35
T-16 ──► T-36 ──► T-37 ──► T-38, T-39, T-40
T-35 ──► T-41 ──► T-42 ──► T-43
T-28 ──► T-44 ──► T-45
T-24 ──► T-46 ──► T-47 ──► T-48
```

**T-29 y T-38 son las dos tareas de mayor valor** y pueden ir en paralelo
desde el principio: las dos cierran heridas medidas, y las dos son
independientes del resto.

### 8.5 Qué se puede hacer en paralelo

| Grupo | Tareas | Por qué son independientes |
|---|---|---|
| A | T-07, T-08, T-09, T-11 | Todas dependen sólo de T-06 y tocan ficheros distintos |
| B | T-31, T-32, T-33 | Las tres ramas de la forja (checkpoint, MTP, cabecera) |
| C | T-37, T-39, T-40 | Auth, enrutado y límites: middleware independiente cada uno |
| D | T-22, T-23, T-24 | Parada, locks y estado: mismo crate, ficheros distintos |

### 8.6 El orden de una fase completa

Para cada fase, el ritual es el mismo:

```
1. Leer los requisitos de la fase en el doc A (§ 7.4 da la fila de cada uno).
2. Localizar la evidencia v1 y congelar el golden (§ 8.1).
3. Escribir los tests (rojos).
4. Implementar.
5. Verificar la puerta global (§ 7.2).
6. Actualizar el doc A si algo cambió.
7. PR con la tabla de requisitos cerrados.
```

---
---

## 9. Estrategia de pruebas

### 9.1 La pirámide

| Nivel | Qué prueba | Cuántos | Cuándo corre | Duración objetivo |
|---|---|---|---|---|
| **Unit** | Funciones puras del dominio: argv, validación, estado, checksum, placeholders | ~400 | En cada push | < 5 s |
| **Property** | Invariantes sobre entradas arbitrarias | ~60 | En cada push | < 20 s |
| **Integración** | Procesos reales: arrancar, sondear, parar; el servidor HTTP | ~120 | En cada push | < 60 s |
| **Golden** | Salidas congeladas del v1 | ~80 | En cada push | < 10 s |
| **End-to-end** | La CLI completa contra un motor real | ~25 | Diario + antes de release | < 10 min |
| **Manual** | Hardware real: GPU, NPU, Metal | guion | Antes de cada release | horas |

**El 88 % del trabajo se comprueba en menos de un minuto.** Un ciclo de
verificación lento es un ciclo que no se hace.

### 9.2 Property tests: los invariantes que no deben romperse

Un property test no prueba un ejemplo: prueba una **familia** de entradas. Los
invariantes de ropero:

```rust
// El argv montado NUNCA contiene una cadena vacía donde iba un placeholder.
proptest! {
    #[test]
    fn argv_no_tiene_placeholders_sin_resolver(spec in arbitraria_spec()) {
        let argv = build_argv(&spec, &req, &ctx).unwrap();
        for a in &argv {
            prop_assert!(!a.contains("${"), "placeholder sin resolver: {a}");
        }
    }
}

// El orden del argv es SIEMPRE base → level → flags → options.
proptest! {
    #[test]
    fn argv_respeta_el_orden_fijo(spec in arbitraria_spec(), req in arbitraria_request()) {
        let argv = build_argv(&spec, &req, &ctx).unwrap();
        // el último flag del usuario es el último flag del argv
        if let Some(last) = req.flags.last() {
            let pos = argv.iter().rposition(|a| a.contains(last)).unwrap();
            prop_assert!(pos > 0);
        }
    }
}

// build_argv es puro: dos llamadas con la misma entrada dan el mismo argv.
proptest! {
    #[test]
    fn argv_es_determinista(spec in arbitraria_spec(), req in arbitraria_request()) {
        let a = build_argv(&spec, &req, &ctx).unwrap();
        let b = build_argv(&spec, &req, &ctx).unwrap();
        prop_assert_eq!(a, b);
    }
}

// Ningún nivel no declarado se acepta.
proptest! {
    #[test]
    fn nivel_no_declarado_siempre_falla(spec in arbitraria_spec()) {
        let req = LaunchRequest { level: Some("nivel-inexistente-xyz".into()), .. };
        prop_assert!(build_argv(&spec, &req, &ctx).is_err());
    }
}

// El estado en disco hace round-trip.
proptest! {
    #[test]
    fn estado_hace_roundtrip(st in arbitraria_estado()) {
        let dir = tempdir().unwrap();
        StateStore::new(dir).write(9999, &st).unwrap();
        let leido = StateStore::new(dir).read(9999).unwrap();
        prop_assert_eq!(st, leido);
    }
}
```

**El último es el que más valor tiene**: el estado en disco es la fuente de
verdad (P-4) y un round-trip roto significa que ropero «olvida» un modelo en
plano. Un test de ejemplo no lo encuentra; uno de property, sí.

### 9.3 Los dobles (fakes)

El repo v1 ya tiene `test/fakes/` con `hf`, `curl`, `llama-quantize`,
`convert_hf_to_gguf.py` y `strata` falsos. En v2 pasan a ser **binarios de
Rust** dentro de un crate `ropero-fakes`, con una diferencia importante:

| En v1 (bash) | En v2 (Rust) |
|---|---|
| El doble registra en un fichero de texto | El doble **es** un proceso que responde a un protocolo |
| El test grepa el fichero | El test hace una **pregunta** al doble |
| Si el doble no se invoca, el fichero no existe | Si el doble no se invoca, el test falla al preguntar |

El patrón que hay que evitar es el del wound #1: un doble que responde
«ok» y un test que **no mira la respuesta**. En Rust, el doble se consulta
con una API que devuelve un resultado tipado, y el test afirma sobre ese
resultado.

**El doble de motor** es el más importante: un proceso que abre un socket en
un puerto, responde a `/health` con 200 y a una inferencia con una respuesta
fija. Con él se prueban el arranque, la sonda, el sello y la parada sin GPU.

### 9.4 Cobertura

| Crate | Umbral de líneas | Umbral de ramas | Motivo |
|---|---|---|---|
| `ropero-core` | **95 %** | 90 % | Es puro y todo es testeable. Aquí no hay excusa. |
| `ropero-runtime` | 80 % | 70 % | Hay código que sólo se ejecuta con un motor real. |
| `ropero-download` | 85 % | 75 % | La lógica de reanudación es crítica. |
| `ropero-forge` | 80 % | 70 % | Las fases se prueban con dobles. |
| `ropero-server` | 85 % | 80 % | La auth y el streaming son críticos. |
| `ropero-cli` | 60 % | 50 % | Es cableado; lo importante son los tests de contrato. |

**La cobertura no es el objetivo; es el detector de humo.** Un umbral alto
obliga a que el código sea alcanzable, que es lo que hace posibles los tests
unitarios. El objetivo real es P-14: que los tests **puedan fallar**.

### 9.5 Los tests que faltan hoy (y hay que escribir)

El doc A mide tres heridas. Cada una se cierra con un test **concreto**:

| Herida | El test que la cierra | Por qué el actual no la ciertaba |
|---|---|---|
| Auth verde falso (R-TST-19) | `auth_rechaza_clave_incorrecta` sobre la función, no sobre el rc | El test v1 capturaba `rc=$?` y nunca lo volvía a mirar; `exit 0` en la línea 206 lo hacía inalcanzable; el recuento iba con `+12` hardcodeado |
| Niveles sin test (R-TST-22) | `nivel_llega_al_argv`, `nivel_no_declarado_es_error_2`, `nivel_por_defecto_se_aplica` | El catálogo define niveles y `rp_model_argv` los resuelve, pero ningún test lo comprueba |
| Clave en el argv (R-TST-23) | `la_clave_no_aparece_en_el_cmdline` leyendo `/proc/<pid>/cmdline` | La clave iba en el argv del motor, legible por cualquier usuario del host |

Y una cuarta, que el doc A no lista como herida pero que el análisis de este
documento revela:

| Cuarto riesgo | El test que lo cierra |
|---|---|
| **La precondición del gguffer no se comprueba** (R-GGUF-54) | `gguffer_sin_motor_no_descarga_nada`: sin motor, el error es `NoEngineInstalled` y el `hf` falso **no registra ninguna llamada** |

### 9.6 El CI

```yaml
# .github/workflows/ci.yml — esqueleto
name: ci
on: [push, pull_request]

jobs:
  calidad:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { toolchain: "1.100.0", components: "rustfmt, clippy" }
      - run: cargo fmt --check
      - run: cargo clippy --all-targets --all-features -- -D warnings
      - run: cargo nextest run --all-features --no-fail-fast
      - run: cargo llvm-cov --all-features --lcov --output-path coverage.lcov
      - run: cargo deny check

  macos:
    runs-on: macos-latest   # arm64: el hardware de referencia
    steps: [igual que arriba]

  e2e:
    needs: calidad
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --test e2e -- --ignored   # los que necesitan GPU
```

**El job de macOS no es opcional.** La mitad de los motores sólo existen en
macOS (Metal, MLX) y la restricción de bash 3.2 viene de ahí. Un CI sólo con
Linux no detecta ninguno de los dos problemas.

---

## 10. CI/CD y publicación

### 10.1 Publicación

| Aspecto | Decisión |
|---|---|
| Versionado | SemVer. `0.x` mientras la CLI cambie; `1.0.0` cuando el contrato de stdout y la CLI se declaren estables. |
| Binarios | 4 targets (§ 2.2). Linux con `musl` (estático), macOS nativo. |
| Firmado | `cosign` sobre el tarball, con clave pública en el repo. |
| Checksums | `SHA256SUMS` junto al tarball. |
| SBOM | `cargo cyclonedx` en cada release. |
| Canal | `releases/latest` y `releases/canary` (cada merge a `main`). |
| Instalación | `install.sh` descarga el binario del canal que elija el usuario. |

### 10.2 El `install.sh` v2

El instalador v1 tiene 13 reglas duras (R1–R13) y 580 líneas. La v2:

| Regla v1 | En v2 |
|---|---|
| R1: sólo symlinks | Se mantiene la tesis (S-4), pero el symlink apunta a un **binario único**, no a un árbol de scripts. |
| R2: `~/.local/share/<name>` → `<repo>/bin` | Desaparece: no hay árbol que enlazar. El enlace es a `~/.local/bin/ropero`. |
| R3: idempotencia | Se mantiene, y se **comprueba** con `--check` después de instalar. |
| R4: lo ajeno no se toca | Se mantiene **sin cambios**: un fichero real en `~/.local/bin/ropero` no se sobrescribe. |
| R5: migración del layout viejo | Se mantiene: se detecta el layout v1 y se sustituye **sólo tras** verificar que es nuestro. |
| R6: sin red | **Cambia**: el instalador v2 descarga el binario. El ADR tiene que explicar por qué y con qué comprobación de integridad. |
| R7: flags | `--local`, `--install`, `--check`. Se añaden `--version`, `--uninstall`, `--dir`, `--bin-dir`. |
| R8: salida | Marcadores `[✓]/[!]/[✗]/[-]` a stderr, en español, con recuento. |
| R9: verificación post-install | `ropero --version` y `ropero doctor`. |
| R10: bash 3.2 | Se mantiene: el instalador sigue siendo bash 3.2, porque tiene que correr en una máquina sin Rust. |
| R11: el repo no se contamina | Se mantiene. |
| R12: `--check` read-only | Se mantiene. |
| R13: aviso del PATH | Se mantiene. |

**R6 es el único cambio de fondo**, y por eso necesita un ADR. La razón del
cambio: sin binarios publicados, instalar ropero v2 exigiría un toolchain de
Rust en la máquina del usuario, y eso es una barrera de entrada que un gestor
de modelos no puede tener. El ADR debe recoger la alternativa (dejar el
`install.sh` actual y compilar desde el repo) y por qué se rechaza.

### 10.3 El CHANGELOG

Formato Keep a Changelog, en español, con las categorías del § 6.3. Las
entradas de **cambio de comportamiento** son obligatorias y llevan el
requisito que las provoca:

```markdown
## [2.0.0] — 2026-11-12

### Cambiado
- El estado en disco añade `STARTTIME` y `EXEDIGEST`. Un ropero v1 puede leer
  un estado v2 (ignora las claves que no conoce); un ropero v2 lee un estado v1
  pero lo trata como «ajeno» hasta verificar la identidad. (R-EST-8, R-ARQ-9)
- `ropero gguffer` comprueba la precondición del motor **antes** de descargar.
  Sin motor instalado, antes de la descarga, ahora falla con el motivo real.
  (R-GGUF-54…59)
```

---

## 11. Seguridad

### 11.1 La superficie de ataque

ropero corre en la máquina del usuario, con sus permisos, y lanza procesos que
consumen la GPU. Las amenazas reales son:

| Amenaza | Impacto | Mitigación |
|---|---|---|
| **Robo de la clave de la API** | Acceso al modelo por la red local | Clave por fichero 0600, nunca en el argv (§ 3.6) |
| **Matar un proceso ajeno** | Pérdida del trabajo del usuario | Identidad `pid + start_time + digest` (§ 3.3.3) |
| **Modelos ajenos en la GPU** | El modelo no arranca, o va lento | Detección de motores ajenos (H-26, ver § 11.4) |
| **Descarga de un artefacto manipulado** | Un GGUF arbitrario en disco | Verificación de checksum + procedencia (§ 11.3) |
| **Path traversal en un TOML** | Escribir fuera del directorio de modelos | Validación de rutas: todo path relativo, y se normaliza y se comprueba que no escapa (§ 11.2) |
| **Comando inyectado por un campo** | Ejecución arbitraria | **El TOML es dato, nunca se evalúa** (P-6). El argv se construye como vector, sin shell. |
| **Secretos en los logs** | Filtración | `tracing` con un filtro que redacta; test que comprueba que la clave no aparece en el log |

### 11.2 Rutas

**Regla**: ningún path del TOML es absoluto. `${gguf}` se resuelve siempre
dentro de `$GGUF_DIR/<modelo>/`. Si un TOML trae una ruta absoluta, el
validador la **rechaza** (no la normaliza): el error es más claro que
«ropero no puede escribir en /etc».

Si un path contiene `..`, se rechaza en la validación, no en la escritura.

### 11.3 Procedencia y verificación

Un GGUF que ropero descarga o fabrica lleva un **sidecar** de procedencia:

```toml
# Groot-65B-A6B-GyroM-Q2_0.gguf.provenance.toml
model = "groot"
source_repo = "BoxyFit/Groot-65B-A6B-GyroM"
source_files = ["Groot-65B-A6B-GyroM-Q2_0.gguf"]
quant = "Q2_0"
imatrix = "imatrix.dat"
sha256 = "…"
built_at = "2026-11-12T18:45:12Z"
built_by = "ropero 2.0.0"
engine = "llama-server @ <commit>"
```

**Un GGUF sin sidecar no se considera de ropero.** `ropero install models` lo
avisa y `ropero doctor` lo señala. Es lo que hace que un artefacto sea
**auditable**: se puede responder de dónde salió y con qué se construyó, sin
preguntar a nadie.

El `sha256` es del fichero final y se comprueba **antes** de usarlo: si el
fichero cambió en disco, es otro fichero.

### 11.4 La pregunta abierta: los motores ajenos

El doc A la deja explícitamente abierta (R-EST-15 🔶, H-26) y este documento
**no la cierra por decreto**. Lo que sí hace es fijar el marco:

- **Informar y parar son cosas distintas.** Detectar un motor ajeno es
  barato; pararlo no lo es y no se hace.
- **En Linux** hay dato directo: `nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv,noheader`. La lista de PIDs que usan la GPU se cruza con los PIDs que ropero gestiona, y la diferencia son los ajenos.
- **En macOS** no hay equivalente directo. `ioreg` da información del sistema, no de los procesos de cálculo. La detección en macOS queda **abierta** y se documenta como no resuelta.

**Lo que sí se implementa en F4**, porque es lo que evita el síntoma más
molesto («ropero no arranca y no dice por qué»): cuando el puerto está ocupado
por un proceso que **no** es de ropero, el error lo dice explícitamente:

```
ERROR: el puerto 9999 está ocupado por un proceso ajeno (pid 42137, llama-server)
       ropero no lo va a parar: no es suyo.
       Opciones: usar otro puerto (--port 9997), o parar ese proceso tú mismo.
```

Eso es P-2 y P-13 aplicados, y convierte un «no arranca» sin explicación en un
problema con nombre.

---

## 12. Observabilidad

### 12.1 Trazas

`tracing` con dos formatos: `pretty` para un humano, `json` para una máquina.
El nivel se fija con `ROPERO_LOG=debug|trace|info|warn|error`.

Cada operación larga emite un span con su duración:

```
INFO ropero::runtime::supervisor: arrancando motor
  ├─ model=groot engine=llama-server port=9999 level=high ctx=65536
  ├─ 04-launch.sh:216 elapsed=42.1s
  └─ probe=inference elapsed=3.4s status=ready
```

**El tiempo de sonda va en la traza.** Cuando un modelo tarda 90 segundos en
arrancar, la pregunta es siempre «¿cuánto tardó el motor y cuánto tardó
ropero en darse cuenta?». Sin esa separación, no hay diagnóstico.

### 12.2 `ropero doctor`

El `doctor` v2 es el instrumento de diagnóstico, y su salida es **accionable**
por construcción (P-13). Cada línea tiene una de estas formas:

| Forma | Significado |
|---|---|
| `[✓] <comprobación>` | Pasa |
| `[✗] <comprobación> — <motivo> — <acción>` | Falla, con motivo y acción |
| `[!] <comprobación> — <motivo>` | Aviso: no bloquea, pero conviene saberlo |
| `[-] <comprobación>` | No se ha podido comprobar (y por qué) |

**Un `[✗]` sin acción es un bug del `doctor`.** Si el `doctor` dice que algo
está mal y no dice qué hacer, el usuario tiene que investigar lo que ropero
ya sabe.

Las comprobaciones del `doctor` v2:

1. El binario y su versión.
2. El directorio de modelos y cuántos TOML hay (activos y `fired/`).
3. Los TOML que **no validan** (con fichero y línea).
4. Los motores instalados y los compatibles con esta máquina.
5. Los motores **instalados pero no compatibles** (con el motivo).
6. Los motores **compatibles pero no instalados**.
7. Los puertos gestionados y su estado real (sonda).
8. Los puertos ocupados por procesos ajenos.
9. Los modelos cuyo fichero no está o está vacío.
10. Los modelos cuyo GGUF no tiene sidecar de procedencia.
11. El espacio en disco disponible frente a lo que necesitan los modelos.
12. La RAM frente a `[limits].min_ram_gb`.
13. La clave de la API: presente, ausente o con permisos demasiado abiertos.
14. El plugin de opencode: instalado y coherente con la versión.

### 12.3 `ropero stats`

Muestrea CPU, RAM y GPU del proceso del motor y del sistema. Sin GPU es
información del proceso y del sistema; con GPU, también de la GPU (vía
`nvidia-smi` en Linux, `ioreg`/`powermetrics` en macOS cuando sea viable).

**`doctor` y `stats` comparten la capa de muestreo** (R-CLI-17 🔶): las cifras
de uno y de otro no pueden divergir, porque salen del mismo código.

---

## 13. Lo que este documento deja abierto

Un documento que finge cerrar todo es un documento que esconde los huecos.
Estos son los reales:

| # | Hueco | Por qué no se cierra aquí | Qué haría falta |
|---|---|---|---|
| H-26 | Detección de motores ajenos en macOS | No hay API directa | Un método fiable, o aceptarlo como no resuelto y decirlo (§ 11.4) |
| H-a | Distribución fuera del repo (Homebrew, AUR) | Decisión del dueño | Una frase suya |
| H-b | Política de actualizaciones de dependencias | Depende de quién mantiene | Cadencia acordada (semanal/mensual) |
| H-c | Soporte de más de dos sistemas | No está en el alcance | Decisión del dueño |
| H-d | internationalización de los mensajes | El repo actual es en español y no lo pide | Decisión del dueño |
| H-e | Métricas persistentes (histórico de uso) | Nadie las ha pedido | Un formato y un sitio donde guardarlas |

**Los cinco están anotados en el doc A como deuda abierta** (o como algo que
el doc A no cubre). Ninguno bloquea el plan: las diez fases se pueden
ejecutar sin cerrarlos.

---

## 14. Glosario

| Término | Significado |
|---|---|
| **motor** | Un programa externo que sirve un modelo: llama.cpp, MLX, TensorRT-LLM, vLLM, AirLLM, FLM, Strata. |
| **modelo** | Un fichero de datos (TOML) que declara qué servir, con qué motor y con qué argumentos. |
| **artefacto** | El fichero que se sirve: un GGUF, un checkpoint, un paquete de Strata. |
| **sonda** | La comprobación de si un motor está listo. Tres niveles: TCP, HTTP, inferencia. |
| **sello** (stamp) | Hash del modelo+contexto+nivel+ficheros. Detecta que la configuración ha cambiado. |
| **estado** | El fichero en `$STATE_DIR/ports/<puerto>` que dice qué está corriendo. |
| **identidad** | La terna `pid + start_time + digest del binario` que prueba que un proceso es el nuestro. |
| **procedencia** | El sidecar que dice de dónde salió un GGUF y con qué se construyó. |
| **checkpoint** | El registro de hasta dónde llegó una fase de la forja, para poder reintentar sólo lo caro. |
| **puerta** | La Definition of Done: qué se comprueba y dónde. |
| **golden master** | El comportamiento v1 congelado como valor esperado de un test. |
| **ADR** | Architecture Decision Record: por qué se decidió algo. |
| **wound** | Herida medida: un defecto del repo v1 que la batería no detecta. |

---

## 15. Procedencia de este documento

**Fecha**: 2026-11-12.
**Fuente primaria**: el repo `lorenzo-sf/ropero` en el commit `f21b442`,
147 ficheros, 14.839 líneas de bash y 4.803 de Python, 8 motores, 45 TOML
(17 activos + 28 retirados), 16 ficheros de test.
**Fuente secundaria**: `ropero-especificacion.md`, con 317 requisitos y 281 de
ellos con evidencia `fichero:línea`.
**Versiones**: verificadas contra el índice de crates.io y
`static.rust-lang.org` el 2026-11-12 (§ 2.2, § 2.3).
**Entregable**: este documento y `ropero-especificacion.md`, leídos juntos.

### 15.1 Lo que este documento **no** verificó

- **Que el código de v1 cumpla todos sus requisitos.** El doc A describe lo
  que el código hace; donde el código y el doc A discrepan, el doc A lo señala
  como deuda. Este documento no ha re-ejecutado la batería de v1.
- **Que las versiones de crates funcionen juntas.** Se verificaron las
  versiones **existentes**; la combinación concreta se valida en F0.
- **Que el plan de fases sea el óptimo.** Es un plan **ejecutable**, no
  necesariamente el más corto. Se puede reordenar si aparece un dato nuevo.
- **Que el hardware de referencia siga siendo el mismo.** La sección 3.3 del
  doc A describe la máquina del dueño en el momento del análisis.
