//! Los textos de la CLI que describen **qué existe**.
//!
//! Viven en el core (que es dominio puro) para que la ayuda no pueda
//! mentir: el texto se genera a partir de la misma tabla de comandos que la
//! CLI despacha. Si se añade un comando, se añade aquí y la ayuda lo
//! anuncia; si se quita, la ayuda deja de anunciarlo.
//!
//! La regla es simple: **la ayuda sólo anuncia lo que existe**. Nada de
//! prometer un arranque que todavía no está (F3).

/// Un comando de la CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// El nombre con el que se invoca.
    pub name: &'static str,
    /// La forma de invocarlo, tal y como se muestra en la ayuda.
    pub usage: &'static str,
    /// Qué hace, en una línea.
    pub summary: &'static str,
}

/// Los comandos que la CLI implementa hoy.
///
/// La lista es la verdad: `ropero help` se genera a partir de ella, así
/// que no puede anunciar un comando que no exista.
pub const COMMANDS: &[Command] = &[
    Command {
        name: "validate",
        usage: "validate",
        summary: "valida los TOML del catálogo",
    },
    Command {
        name: "list",
        usage: "list [--all]",
        summary: "modelos del catálogo",
    },
    Command {
        name: "template",
        usage: "template",
        summary: "vuelca models/_template.toml",
    },
    Command {
        name: "args",
        usage: "args <modelo>",
        summary: "argv final del modelo",
    },
    Command {
        name: "levels",
        usage: "levels <modelo>",
        summary: "niveles declarados",
    },
    Command {
        name: "path",
        usage: "path <modelo>",
        summary: "rutas resueltas",
    },
    Command {
        name: "help",
        usage: "help",
        summary: "esta ayuda",
    },
];

/// El texto de la ayuda de la CLI.
///
/// Se genera a partir de [`COMMANDS`], de modo que sólo anuncia lo que
/// existe.
pub fn help_text() -> String {
    let mut out = String::from("USO: ropero <comando> [opciones]\n\nComandos:\n");
    for command in COMMANDS {
        out.push_str(&format!("  {:<18}{}\n", command.usage, command.summary));
    }
    out.push_str("  --version           versión\n");
    out
}

/// El mensaje de un error de uso, con la pista de dónde está la ayuda.
///
/// Un error de uso es el que el usuario puede arreglar escribiendo otra
/// cosa: por eso el mensaje dice **qué hacer** (P-13).
pub fn usage_message(cmd: &str, msg: &str) -> String {
    let mut out = String::new();
    if !msg.is_empty() {
        out.push_str("error: ");
        out.push_str(msg);
        out.push('\n');
    }
    if cmd.is_empty() {
        out.push_str("  (ropero help → uso completo)");
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

/// El mensaje de un comando (u opción) que la CLI no conoce.
pub fn unknown_command_message(token: &str) -> String {
    let kind = if token.starts_with('-') {
        "opción"
    } else {
        "comando"
    };
    format!("error: {kind} desconocido: '{token}'\n  (ropero help → uso completo)")
}

/// El resultado de un chequeo de uso: o la ayuda (que es un éxito), o un
/// error de uso.
///
/// Es la forma de que el **qué** (el texto) viva en el core y el **cuándo**
/// (imprimir y salir con 2) viva en la CLI.
#[derive(Debug, Clone, PartialEq)]
pub enum UsageCheck {
    /// Continúa: los argumentos son válidos.
    Ok,
    /// Hay que imprimir el uso y salir con 0.
    Help,
    /// Hay que imprimir el error y salir con 2.
    Error { message: String },
}

/// Chequea el uso de un comando que opera sobre un modelo: `--help`, modelo
/// ausente y argumentos de más.
///
/// `too_many` es el texto del error de "argumentos de más"; si viene vacío,
/// el comando no lo tiene.
pub fn check_model_usage(args: &[String], cmd: &str, too_many: &str) -> UsageCheck {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        return UsageCheck::Help;
    }
    if args.is_empty() {
        return UsageCheck::Error {
            message: missing_model_message(cmd),
        };
    }
    if !too_many.is_empty() && args.len() > 1 {
        return UsageCheck::Error {
            message: too_many.replace("{}", args[1].as_str()),
        };
    }
    UsageCheck::Ok
}

/// Chequea el uso de un comando que no acepta argumentos: `--help` o error.
pub fn check_no_args_usage(args: &[String], cmd: &str) -> UsageCheck {
    match args.first().map(|a| a.as_str()) {
        Some("--help") | Some("-h") => UsageCheck::Help,
        Some(a) => UsageCheck::Error {
            message: format!("{} no acepta '{}'", cmd, a),
        },
        None => UsageCheck::Ok,
    }
}
