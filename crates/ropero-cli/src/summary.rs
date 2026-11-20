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
    if argumentos
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        let _ = crate::parser::arranque().get_matches_from(
            std::iter::once("ropero run".to_string()).chain(argumentos.iter().cloned()),
        );
    }
    let Ok(scan) = crate::discovery::scan() else {
        return 1;
    };
    let Some(model_name) = argumentos.first() else {
        return 2;
    };
    let Some(spec) = ropero_core::find_spec(&scan.catalog, model_name) else {
        ropero_core::err_line(&ropero_core::surface::unknown_model_message(model_name));
        return 2;
    };
    let (static_args, model_args) = split_model_args(argumentos);
    let matches = crate::parser::arranque()
        .get_matches_from(std::iter::once("ropero <modelo>".to_string()).chain(static_args));
    let parsed = ropero_core::parse_request(spec, &model_args);
    if let Some(previous) = parsed.repeated_level.as_deref() {
        let current = parsed.level.as_deref().unwrap_or_default();
        ropero_core::err_line(&ropero_core::repeated_level_notice(previous, current));
    }
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
        Ok(mut peticion) => {
            peticion.level = parsed.level.filter(|level| {
                spec.levels.list.contains(level)
                    || spec
                        .aliases
                        .get(level)
                        .is_some_and(|target| spec.levels.list.contains(target))
            });
            peticion.model_flags = parsed
                .flags
                .into_iter()
                .filter(|flag| spec.flags.list.contains(flag))
                .collect();
            peticion.model_options = parsed
                .options
                .into_iter()
                .filter(|(option, _)| spec.options.list.contains(option))
                .collect();
            crate::commands::arranca(&peticion)
        }
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

/// Separa los flags generales de arranque de los tokens que el modelo define
/// en sus propias tablas `[levels]`, `[flags]` y `[options]`.
fn split_model_args(args: &[String]) -> (Vec<String>, Vec<String>) {
    let known = ropero_core::surface::flags_de("run")
        .iter()
        .map(|flag| format!("--{}", flag.name))
        .collect::<std::collections::BTreeSet<_>>();
    let mut fixed = args.first().cloned().into_iter().collect::<Vec<_>>();
    let mut dynamic = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let token = &args[index];
        if known.contains(token) {
            fixed.push(token.clone());
            if matches!(
                token.as_str(),
                "--port" | "--api-key" | "--ctx" | "--log-level"
            ) && let Some(value) = args.get(index + 1)
            {
                fixed.push(value.clone());
                index += 1;
            }
        } else {
            dynamic.push(token.clone());
        }
        index += 1;
    }
    (fixed, dynamic)
}

#[cfg(test)]
mod tests {
    use super::split_model_args;

    #[test]
    fn separa_flags_generales_de_los_dinamicos_del_modelo() {
        let input = [
            "qwen".to_string(),
            "--background".to_string(),
            "--high".to_string(),
            "--gpu-layers".to_string(),
            "--ctx".to_string(),
            "32768".to_string(),
            "--temperature=0.7".to_string(),
        ];
        let (fixed, dynamic) = split_model_args(&input);
        assert_eq!(fixed, ["qwen", "--background", "--ctx", "32768"]);
        assert_eq!(dynamic, ["--high", "--gpu-layers", "--temperature=0.7"]);
    }
}
