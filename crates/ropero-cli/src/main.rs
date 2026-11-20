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
mod connect;
mod discovery;
mod gguffer;
mod install;
mod models;
mod parser;
mod remove;
mod summary;
mod test;

use clap::ArgMatches;

fn main() {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(dispatch(&argumentos));
}

/// Despacha los argumentos de la línea de comandos.
fn dispatch(argumentos: &[String]) -> i32 {
    if let [config, keys, rest @ ..] = argumentos
        && config == "config"
        && keys == "keys"
    {
        return commands::config_keys(rest);
    }
    // Sintaxis explícita de arranque e instalación. Se traduce al contrato
    // de instalación existente para mantener una única implementación.
    if argumentos.first().map(String::as_str) == Some("run") {
        return summary::run_modelo(&argumentos[1..]);
    }
    if argumentos.len() >= 2
        && matches!(argumentos[0].as_str(), "models" | "engines")
        && argumentos[1] == "install"
    {
        let que = if argumentos[0] == "models" {
            "models"
        } else {
            "engines"
        };
        let traducidos = std::iter::once("ropero".to_string())
            .chain(std::iter::once("install".to_string()))
            .chain(std::iter::once(que.to_string()))
            .chain(argumentos[2..].iter().cloned());
        let matches = parser::build().get_matches_from(traducidos);
        return comando(&matches);
    }
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
        Some(alias) if argumentos.get(1).map(String::as_str) == Some("remove") => {
            remove::run(alias, &argumentos[2..])
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
        "gguffer" => gguffer::run(sub),
        "status" => commands::status(sub.get_one::<f64>("live").copied()),
        "doctor" => commands::doctor(),
        "config" => commands::config(sub.get_one::<String>("lang").map(String::as_str)),
        "connect" => connect::run(sub),
        "models"
            if sub
                .get_one::<String>("action")
                .is_some_and(|action| action == "add") =>
        {
            models::add(sub)
        }
        "models" => commands::list(
            tiene_all(sub),
            sub.get_flag("fired"),
            sub.get_one::<String>("engine").map(String::as_str),
        ),
        "model" => models::state(sub),
        "engines" => commands::engines(tiene_all(sub)),
        "engine" => commands::engine_params(
            sub.get_one::<String>("engine").map(String::as_str),
            sub.get_flag("params"),
        ),
        "template" => commands::template(),
        "validate" => commands::validate(),
        "command" => commands::args(&parser::modelo(sub)),
        "info" => commands::info(&parser::modelo(sub)),
        "test" => test::run(sub),
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
