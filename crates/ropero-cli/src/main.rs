mod args_cmd;
mod discovery;
mod levels;
mod list;
mod path;
mod template;
mod validate;
use ropero_core::{Catalog, ModelSpec};
use std::env;
fn main() {
    let args: Vec<String> = env::args().collect();
    std::process::exit(run(&args));
}
fn run(args: &[String]) -> i32 {
    if args.len() <= 1 {
        return print_help();
    }
    match args[1].as_str() {
        "help" | "-h" | "--help" => print_help(),
        "--version" | "-V" => {
            println!("ropero {}", env!("CARGO_PKG_VERSION"));
            0
        }
        "validate" => validate::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "list" => list::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "template" => template::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "args" => args_cmd::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "levels" => levels::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "path" => path::run(&args[2..]).unwrap_or_else(std::convert::identity),
        c => {
            eprintln!("{}", ropero_core::unknown_command_message(c));
            2
        }
    }
}
/// Chequeos de uso de un comando que opera sobre un modelo: `--help`, modelo
/// ausente y argumentos de más. El error ya sale impreso.
pub fn check_usage(args: &[String], cmd: &str, uso: &str, too_many: &str) -> Result<(), i32> {
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("USO: {}", uso);
        return Err(0);
    }
    if args.is_empty() {
        return Err(usage_error(cmd, &ropero_core::missing_model_message(cmd)));
    }
    if !too_many.is_empty() && args.len() > 1 {
        let msg = too_many.replace("{}", args[1].as_str());
        return Err(usage_error(cmd, &msg));
    }
    Ok(())
}
/// Chequeo de uso de un comando que no acepta argumentos: `--help` o error.
pub fn check_no_args(args: &[String], cmd: &str, uso: &str) -> Result<(), i32> {
    match args.first().map(|a| a.as_str()) {
        Some("--help") | Some("-h") => {
            println!("USO: {}", uso);
            Err(0)
        }
        Some(a) => Err(usage_error(cmd, &format!("{} no acepta '{}'", cmd, a))),
        None => Ok(()),
    }
}
/// Busca un modelo en el catálogo. El error ya sale impreso; el código de
/// salida es el de un error de uso (2).
pub fn spec_for<'a>(cat: &'a Catalog, name: &str, cmd: &str) -> Result<&'a ModelSpec, i32> {
    let msg = ropero_core::unknown_model_message(name);
    ropero_core::find_spec(cat, name).ok_or_else(|| usage_error(cmd, &msg))
}
pub fn print_help() -> i32 {
    println!("{}", ropero_core::help_text());
    0
}
pub fn usage_error(cmd: &str, msg: &str) -> i32 {
    eprintln!("{}", ropero_core::usage_message(cmd, msg));
    2
}
