//! El dispatcher de la CLI.
//!
//! La CLI es fina por contrato (R-ARQ-1): **despacha e imprime**. La lógica
//! vive en `ropero-core` (dominio puro) y en `discovery` (la frontera de
//! E/S).
mod args_cmd;
mod discovery;
mod engines_cmd;
mod levels;
mod list;
mod path;
mod template;
mod validate;
use ropero_core::{Catalog, ModelSpec, UsageCheck};
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
            ropero_core::out_line(&format!("ropero {}", env!("CARGO_PKG_VERSION")));
            0
        }
        "validate" => validate::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "list" => list::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "engines" => engines_cmd::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "template" => template::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "args" => args_cmd::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "levels" => levels::run(&args[2..]).unwrap_or_else(std::convert::identity),
        "path" => path::run(&args[2..]).unwrap_or_else(std::convert::identity),
        c => {
            ropero_core::err_line(&ropero_core::unknown_command_message(c));
            2
        }
    }
}
pub fn check_usage(args: &[String], cmd: &str, uso: &str, too_many: &str) -> Result<(), i32> {
    let check = ropero_core::check_model_usage(args, cmd, too_many);
    apply_usage(check, cmd, uso)
}
pub fn check_no_args(args: &[String], cmd: &str, uso: &str) -> Result<(), i32> {
    apply_usage(ropero_core::check_no_args_usage(args, cmd), cmd, uso)
}

pub fn check_all_flag(args: &[String], cmd: &str, uso: &str) -> Result<bool, i32> {
    let (check, all) = ropero_core::check_all_flag_usage(args, cmd);
    apply_usage(check, cmd, uso)?;
    Ok(all)
}
fn apply_usage(check: UsageCheck, cmd: &str, uso: &str) -> Result<(), i32> {
    match check {
        UsageCheck::Ok => Ok(()),
        UsageCheck::Help => {
            ropero_core::out_line(&format!("USO: {}", uso));
            Err(0)
        }
        UsageCheck::Error { message } => Err(usage_error(cmd, &message)),
    }
}
pub fn spec_for<'a>(cat: &'a Catalog, name: &str, cmd: &str) -> Result<&'a ModelSpec, i32> {
    let msg = ropero_core::unknown_model_message(name);
    ropero_core::find_spec(cat, name).ok_or_else(|| usage_error(cmd, &msg))
}
pub fn print_help() -> i32 {
    ropero_core::out_line(&ropero_core::help_text());
    0
}
pub fn usage_error(cmd: &str, msg: &str) -> i32 {
    ropero_core::err_line(&ropero_core::usage_message(cmd, msg));
    2
}
