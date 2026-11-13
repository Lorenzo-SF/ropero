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
            let dash = c.starts_with('-');
            eprintln!(
                "error: {} desconocido: '{}'",
                if dash { "opción" } else { "comando" },
                c
            );
            eprintln!("  (ropero help → uso completo)");
            2
        }
    }
}
/// Chequeos de uso de un comando que opera sobre un modelo: `--help`, modelo
/// ausente y argumentos de más. El error ya sale impreso.
pub fn check_usage(args: &[String], cmd: &str, uso: &str, too_many: &str) -> Result<(), i32> {
    for a in args {
        if a == "--help" || a == "-h" {
            println!("USO: {}", uso);
            return Err(0);
        }
    }
    if args.is_empty() {
        return Err(usage_error(
            cmd,
            &format!("falta el modelo (ropero {} <modelo>)", cmd),
        ));
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
    let msg = format!("modelo desconocido: '{}' (ropero list)", name);
    ropero_core::find_spec(cat, name).ok_or_else(|| usage_error(cmd, &msg))
}
pub fn print_help() -> i32 {
    println!(
        "USO: ropero <modelo> [--port N] [--cpu] [--background] [--force]\n     ropero <comando> [opciones]\n\nComandos:\n  validate            valida los TOML del catálogo\n  list [--all]        modelos del catálogo\n  template            vuelca models/_template.toml\n  args <modelo>       argv final del modelo\n  levels <modelo>     niveles declarados\n  path <modelo>       rutas resueltas\n  help                esta ayuda\n  --version           versión"
    );
    0
}
pub fn usage_error(cmd: &str, msg: &str) -> i32 {
    if !msg.is_empty() {
        eprintln!("error: {}", msg);
    }
    match cmd.is_empty() {
        true => eprintln!("  (ropero help → uso completo)"),
        false => eprintln!("  (ropero {} --help → uso de {})", cmd, cmd),
    }
    2
}
