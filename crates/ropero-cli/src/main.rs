//! El dispatcher de la CLI.
//!
//! La CLI es fina por contrato (R-ARQ-1): **despacha e imprime**. La lógica
//! vive en `ropero-core` (dominio puro), en `ropero-runtime` (la E/S) y en
//! `parser` (la traducción del contrato a `clap`).
//!
//! ## Los códigos de salida
//!
//! | Código | Significado |
//! |---:|---|
//! | `0` | Ok |
//! | `1` | Error |
//! | `2` | Uso (argumento o comando inválido) |
//!
//! El `130` (Ctrl-C) llega con el runtime (F3).
//!
//! ## El arranque de un modelo
//!
//! `ropero <modelo>` **no** es un comando con nombre fijo: lo que sigue al
//! binario puede ser un comando o el nombre de un modelo. La decisión se
//! toma **antes** de parsear: si el primer token es un comando del
//! contrato, se parsea con el parser de comandos; si no, se parsea con el
//! parser del arranque ([`parser::arranque`]), que es el del doc CLI v2 §8.

mod commands;
mod discovery;
mod install;
mod parser;
mod summary;

use clap::ArgMatches;

fn main() {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&argumentos));
}

/// Despacha los argumentos de la línea de comandos.
fn dispatch(argumentos: &[String]) -> i32 {
    match argumentos.first() {
        None => summary::run(),
        // Los flags de la ayuda y de la versión son del **binario**, no de
        // un comando: los atiende el parser principal (que es el único que
        // los tiene). `clap` los resuelve y sale con 0.
        Some(primer) if es_flag_global(primer) => {
            let _ = parser::build().get_matches_from(
                std::iter::once("ropero".to_string()).chain(argumentos.iter().cloned()),
            );
            0
        }
        Some(primer) if parser::es_comando(primer) => {
            let matches = parser::build().get_matches_from(
                std::iter::once("ropero".to_string()).chain(argumentos.iter().cloned()),
            );
            comando(&matches)
        }
        Some(_) => summary::run_modelo(argumentos),
    }
}

/// Despacha el comando que `clap` parseó.
fn comando(matches: &ArgMatches) -> i32 {
    let Some((nombre, sub)) = matches.subcommand() else {
        return summary::run();
    };
    match nombre {
        "install" => install::run(sub),
        "stop" => commands::stop(&parser::modelo(sub)),
        "gguffer" => pendiente("gguffer"),
        "status" => commands::status(),
        "doctor" => commands::doctor(),
        "list" => commands::list(tiene_all(sub)),
        "engines" => commands::engines(tiene_all(sub)),
        "template" => commands::template(),
        "validate" => commands::validate(),
        "args" => commands::args(&parser::modelo(sub), &parser::tokens(sub)),
        "levels" => commands::levels(&parser::modelo(sub)),
        "path" => commands::path(&parser::modelo(sub)),
        _ => 2,
    }
}

/// Si el subcomando lleva el `--all`.
fn tiene_all(matches: &ArgMatches) -> bool {
    matches.get_flag("all")
}

/// La respuesta de un comando que se parsea pero todavía no se ejecuta.
///
/// El mensaje dice **qué falta** (P-13), y el código de salida es **1**: el
/// uso es correcto, lo que falta es la implementación.
pub fn pendiente(cmd: &str) -> i32 {
    let mensaje = ropero_core::surface::pending_message(cmd);
    ropero_core::err_line(&mensaje);
    1
}

/// Si el token es un flag que pertenece al binario y no a un comando.
fn es_flag_global(token: &str) -> bool {
    matches!(token, "--help" | "-h" | "--version" | "-V")
}
