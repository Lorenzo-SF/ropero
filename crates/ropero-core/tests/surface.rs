//! La superficie del CLI: comandos, sus frases y su estado.
//!
//! El contrato de comandos vive en `ropero_core::surface`. Este test
//! comprueba que la tabla es coherente y que los comandos declarados tienen
//! frase y forma de uso.

use ropero_core::COMMANDS;
use ropero_core::surface::{self, Command, CommandState};

/// Los comandos del contrato actual, con su forma de invocación.
const COMANDOS: &[(&str, &str)] = &[
    ("install", "install [engines|models] [modelo...]"),
    ("stop", "stop [modelo]"),
    ("gguffer", "gguffer --repo OWNER/NAME"),
    ("status", "status"),
    ("doctor", "doctor"),
    ("config", "config"),
    ("list", "list [--all]"),
    ("template", "template"),
    ("engines", "engines [--all]"),
    ("validate", "validate"),
    (
        "args",
        "args <modelo> [--<nivel>] [--<flag>] [--<opcion>=<valor>]",
    ),
    ("levels", "levels <modelo>"),
    ("path", "path <modelo>"),
    (
        "test",
        "test [--modelo <id>] [--grupo A..F] [--test <id>] [--dimension <nombre>] [--todos]",
    ),
];

/// Los comandos están en la tabla, con su forma de invocación.
#[test]
fn los_once_comandos_estan_declarados() {
    for (nombre, uso) in COMANDOS {
        let cmd = COMMANDS
            .iter()
            .find(|c| c.name == *nombre)
            .unwrap_or_else(|| panic!("el comando '{nombre}' no está en la tabla"));
        assert_eq!(cmd.usage, *uso, "el uso de '{nombre}' cambió");
    }
    assert_eq!(
        COMMANDS.len(),
        COMANDOS.len(),
        "sobran comandos en la tabla"
    );
}

/// Ningún comando se repite.
#[test]
fn ningun_comando_se_repite() {
    let mut nombres: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
    nombres.sort();
    let total = nombres.len();
    nombres.dedup();
    assert_eq!(nombres.len(), total, "hay un comando repetido");
}

/// Todo comando tiene una frase de **una** línea.
#[test]
fn todo_comando_tiene_una_frase_de_una_linea() {
    for cmd in COMMANDS {
        assert!(!cmd.summary.is_empty(), "'{}' no tiene frase", cmd.name);
        assert!(
            !cmd.summary.contains('\n'),
            "la frase de '{}' ocupa más de una línea",
            cmd.name
        );
        assert!(
            cmd.summary.chars().count() <= 70,
            "la frase de '{}' es demasiado larga: '{}'",
            cmd.name,
            cmd.summary
        );
    }
}

/// Los comandos son exactamente los del contrato actual.
#[test]
fn la_tabla_es_exactamente_la_del_contrato() {
    let mut declarados: Vec<&str> = COMMANDS.iter().map(|c| c.name).collect();
    declarados.sort();
    let mut esperados: Vec<&str> = COMANDOS.iter().map(|(n, _)| *n).collect();
    esperados.sort();
    assert_eq!(declarados, esperados);
}

/// Los comandos que aún no están implementados **lo dicen**: su mensaje
/// explica qué falta y qué hacer (P-13).
#[test]
fn los_comandos_pendientes_dicen_que_falta() {
    for cmd in COMMANDS {
        if cmd.es_ready() {
            continue;
        }
        let mensaje = match cmd.state {
            CommandState::Parsed => surface::pending_message(cmd.name),
            CommandState::Phase => surface::phase_message(cmd.name, cmd.phase.unwrap_or("F3")),
            CommandState::Ready => unreachable!("los listos no pasan por aquí"),
        };
        assert!(
            mensaje.contains(cmd.name),
            "el mensaje de '{}' no lo nombra: {mensaje}",
            cmd.name
        );
        assert!(
            !mensaje.is_empty() && mensaje.len() > 20,
            "el mensaje de '{}' no dice nada: {mensaje}",
            cmd.name
        );
    }
}

/// `gguffer` **funciona** (F6): su estado es [`CommandState::Ready`],
/// como `install` (F5).
#[test]
fn gguffer_esta_listo() {
    let cmd = COMMANDS
        .iter()
        .find(|c| c.name == "gguffer")
        .unwrap_or_else(|| panic!("falta 'gguffer'"));
    assert_eq!(
        cmd.state,
        CommandState::Ready,
        "'gguffer' debería estar listo"
    );
    assert!(cmd.es_ready());
}

/// `install` funciona: su estado es [`CommandState::Ready`].
#[test]
fn install_esta_listo() {
    let cmd = COMMANDS
        .iter()
        .find(|c| c.name == "install")
        .unwrap_or_else(|| panic!("falta 'install'"));
    assert_eq!(cmd.state, CommandState::Ready);
    assert!(cmd.es_ready());
}

/// `stop` funciona: su estado es [`CommandState::Ready`].
#[test]
fn stop_esta_listo() {
    let cmd = COMMANDS
        .iter()
        .find(|c| c.name == "stop")
        .unwrap_or_else(|| panic!("falta 'stop'"));
    assert_eq!(cmd.state, CommandState::Ready);
    assert!(cmd.es_ready());
}

/// Los comandos que sí funcionan están marcados como listos.
#[test]
fn los_comandos_listos_estan_marcados() {
    for nombre in [
        "install", "status", "doctor", "config", "list", "template", "engines", "validate", "args",
        "levels", "path",
    ] {
        let cmd: &Command = COMMANDS
            .iter()
            .find(|c| c.name == nombre)
            .unwrap_or_else(|| panic!("falta '{nombre}'"));
        assert!(cmd.es_ready(), "'{nombre}' debería estar listo");
    }
}

/// La ayuda completa anuncia los once comandos **y** el arranque de un
/// modelo, y termina sugieriendo `ropero <comando> --help`.
#[test]
fn la_ayuda_anuncia_todo_y_sugiere_el_uso_de_cada_comando() {
    let ayuda = surface::help_text();
    for cmd in COMMANDS {
        assert!(
            ayuda.contains(cmd.name),
            "la ayuda no anuncia '{}'",
            cmd.name
        );
    }
    assert!(
        ayuda.contains("<modelo>"),
        "la ayuda no anuncia el arranque"
    );
    assert!(
        ayuda.contains("ropero <comando> --help"),
        "la ayuda no sugiere el uso de cada comando"
    );
}

/// La ayuda breve del resumen tiene las **cuatro** secciones del doc
/// (doc CLI v2 §6): motores, modelos, build y autor.
#[test]
fn el_resumen_tiene_las_cuatro_secciones() {
    let esqueleto = surface::resumen_esqueleto();
    assert!(esqueleto.contains("MOTORES"));
    assert!(esqueleto.contains("MODELOS"));
    assert!(esqueleto.contains("build:"));
    assert!(esqueleto.contains("autor:"));
    assert!(
        esqueleto.contains("ropero --help"),
        "el resumen debe sugerir la ayuda"
    );
}

/// El mensaje de un comando desconocido nombra el comando y dice dónde está
/// la ayuda.
#[test]
fn el_comando_desconocido_dice_que_es_desconocido() {
    let mensaje = surface::unknown_command_message("inventado");
    assert!(mensaje.contains("inventado"));
    assert!(mensaje.contains("desconocido"));
    assert!(mensaje.contains("ropero --help"));
}

/// Una opción desconocida (que empieza por `-`) se llama opción, no
/// comando.
#[test]
fn la_opcion_desconocida_se_llama_opcion() {
    let mensaje = surface::unknown_command_message("--inventado");
    assert!(mensaje.contains("opción"));
}
