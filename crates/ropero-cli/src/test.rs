//! `ropero test`: la batería de pruebas (doc CLI v2 §13).
//!
//! La CLI sólo pega (R-ARQ-1): la selección es del dominio
//! ([`ropero_bateria`]) y la tirada es del runtime
//! ([`ropero_runtime::inferencia`]). Sin `--json` el informe sale por
//! stdout; con `--json`, el JSON por stdout y el texto por stderr.

use clap::ArgMatches;

use ropero_bateria::seleccion::{Opciones, seleccion_de};
use ropero_runtime::inferencia::{ErrorInferencia, tira};

/// `ropero test [--modelo <id>] [--grupo A..F] [--test <id>] [--dimension <n>]`.
pub fn run(sub: &ArgMatches) -> i32 {
    let opciones = Opciones {
        grupo: valor(sub, "grupo"),
        test: valor(sub, "test"),
        dimension: valor(sub, "dimension"),
        tipo: valor(sub, "tipo"),
        todos: sub.get_flag("todos"),
    };
    let seleccion = match seleccion_de(opciones) {
        Ok(seleccion) => seleccion,
        Err(e) => return ErrorInferencia::uso(e.mensaje).informa(),
    };
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    tira(
        &s.catalog,
        &s.engines,
        &s.env,
        &s.root,
        valor(sub, "modelo"),
        &seleccion,
        sub.get_flag("json"),
    )
}

/// El valor de un flag, si lo hay.
fn valor(sub: &ArgMatches, nombre: &str) -> Option<String> {
    sub.get_one::<String>(nombre).cloned()
}
