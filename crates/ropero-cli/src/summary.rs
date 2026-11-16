//! `ropero` a secas: el resumen de la máquina.
//!
//! El doc CLI v2 §6 pide seis cosas: el nombre, el build (versión y fecha),
//! el autor, los motores disponibles e instalados, los modelos disponibles
//! e instalados, y la sugerencia de `ropero --help`.
//!
//! Sale con **0**: es una ayuda breve, no un error (§5.2).

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
/// Traduce los flags del contrato a la petición del runtime; la lógica del
/// arranque vive en `ropero-runtime` (R-ARQ-1).
pub fn run_modelo(argumentos: &[String]) -> i32 {
    let matches = crate::parser::arranque().get_matches_from(
        std::iter::once("ropero <modelo>".to_string()).chain(argumentos.iter().cloned()),
    );
    let peticion = ropero_runtime::peticion_de(
        &crate::parser::modelo(&matches),
        matches.get_flag("force"),
        matches.get_flag("cpu"),
        matches.get_one::<String>("port").map(|s| s.as_str()),
        matches.get_one::<String>("api-key").map(|s| s.as_str()),
        matches.get_flag("no-api-key"),
        matches.get_one::<String>("ctx").map(|s| s.as_str()),
        matches.get_flag("background"),
        matches.get_one::<String>("log-level").map(|s| s.as_str()),
    );
    match peticion {
        Ok(peticion) => crate::commands::arranca(&peticion),
        Err(outcome) => {
            if let Some(mensaje) = &outcome.error {
                ropero_core::err_line(&ropero_core::surface::usage_message("", mensaje));
            }
            if outcome
                .error
                .as_deref()
                .is_some_and(|m| m.contains("modelo desconocido"))
            {
                2
            } else {
                outcome.code
            }
        }
    }
}
