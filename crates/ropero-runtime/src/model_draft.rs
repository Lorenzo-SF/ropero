//! Asistente de terminal para crear modelos sin depender de la plantilla.

use ropero_core::{EngineManifest, Environment};

pub fn wizard_toml(
    engines: &[EngineManifest],
    env: &Environment,
    lang: &str,
) -> Result<String, String> {
    let english = lang == "EN";
    let ask = |label: &str, optional: bool| -> Result<Option<String>, String> {
        let answer = crate::install::pregunta(&format!(
            "{label}{}: ",
            if optional {
                if english {
                    " (optional)"
                } else {
                    " (opcional)"
                }
            } else {
                ""
            }
        ))
        .ok_or_else(|| {
            if english {
                "no answer was provided; cancelling".to_string()
            } else {
                "no hay respuesta: se cancela".to_string()
            }
        })?;
        let answer = answer.trim().to_string();
        if answer.is_empty() && !optional {
            return Err(if english {
                format!("{label} is required")
            } else {
                format!("el campo {label} es obligatorio")
            });
        }
        Ok((!answer.is_empty()).then_some(answer))
    };
    let ask_assignment = |block: &str, text: &mut String| -> Result<(), String> {
        loop {
            let Some(answer) = crate::install::pregunta(&format!(
                "{block}: {}: ",
                if english {
                    "TOML field=value (empty to finish)"
                } else {
                    "campo=valor TOML (vacío termina)"
                }
            )) else {
                return Err(if english {
                    "no answer was provided; cancelling".into()
                } else {
                    "no hay respuesta: se cancela".into()
                });
            };
            let answer = answer.trim();
            if answer.is_empty() {
                return Ok(());
            }
            let path = if answer
                .split_once('=')
                .is_some_and(|(path, _)| path.contains('.'))
            {
                answer.to_string()
            } else {
                format!("{block}.{answer}")
            };
            *text = ropero_core::model_draft::apply_sets(text, &[path])?;
        }
    };

    let installed = engines
        .iter()
        .filter(|engine| crate::engine_installed_with_env(engine, env))
        .collect::<Vec<_>>();
    if installed.is_empty() {
        return Err(if english {
            "no installed engines are available to register a model".into()
        } else {
            "no hay engines instalados para registrar el modelo".into()
        });
    }
    let engine_choices = installed
        .iter()
        .map(|engine| engine.id.as_str().to_string())
        .collect::<Vec<_>>();
    let palette = ropero_core::Palette::color();
    let legend = installed
        .iter()
        .map(|engine| palette.paint(engine.color, &engine.label))
        .collect::<Vec<_>>()
        .join(" - ");
    let mut engine_lines = vec![
        if english {
            "Installed engines:".to_string()
        } else {
            "Engines instalados:".to_string()
        },
        format!("  {legend}"),
        String::new(),
        if english {
            "Choose an engine:".to_string()
        } else {
            "Elige un engine:".to_string()
        },
    ];
    for (index, engine) in installed.iter().enumerate() {
        engine_lines.push(format!(
            "  {}. {} ({})",
            index + 1,
            engine.label,
            engine.id.as_str()
        ));
    }
    for line in &engine_lines {
        ropero_core::out_line(line);
    }
    let answer = crate::install::pregunta(if english {
        "Choose an engine by number or id: "
    } else {
        "Elige el engine por número o id: "
    })
    .ok_or_else(|| {
        if english {
            "no answer was provided; cancelling".to_string()
        } else {
            "no hay respuesta: se cancela".to_string()
        }
    })?;
    let engine = ropero_core::install::resuelve_seleccion(&answer, &engine_choices)
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or("no se eligió ningún engine")?;
    let manifest = installed
        .iter()
        .find(|item| item.id.as_str() == engine)
        .expect("opción procede del menú");
    let name = ask(
        if english {
            "Internal model and TOML name"
        } else {
            "Nombre interno del modelo y del TOML"
        },
        false,
    )?
    .unwrap();
    let format = ask(
        if english {
            "Format (gguf/safetensors/q4nx/strata/ninfer)"
        } else {
            "Formato (gguf/safetensors/q4nx/strata/ninfer)"
        },
        false,
    )?
    .unwrap();
    let artifact = ask(
        if english {
            "Model or main artifact path"
        } else {
            "Ruta del modelo o artefacto principal"
        },
        true,
    )?;
    let context = ask(
        if english {
            "Context in tokens"
        } else {
            "Contexto en tokens"
        },
        true,
    )?
    .map(|value| {
        value.parse::<u32>().map_err(|_| {
            if english {
                "invalid context".to_string()
            } else {
                "contexto inválido".to_string()
            }
        })
    })
    .transpose()?;
    let alias = ask(
        if english {
            "Alias/model id exposed by the engine"
        } else {
            "Alias/model id que expone el engine"
        },
        true,
    )?;

    let mut text = if let Some(artifact) = artifact.as_deref() {
        ropero_core::model_draft::from_fields(
            &name,
            &engine,
            &format,
            artifact,
            context,
            None,
            alias.as_deref(),
        )?
    } else {
        ropero_core::model_draft::from_fields(
            &name,
            &engine,
            &format,
            "",
            context,
            None,
            alias.as_deref(),
        )?
    };

    // Configure common model arguments first. Reserved engine parameters
    // remain controlled by the runtime.
    let defaults = manifest
        .serve
        .as_ref()
        .map(|serve| serve.defaults.as_slice())
        .unwrap_or(&[]);
    ropero_core::out_line(&format!(
        "{}: {}",
        if english {
            "Engine default arguments (you can override options)"
        } else {
            "Argumentos por defecto del engine (puedes sobreescribir opciones)"
        },
        if defaults.is_empty() {
            if english { "none" } else { "ninguno" }.into()
        } else {
            defaults.join(" ")
        }
    ));
    let base = ask(
        if english {
            "Model args/overrides for [args].base (TOML, e.g. [\"--gpu-layers\", \"44\"])"
        } else {
            "Args del modelo/override para [args].base (TOML, ej. [\"--gpu-layers\", \"44\"])"
        },
        true,
    )?;
    if let Some(base) = base {
        text = ropero_core::model_draft::apply_sets(&text, &[format!("args.base={base}")])?;
    }
    for block in ["files", "download", "build"] {
        ask_assignment(block, &mut text)?;
    }
    for block in [
        "levels",
        "flags",
        "options",
        "limits",
        "aliases",
        "description",
        "args.level",
        "args.flag",
        "args.option",
    ] {
        ask_assignment(block, &mut text)?;
    }
    reject_reserved_args(&text)?;
    Ok(text)
}

fn reject_reserved_args(text: &str) -> Result<(), String> {
    let document: toml::Value = toml::from_str(text).map_err(|error| error.to_string())?;
    let Some(args) = document.get("args").and_then(toml::Value::as_table) else {
        return Ok(());
    };
    let mut values = Vec::new();
    collect_arg_values(args, &mut values);
    let reserved = [
        "--host",
        "--port",
        "--api-key",
        "--server-bind",
        "--model",
        "--model-path",
        "--model-id",
        "--served-model-name",
        "--served_model_name",
        "-m",
    ];
    if let Some(argument) = values.iter().find(|value| {
        reserved
            .iter()
            .any(|flag| value.as_str() == *flag || value.starts_with(&format!("{flag}=")))
    }) {
        return Err(format!(
            "`{argument}` está reservado a Ropero (host, puerto, clave o ruta del modelo); configúralo con los flags de arranque"
        ));
    }
    Ok(())
}

fn collect_arg_values(table: &toml::Table, output: &mut Vec<String>) {
    for value in table.values() {
        match value {
            toml::Value::String(value) => output.push(value.clone()),
            toml::Value::Array(values) => {
                output.extend(
                    values
                        .iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_owned),
                );
            }
            toml::Value::Table(nested) => collect_arg_values(nested, output),
            _ => {}
        }
    }
}
