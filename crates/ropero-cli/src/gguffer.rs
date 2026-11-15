//! `ropero gguffer`: fabrica un GGUF a partir de pesos o de un repo.
//!
//! La CLI sólo **pega** (R-ARQ-1): lee los flags de `clap`, los mete en
//! [`ropero_forge::peticion::Flags`] y el dominio (`ropero-forge`) valida,
//! planifica y ejecuta las seis fases. Los códigos de salida los decide el
//! [`ropero_forge::Error`] (un `Uso` es un **2**; el resto, un **1**).
//!
//! Las dos invocaciones que no fabrican nada:
//!
//! | Invocación | Qué hace |
//! |---|---|
//! | `--list-quants` | lista los quants del motor y los ya publicados |
//! | `--diagnostico` | enseña el motor y las rutas, sin escribir nada |

use clap::ArgMatches;

use ropero_forge::peticion::Flags;

/// `ropero gguffer [<flags>]`.
pub fn run(sub: &ArgMatches) -> i32 {
    let env = crate::discovery::capture_environment();
    let reporte = ropero_forge::gguffer(flags_de(sub), &env);
    for linea in reporte.lineas {
        ropero_core::out_line(&linea);
    }
    match reporte.error {
        Some(e) => informa(&e),
        None => 0,
    }
}

/// Informa de un error de la forja: la línea, la pista y su código.
fn informa(e: &ropero_forge::Error) -> i32 {
    let mensaje = match e {
        ropero_forge::Error::Uso(_) => {
            ropero_core::surface::usage_message("gguffer", &e.to_string())
        }
        _ => format!("error: {e}"),
    };
    ropero_core::err_line(&mensaje);
    if let Some(pista) = e.pista() {
        ropero_core::err_line(&pista);
    }
    e.codigo()
}

/// Los flags de `clap` → los flags crudos del dominio.
///
/// Ninguna interpretación aquí (R-ARQ-1): la CLI sólo copia lo que el
/// usuario escribió; las validaciones viven en [`ropero_forge::gguffer`].
fn flags_de(sub: &ArgMatches) -> Flags {
    let valor = |nombre: &str| sub.get_one::<String>(nombre).cloned();
    let varios = |nombre: &str| {
        sub.get_many::<String>(nombre)
            .map(|v| v.cloned().collect())
            .unwrap_or_default()
    };
    Flags {
        repo: valor("repo"),
        path: valor("path"),
        gguf: valor("gguf"),
        quants: varios("quant"),
        imatrix: valor("imatrix"),
        imatrix_repo: valor("imatrix-repo"),
        imatrix_file: valor("imatrix-file"),
        out: valor("out"),
        out_principal: valor("out-principal"),
        intermedio: valor("intermedio"),
        keep_source: sub.get_flag("keep-source"),
        keep_intermedio: sub.get_flag("keep-intermedio"),
        no_mtp: sub.get_flag("no-mtp"),
        no_dflash: sub.get_flag("no-dflash"),
        force: sub.get_flag("force"),
        list_quants: sub.get_flag("list-quants"),
        diagnostico: sub.get_flag("diagnostico"),
    }
}
