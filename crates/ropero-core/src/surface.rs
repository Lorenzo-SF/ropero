//! La **superficie** del CLI v2: sus comandos, flags y frases.
//!
//! El doc normativo ([`docs/especificacion/ropero-cli.md`](../../../docs/especificacion/ropero-cli.md))
//! fija once invocaciones, sus opciones y sus códigos de salida. Este módulo
//! es la **única** traducción de ese contrato al código: los nombres, las
//! frases de ayuda, la forma de cada flag y los mensajes de "todavía no"
//! viven aquí, y la capa de presentación los usa para construir el parser.
//!
//! ## Por qué aquí y no en la CLI
//!
//! Porque la ayuda no puede mentir. Si la frase de un comando viviera en
//! el `#[command(about = "…")]` de la CLI, nadie podría comprobar que lo que
//! la ayuda dice es lo que el programa hace: sólo se vería lanzando el
//! binario. Al vivir aquí, un test puede recorrer la tabla de comandos y
//! afirmar que **todos** tienen frase, que ninguno está duplicado y que
//! las invocaciones pendientes dicen **qué hacer**.
//!
//! ## Los tres estados de un comando
//!
//! - [`CommandState::Ready`]: el comando funciona.
//! - [`CommandState::Parsed`]: el comando se **parsea** (el parser lo conoce
//!   y sus flags son los del contrato) pero su cuerpo todavía no existe; el
//!   programa responde con [`pending_message`] y sale con 1.
//! - [`CommandState::Phase`]: el comando pertenece a una fase posterior (el
//!   arranque del modelo es de F3); su mensaje dice en qué fase llega.
//!
//! Ningún comando queda **roto**: un comando que se parsea pero no se
//! implementa dice qué falta y sale con 1, no con un error de uso (2) ni con
//! un panic.

use std::fmt;

/// El estado de un comando del contrato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandState {
    /// El comando está implementado y funciona.
    Ready,
    /// El comando se parsea con sus flags del contrato, pero su cuerpo
    /// todavía no está: responde con un mensaje y sale con 1.
    Parsed,
    /// El comando pertenece a una fase posterior: su mensaje dice en qué
    /// fase llega y qué hacer entonces.
    Phase,
}

/// Un comando del contrato del CLI v2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// El nombre con el que se invoca.
    pub name: &'static str,
    /// La forma de invocarlo, tal y como se muestra en la ayuda.
    pub usage: &'static str,
    /// Qué hace, en **una** línea (doc CLI v2 §7).
    pub summary: &'static str,
    /// La fase en la que llega, si el comando no está listo todavía.
    pub phase: Option<&'static str>,
    /// El estado del comando.
    pub state: CommandState,
}

impl Command {
    /// Si el comando está listo para ejecutarse.
    pub fn es_ready(&self) -> bool {
        self.state == CommandState::Ready
    }
}

/// Los comandos del contrato del CLI v2.
///
/// La lista es la verdad: la ayuda se genera a partir de ella, así que no
/// puede anunciar un comando que no exista, ni dejar de anunciar uno que sí.
pub const COMMANDS: &[Command] = &[
    Command {
        name: "install",
        usage: "install [engines|models] [modelo...]",
        summary: "instala motores y modelos",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "stop",
        usage: "stop [modelo]",
        summary: "detiene los motores lanzados por ropero",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "gguffer",
        usage: "gguffer --repo OWNER/NAME",
        summary: "fabrica un GGUF a partir de pesos o de un repo",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "status",
        usage: "status",
        summary: "estado de la máquina: CPU, GPU, RAM y swap",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "doctor",
        usage: "doctor",
        summary: "comprueba las dependencias de ropero y de cada motor",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "config",
        usage: "config",
        summary: "muestra la configuración, las rutas y su origen",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "connect",
        usage: "connect (--list | --model MODELOS --agent AGENTE [--remove])",
        summary: "conecta o desconecta modelos de agentes de código",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "models",
        usage: "models [--all | --fired] [add]",
        summary: "modelos disponibles, agrupados por motor",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "model",
        usage: "model <modelo> (--fired | --hired)",
        summary: "despide o reactiva un modelo sin borrar su TOML",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "template",
        usage: "template",
        summary: "muestra la plantilla del modelo y sugiere dónde guardarla",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "engines",
        usage: "engines [--all]",
        summary: "motores, con su color y su compatibilidad",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "engine",
        usage: "engine --params [ENGINE]",
        summary: "muestra los parámetros comunes configurados de un motor",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "validate",
        usage: "validate",
        summary: "valida los TOML del catálogo",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "command",
        usage: "command <modelo>",
        summary: "comando completo de arranque, listo para copiar",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "info",
        usage: "info <modelo>",
        summary: "configuración, artefactos y argumentos del modelo",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "test",
        usage: "test [--model <ids> | --all-models] [--test <ids>] [--test-group <groups>] [--all-tests | --all-test] [--path <path>]",
        summary: "lanza la batería de pruebas contra un modelo",
        phase: None,
        state: CommandState::Ready,
    },
];

/// La invocación del arranque de un modelo: `ropero <modelo>`.
///
/// No es un comando del [`COMMANDS`] porque **no** es una palabra fija: lo
/// que sigue al binario es el **nombre del modelo**. Aun así es parte del
/// contrato (doc CLI v2 §8) y por eso tiene su propia entrada.
pub const RUN_USAGE: &str = "run <modelo> [--force] [--cpu] [--port N] [--api-key CLAVE] [--no-api-key] [--ctx N] [--log-level NIVEL] [--background]";

/// La frase de `ropero <modelo>` en la ayuda.
pub const RUN_SUMMARY: &str = "arranca el modelo en su motor";

/// La invocación de eliminación de un modelo: `ropero <modelo> remove`.
pub const REMOVE_USAGE: &str = "<modelo> remove";

/// La frase de `ropero <modelo> remove` en la ayuda.
pub const REMOVE_SUMMARY: &str = "elimina los artefactos descargados de un modelo";

/// La frase de `ropero` a secas (doc CLI v2 §6): es una ayuda breve.
pub const RESUMEN_SUMMARY: &str = "resumen de la máquina: motores y modelos";

// ─────────────────────────────────────────────────────────────────────────────
// Los flags
// ─────────────────────────────────────────────────────────────────────────────

/// La forma de un flag del contrato.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagKind {
    /// Un interruptor: `--force`.
    Switch,
    /// Un valor: `--port 8100`.
    Value,
    /// Un valor repetible: `--quant q4_k_m --quant q5_k_m`.
    Many,
}

/// Un flag del contrato: su nombre, su forma y su frase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flag {
    /// El nombre largo, sin los guiones.
    pub name: &'static str,
    /// La forma del flag.
    pub kind: FlagKind,
    /// Qué hace, en una línea.
    pub help: &'static str,
    /// El nombre del valor en la ayuda (`PORT`, `OWNER/NAME`…).
    pub value_name: Option<&'static str>,
    /// El valor por defecto, si lo tiene.
    pub default: Option<&'static str>,
    /// Si el flag necesita otro (los de la imatrix van en pareja).
    pub requires: Option<&'static str>,
}

impl Flag {
    /// Un interruptor.
    pub const fn switch(name: &'static str, help: &'static str) -> Self {
        Flag {
            name,
            kind: FlagKind::Switch,
            help,
            value_name: None,
            default: None,
            requires: None,
        }
    }

    /// Un flag con valor.
    pub const fn value(name: &'static str, value_name: &'static str, help: &'static str) -> Self {
        Flag {
            name,
            kind: FlagKind::Value,
            help,
            value_name: Some(value_name),
            default: None,
            requires: None,
        }
    }

    /// Un flag con valor repetible.
    pub const fn many(name: &'static str, value_name: &'static str, help: &'static str) -> Self {
        Flag {
            name,
            kind: FlagKind::Many,
            help,
            value_name: Some(value_name),
            default: None,
            requires: None,
        }
    }
}

/// Los flags de `ropero install`.
///
/// El `--prio` es **repetible**: puede ser global (antes de la lista) o
/// por modelo (detrás del modelo), y en una invocación puede haber varios
/// (doc B §4.10.7).
pub const FLAGS_INSTALL: &[Flag] = &[
    Flag::many(
        "prio",
        "VIA",
        "la vía de instalación: download, build o tool (repetible)",
    ),
    Flag::switch(
        "force",
        "borra el código fuente de los motores seleccionados y reinstala",
    ),
];

/// Los flags de `ropero stop`.
pub const FLAGS_STOP: &[Flag] = &[];
pub const FLAGS_CONNECT: &[Flag] = &[
    Flag::switch("list", "lista agentes y su tipo de integración"),
    Flag::value(
        "model",
        "MODELOS",
        "modelo(s) separados por coma o punto y coma",
    ),
    Flag::value("agent", "AGENTE", "agente de destino"),
    Flag::switch("remove", "quita la conexión indicada"),
];

/// Los flags de `ropero gguffer` (doc CLI v2 §11.2).
pub const FLAGS_GGUFFER: &[Flag] = &[
    Flag::switch(
        "turbo-llama",
        "usa las herramientas compiladas del motor Turbo-Llama (alias --turbo)",
    ),
    Flag::value("repo", "OWNER/NAME", "repo de HuggingFace de origen"),
    Flag::value("path", "DIR", "directorio de pesos locales de origen"),
    Flag::value(
        "gguf",
        "FICHERO|OWNER/REPO",
        "GGUF ya hecho, de donde cuantizar",
    ),
    Flag::many("quant", "TIPO", "tipo(s) de cuantización"),
    Flag::value("imatrix", "FICHERO", "matriz de importancia local"),
    // El par remoto va **siempre junto**, pero lo comprueba el dominio
    // (ropero-forge), no `clap`: el mensaje tiene que ser el del doc, en
    // español, y el que dice **qué hacer** (P-13).
    Flag::value(
        "imatrix-repo",
        "OWNER/NAME",
        "repo de la matriz de importancia remota (con --imatrix-file)",
    ),
    Flag::value(
        "imatrix-file",
        "NOMBRE",
        "fichero de la matriz de importancia remota (con --imatrix-repo)",
    ),
    Flag::value("out", "NOMBRE|RUTA", "nombre o ruta del GGUF de salida"),
    Flag::value(
        "out-principal",
        "RUTA",
        "ruta exacta del GGUF del primer quant",
    ),
    Flag {
        name: "intermedio",
        kind: FlagKind::Value,
        help: "formato intermedio",
        value_name: Some("F16|BF16"),
        default: Some("f16"),
        requires: None,
    },
    Flag::switch("keep-source", "no borra los pesos de origen"),
    Flag::switch("keep-intermedio", "no borra el intermedio"),
    Flag::switch("no-mtp", "genera el GGUF sin cabeceras MTP"),
    Flag::switch("no-dflash", "genera el GGUF sin cabeceras DFlash"),
    Flag::switch("force", "sobrescribe y fuerza"),
    Flag::switch("list-quants", "lista los tipos de cuantización disponibles"),
    Flag::switch(
        "diagnostico",
        "modo diagnóstico: enseña qué haría, sin escribir nada",
    ),
];

/// Los flags compartidos por `ropero models` y `ropero engines`.
pub const FLAGS_ALL: &[Flag] = &[Flag::switch(
    "all",
    "muestra también engines no instalados y modelos no disponibles",
)];
pub const FLAGS_ENGINE: &[Flag] = &[Flag::switch(
    "params",
    "muestra los argumentos comunes configurados",
)];
pub const FLAGS_CONFIG: &[Flag] = &[Flag::value("lang", "ES|EN", "idioma de la interfaz")];
pub const FLAGS_STATUS: &[Flag] = &[Flag::value(
    "live",
    "SEGUNDOS",
    "actualiza la vista; intervalo en segundos (por defecto 1)",
)];
pub const FLAGS_MODELS_ADD: &[Flag] = &[
    Flag::value("path", "TOML", "lee el TOML desde un fichero"),
    Flag::switch("stdin", "lee el TOML completo desde la entrada estándar"),
    Flag::value("name", "ID", "nombre interno y nombre del fichero"),
    Flag::value("engine", "ENGINE", "engine del modelo"),
    Flag::value("format", "FORMATO", "formato del artefacto"),
    Flag::value("artifact", "RUTA", "artefacto del modelo"),
    Flag::value("repo", "OWNER/NAME", "repositorio de descarga o build"),
    Flag::value("context", "TOKENS", "ventana de contexto"),
    Flag::value("alias", "ID", "alias público del modelo"),
    Flag::many(
        "set",
        "CAMPO=VALOR",
        "valor TOML adicional; repetible y con rutas por bloques",
    ),
];
pub const FLAGS_MODELS: &[Flag] = &[
    Flag::switch("all", "muestra también lo que no está disponible"),
    Flag::switch("fired", "muestra sólo los modelos despedidos"),
    Flag::value("path", "TOML", "lee el TOML desde un fichero"),
    Flag::switch("stdin", "lee el TOML completo desde la entrada estándar"),
    Flag::value("name", "ID", "nombre interno y nombre del fichero"),
    Flag::value("engine", "ENGINE", "engine del modelo"),
    Flag::value("format", "FORMATO", "formato del artefacto"),
    Flag::value("artifact", "RUTA", "artefacto del modelo"),
    Flag::value("repo", "OWNER/NAME", "repositorio de descarga o build"),
    Flag::value("context", "TOKENS", "ventana de contexto"),
    Flag::value("alias", "ID", "alias público del modelo"),
    Flag::many(
        "set",
        "CAMPO=VALOR",
        "valor TOML adicional; repetible y con rutas por bloques",
    ),
];
pub const FLAGS_MODEL: &[Flag] = &[
    Flag::switch("fired", "desactiva el modelo y conserva su TOML"),
    Flag::switch("hired", "reactiva el modelo desde fired/"),
];

/// Los flags de `ropero <modelo>` (doc CLI v2 §8.2).
pub const FLAGS_RUN: &[Flag] = &[
    Flag::switch(
        "force",
        "mata los procesos de ropero que ocupan el device y carga lo pedido",
    ),
    Flag::switch("cpu", "fuerza el offload 100 % a CPU/RAM"),
    Flag::value("port", "PUERTO", "fuerza el puerto"),
    Flag::value("api-key", "CLAVE", "fija la clave de API del gateway"),
    Flag::switch("no-api-key", "arranca sin clave de API"),
    Flag::value("ctx", "N", "fuerza el contexto"),
    Flag::value(
        "log-level",
        "NIVEL",
        "nivel de log: error, warn, info, debug o trace",
    ),
    Flag::switch(
        "background",
        "arranca desacoplado, sin salida a la terminal, y guarda los logs",
    ),
];

/// Los flags de `ropero test`.
pub const FLAGS_TEST: &[Flag] = &[
    Flag::value(
        "model",
        "MODELOS",
        "modelos separados por comas; se prueban en serie",
    ),
    Flag::switch("all-tests", "ejecuta todos los tests por modelo"),
    Flag::switch(
        "all-test",
        "ejecuta todos los tests en todos los modelos disponibles",
    ),
    Flag::switch(
        "all-models",
        "prueba en serie todos los modelos disponibles y detiene cada engine al terminar; excluyente con --model",
    ),
    Flag::value("test-group", "GRUPOS", "grupos separados por comas"),
    Flag::value(
        "test",
        "IDS",
        "tests separados por comas (se combinan con grupos)",
    ),
    Flag::value(
        "path",
        "RUTA",
        "directorio donde guardar los resultados (por defecto: directorio actual)",
    ),
];

/// Los flags de cada comando, por nombre de comando.
///
/// Es la tabla que el parser consulta: así el contrato de los flags vive
/// en el dominio y el parser no lo duplica.
pub fn flags_de(cmd: &str) -> &'static [Flag] {
    match cmd {
        "install" => FLAGS_INSTALL,
        "stop" => FLAGS_STOP,
        "connect" => FLAGS_CONNECT,
        "gguffer" => FLAGS_GGUFFER,
        "models" => FLAGS_MODELS,
        "model" => FLAGS_MODEL,
        "engines" => FLAGS_ALL,
        "engine" => FLAGS_ENGINE,
        "config" => FLAGS_CONFIG,
        "status" => FLAGS_STATUS,
        "run" => FLAGS_RUN,
        "test" => FLAGS_TEST,
        _ => &[],
    }
}

/// Las frases de ayuda de los comandos, por nombre.
///
/// Son las que la CLI pasa a `clap` (como `about`): así el `--help` y la
/// ayuda breve dicen **lo mismo**, y ambos salen del contrato.
pub mod help {
    /// `ropero install`
    pub const INSTALL: &str = "instala motores y modelos";
    /// `ropero stop`
    pub const STOP: &str = "detiene los motores lanzados por ropero";
    /// `ropero gguffer`
    pub const GGUFFER: &str = "fabrica un GGUF a partir de pesos o de un repo";
    /// `ropero status`
    pub const STATUS: &str = "estado de la máquina: CPU, GPU, RAM y swap";
    /// `ropero doctor`
    pub const DOCTOR: &str = "comprueba las dependencias de ropero y de cada motor";
    /// `ropero models`
    pub const LIST: &str = "modelos disponibles, agrupados por motor";
    /// `ropero template`
    pub const TEMPLATE: &str = "vuelca la plantilla del catálogo";
    /// `ropero engines`
    pub const ENGINES: &str = "motores, con su color y su compatibilidad";
    /// `ropero validate`
    pub const VALIDATE: &str = "valida los TOML del catálogo";
    /// `ropero command`
    pub const ARGS: &str = "argv final con el que se sirve el modelo";
    /// `ropero info`
    pub const LEVELS: &str = "niveles de razonamiento declarados";
    /// `ropero info`
    pub const PATH: &str = "rutas resueltas del modelo";
    /// `ropero test`
    pub const TEST: &str = "lanza la batería de pruebas contra un modelo";
    /// `ropero <modelo>`
    pub const RUN: &str = "arranca el modelo en su motor";
}

/// El mensaje de un comando que todavía no está implementado.
///
/// El mensaje dice **qué falta** y **qué hacer** (P-13): es un error de
/// ejecución (código 1), no un error de uso, porque el uso es correcto.
pub fn pending_message(cmd: &str) -> String {
    format!(
        "error: '{cmd}' todavía no está implementado\n  \
         (la fase de ejecución de modelos llega después; \
         por ahora los datos del catálogo ya están disponibles)"
    )
}

/// El mensaje de un comando que pertenece a una fase posterior.
///
/// A diferencia de [`pending_message`], aquí se dice **en qué fase** llega:
/// el usuario sabe que no es un olvido, sino el orden del trabajo.
pub fn phase_message(cmd: &str, fase: &str) -> String {
    format!("error: '{cmd}' llega en la {fase}\n  (nada que hacer todavía: espera a esa fase)")
}

/// La ayuda breve de `ropero` a secas (doc CLI v2 §6).
///
/// Se compone de las partes que el dominio ya sabe: aquí sólo vive la
/// **estructura** (los rótulos y el esqueleto), no los datos.
pub fn resumen_esqueleto() -> String {
    let mut out = String::new();
    out.push_str("ropero · motores y modelos de inferencia\n\n");
    out.push_str("build:  {version} · {fecha}\n");
    out.push_str("autor:  {autor}\n\n");
    out.push_str("MOTORES\n");
    out.push_str("  disponibles: {motores_disponibles}\n");
    out.push_str("  instalados:  {motores_instalados}\n\n");
    out.push_str("MODELOS\n");
    out.push_str("  disponibles: {modelos_disponibles}\n");
    out.push_str("  instalados:  {modelos_instalados}\n\n");
    out.push_str("más: ropero --help\n");
    out
}

/// La ayuda completa de `ropero --help` (doc CLI v2 §7).
///
/// Una frase por comando, y al final la sugerencia de `ropero <comando>
/// --help`.
pub fn help_text() -> String {
    let mut out = String::from("USO: ropero <comando> [opciones]\n\nCOMANDOS\n");
    for comando in COMMANDS {
        out.push_str(&format!("  {}\n    {}\n", comando.usage, comando.summary));
    }
    out.push_str(&format!("  {RUN_USAGE}\n    {RUN_SUMMARY}\n"));
    out.push_str(&format!("  {REMOVE_USAGE}\n    {REMOVE_SUMMARY}\n"));
    out.push_str("  ropero --help\n    muestra esta ayuda\n");
    out.push_str("\nAyuda detallada: ropero <comando> --help\n");
    out
}

/// El mensaje de error de un comando desconocido (código 2).
pub fn unknown_command_message(token: &str) -> String {
    let kind = if token.starts_with('-') {
        "opción"
    } else {
        "comando"
    };
    format!("error: {kind} desconocido: '{token}'\n  (ropero --help → uso completo)")
}

/// El mensaje de un error de uso, con la pista de dónde está la ayuda.
pub fn usage_message(cmd: &str, msg: &str) -> String {
    let mut out = String::new();
    if !msg.is_empty() {
        out.push_str("error: ");
        out.push_str(msg);
        out.push('\n');
    }
    if cmd.is_empty() {
        out.push_str("  (ropero --help → uso completo)");
    } else {
        out.push_str(&format!("  (ropero {} --help → uso de {})", cmd, cmd));
    }
    out
}

/// El mensaje de un comando al que le falta el modelo.
pub fn missing_model_message(cmd: &str) -> String {
    format!("falta el modelo (ropero {} <modelo>)", cmd)
}

/// El mensaje de un modelo que no está en el catálogo.
pub fn unknown_model_message(name: &str) -> String {
    format!("modelo desconocido: '{}' (ropero models)", name)
}

/// Un token que no tiene forma de `--<nivel>`, `--<flag>` ni `--<opcion>=<valor>`.
pub fn invalid_token_message(token: &str) -> String {
    format!(
        "se esperaba --<nivel>|--<flag>|--<opt>=<v> (recibido: '{}')",
        token
    )
}

/// El aviso de nivel repetido: gana el último.
pub fn repeated_level_notice(previous: &str, current: &str) -> String {
    format!("aviso: nivel repetido: '{previous}' → '{current}' (gana el último)")
}

/// El mensaje de error cuando el argv no se puede montar.
pub fn argv_error_message(error: &str) -> String {
    format!("error: {error}\n  (ropero command --help → uso de command)")
}

/// Un marcador de fase, para los tests: el nombre de la fase actual.
pub const FASE_ACTUAL: &str = "F3 (arranque de modelos)";

/// Implementa `Display` para [`CommandState`]: su nombre legible.
impl fmt::Display for CommandState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let texto = match self {
            Self::Ready => "listo",
            Self::Parsed => "parseado",
            Self::Phase => "fase posterior",
        };
        write!(f, "{texto}")
    }
}

/// Interactive key administration for the engine serving the selected model.
pub const CONFIG_KEYS_USAGE: &str = "sudo ropero config keys --model <MODEL> [--set]";

pub fn valor_ingles(value: &str) -> &'static str {
    match value {
        "VALOR" => "VALUE",
        "MODELO" => "MODEL",
        "MODELOS" => "MODELS",
        "AGENTE" => "AGENT",
        "FICHERO" => "FILE",
        "FORMATO" => "FORMAT",
        "NOMBRE" => "NAME",
        "SEGUNDOS" => "SECONDS",
        "NIVEL" => "LEVEL",
        "RUTA" => "PATH",
        "VIA" => "METHOD",
        "ID|FICHERO" => "ID|FILE",
        _ => "VALUE",
    }
}

pub fn uso_ingles(usage: &str) -> String {
    usage
        .replace("<MODELOS>", "<MODELS>")
        .replace("<MODELO>", "<MODEL>")
        .replace("<AGENTE>", "<AGENT>")
        .replace("<FICHERO>", "<FILE>")
        .replace("<FORMATO>", "<FORMAT>")
        .replace("<NOMBRE>", "<NAME>")
        .replace("MODELOS", "MODELS")
        .replace("AGENTE", "AGENT")
        .replace("CLAVE", "KEY")
        .replace("NIVEL", "LEVEL")
        .replace("<modelo>", "<model>")
        .replace("[modelo]", "[model]")
        .replace("<comando>", "<command>")
        .replace("[modelo... ]", "[model...]")
        .replace("[modelo...]", "[model...]")
}

pub fn resumen_ingles(command: &str) -> &'static str {
    match command {
        "install" => "installs engines and models",
        "stop" => "stops engines launched by ropero",
        "gguffer" => "builds a GGUF from weights or a repository",
        "status" => "machine status: CPU, GPU, RAM, swap, and zram",
        "doctor" => "checks ropero and engine dependencies",
        "config" => "shows configuration, paths, and their source",
        "connect" => "connects or removes models from coding agents",
        "models" => "lists available models grouped by installed engine",
        "model" => "fires or hires a model without deleting its TOML",
        "template" => "prints the model template and suggested location",
        "engines" => "lists engines and their compatibility",
        "engine" => "shows an engine's configured common parameters",
        "validate" => "validates catalog TOML files",
        "command" => "prints model serving arguments for copying",
        "info" => "shows model configuration, artifacts, and arguments",
        "test" => "runs the evaluation suite sequentially and stops each engine afterward",
        "<modelo>" => "starts the model using its configured engine",
        _ => "shows help",
    }
}
