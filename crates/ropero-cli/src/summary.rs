//! `ropero` a secas: el resumen de la máquina.
//!
//! El doc CLI v2 §6 pide seis cosas: el nombre, el build (versión y fecha),
//! el autor, los motores disponibles e instalados, los modelos disponibles
//! e instalados, y la sugerencia de `ropero --help`.
//!
//! Sale con **0**: es una ayuda breve, no un error (§5.2).

use ropero_core::surface;

/// `ropero` sin argumentos: el resumen.
pub fn run() -> i32 {
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let foto = crate::discovery::snapshot(&s);
    let texto = ropero_core::snapshot::resumen_text(
        &foto,
        env!("ROPERO_VERSION"),
        env!("ROPERO_BUILD_DATE"),
        env!("ROPERO_AUTHOR"),
    );
    ropero_core::out(&texto, false);
    0
}

/// `ropero <modelo>`: el arranque de un modelo.
///
/// El doc CLI v2 §8 lo pone en la fase del runtime (F3): todavía no hay
/// motor al que lanzar. El comando **se parsea** (sus flags son los del
/// contrato, y un flag inválido es un error de uso) pero responde con
/// honestidad: dice que falta la fase y qué hacer.
pub fn run_modelo(argumentos: &[String]) -> i32 {
    let matches = crate::parser::arranque().get_matches_from(
        std::iter::once("ropero <modelo>".to_string()).chain(argumentos.iter().cloned()),
    );
    let modelo = crate::parser::modelo(&matches);
    // El modelo tiene que estar en el catálogo: si no, el arranque no puede
    // ni intentarlo, y decirlo es más útil que un "todavía no".
    if let Ok(s) = crate::discovery::scan()
        && ropero_core::find_spec(&s.catalog, &modelo).is_none()
    {
        let msg = surface::unknown_model_message(&modelo);
        ropero_core::err_line(&surface::usage_message("", &msg));
        return 2;
    }
    let flags = flags(&matches);
    let mensaje = surface::pending_message(&format!("{modelo}{flags}"));
    ropero_core::err_line(&mensaje);
    1
}

/// Los flags del arranque, en texto, para que el mensaje los nombre.
fn flags(matches: &clap::ArgMatches) -> String {
    let mut nombres: Vec<String> = Vec::new();
    for flag in ropero_core::surface::FLAGS_RUN {
        let presente = match flag.kind {
            ropero_core::surface::FlagKind::Switch => matches.get_flag(flag.name),
            _ => matches.get_one::<String>(flag.name).is_some(),
        };
        if presente {
            nombres.push(format!("--{}", flag.name));
        }
    }
    if nombres.is_empty() {
        String::new()
    } else {
        format!(" {}", nombres.join(" "))
    }
}
