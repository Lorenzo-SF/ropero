//! La **superficie** del CLI v2: los once comandos, sus flags y sus frases.
//!
//! El doc normativo ([`docs-especificacion/ropero-cli.md`](../../docs-especificacion/ropero-cli.md))
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

/// Los once comandos del contrato del CLI v2.
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
        state: CommandState::Parsed,
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
        name: "list",
        usage: "list [--all]",
        summary: "modelos disponibles, agrupados por motor",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "template",
        usage: "template",
        summary: "vuelca la plantilla del catálogo",
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
        name: "validate",
        usage: "validate",
        summary: "valida los TOML del catálogo",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "args",
        usage: "args <modelo> [--<nivel>] [--<flag>] [--<opcion>=<valor>]",
        summary: "argv final con el que se sirve el modelo",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "levels",
        usage: "levels <modelo>",
        summary: "niveles de razonamiento declarados",
        phase: None,
        state: CommandState::Ready,
    },
    Command {
        name: "path",
        usage: "path <modelo>",
        summary: "rutas resueltas del modelo",
        phase: None,
        state: CommandState::Ready,
    },
];

/// La invocación del arranque de un modelo: `ropero <modelo>`.
///
/// No es un comando del [`COMMANDS`] porque **no** es una palabra fija: lo
/// que sigue al binario es el **nombre del modelo**. Aun así es parte del
/// contrato (doc CLI v2 §8) y por eso tiene su propia entrada.
pub const RUN_USAGE: &str = "<modelo> [--force] [--cpu] [--port N] [--api-key CLAVE] [--ctx N]";

/// La frase de `ropero <modelo>` en la ayuda.
pub const RUN_SUMMARY: &str = "arranca el modelo en su motor";

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
pub const FLAGS_INSTALL: &[Flag] = &[Flag::many(
    "prio",
    "VIA",
    "la vía de instalación: download o build (repetible)",
)];

/// Los flags de `ropero stop`.
pub const FLAGS_STOP: &[Flag] = &[];

/// Los flags de `ropero gguffer` (doc CLI v2 §11.2).
pub const FLAGS_GGUFFER: &[Flag] = &[
    Flag::value("repo", "OWNER/NAME", "repo de HuggingFace de origen"),
    Flag::value("path", "DIR", "directorio de pesos locales de origen"),
    Flag::many("quant", "TIPO", "tipo(s) de cuantización"),
    Flag::value("imatrix", "FICHERO", "matriz de importancia local"),
    Flag {
        name: "imatrix-repo",
        kind: FlagKind::Value,
        help: "repo de la matriz de importancia remota",
        value_name: Some("OWNER/NAME"),
        default: None,
        requires: Some("imatrix-file"),
    },
    Flag {
        name: "imatrix-file",
        kind: FlagKind::Value,
        help: "fichero de la matriz de importancia remota",
        value_name: Some("NOMBRE"),
        default: None,
        requires: Some("imatrix-repo"),
    },
    Flag::value("out", "NOMBRE|RUTA", "nombre o ruta del GGUF de salida"),
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

/// Los flags de `ropero list` y `ropero engines`.
pub const FLAGS_ALL: &[Flag] = &[Flag::switch(
    "all",
    "muestra también lo que no está disponible",
)];

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
];

/// Los flags de cada comando, por nombre de comando.
///
/// Es la tabla que el parser consulta: así el contrato de los flags vive
/// en el dominio y el parser no lo duplica.
pub fn flags_de(cmd: &str) -> &'static [Flag] {
    match cmd {
        "install" => FLAGS_INSTALL,
        "stop" => FLAGS_STOP,
        "gguffer" => FLAGS_GGUFFER,
        "list" | "engines" => FLAGS_ALL,
        "run" => FLAGS_RUN,
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
    /// `ropero list`
    pub const LIST: &str = "modelos disponibles, agrupados por motor";
    /// `ropero template`
    pub const TEMPLATE: &str = "vuelca la plantilla del catálogo";
    /// `ropero engines`
    pub const ENGINES: &str = "motores, con su color y su compatibilidad";
    /// `ropero validate`
    pub const VALIDATE: &str = "valida los TOML del catálogo";
    /// `ropero args`
    pub const ARGS: &str = "argv final con el que se sirve el modelo";
    /// `ropero levels`
    pub const LEVELS: &str = "niveles de razonamiento declarados";
    /// `ropero path`
    pub const PATH: &str = "rutas resueltas del modelo";
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
    let mut out = String::from("USO: ropero <comando> [opciones]\n\nComandos:\n");
    for comando in COMMANDS {
        out.push_str(&format!("  {:<38}{}\n", comando.usage, comando.summary));
    }
    out.push_str(&format!("  {:<38}{}\n", RUN_USAGE, RUN_SUMMARY));
    out.push_str("\n  ropero --help                     esta ayuda\n");
    out.push_str("\nmás: ropero <comando> --help\n");
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
    format!("modelo desconocido: '{}' (ropero list)", name)
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
    format!("error: {error}\n  (ropero args --help → uso de args)")
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
