use clap::ArgMatches;
use ropero_core::{ValidationContext, parse_model};

pub fn add(matches: &ArgMatches) -> i32 {
    let sources = usize::from(matches.get_one::<String>("path").is_some())
        + usize::from(matches.get_flag("stdin"));
    let generated = matches.get_one::<String>("name").is_some()
        || matches.get_one::<String>("engine").is_some()
        || matches.get_one::<String>("format").is_some()
        || matches.get_one::<String>("artifact").is_some()
        || matches.get_one::<String>("repo").is_some()
        || matches.get_one::<String>("context").is_some()
        || matches.get_one::<String>("alias").is_some()
        || matches.get_many::<String>("set").is_some();
    if sources > 1 || (sources > 0 && generated) {
        ropero_core::err_line("error: usa una sola vía: --path, --stdin o los flags de creación");
        return 2;
    }
    let text = if let Some(path) = matches.get_one::<String>("path") {
        match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) => {
                ropero_core::err_line(&format!("error: no se pudo leer {path}: {error}"));
                return 1;
            }
        }
    } else if matches.get_flag("stdin") {
        let mut text = String::new();
        if let Err(error) = std::io::Read::read_to_string(&mut std::io::stdin(), &mut text) {
            ropero_core::err_line(&format!("error: no se pudo leer stdin: {error}"));
            return 1;
        }
        text
    } else if generated {
        match toml_from_flags(matches) {
            Ok(text) => text,
            Err(message) => {
                ropero_core::err_line(&format!("error: {message}"));
                return 2;
            }
        }
    } else if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        match ropero_runtime::model_draft::wizard_toml() {
            Ok(text) => text,
            Err(message) => {
                ropero_core::err_line(&format!("error: {message}"));
                return 2;
            }
        }
    } else {
        ropero_core::err_line("error: sin terminal, indica --path, --stdin o flags de creación");
        return 2;
    };
    let Ok(scan) = crate::discovery::scan() else {
        return 1;
    };
    let (cleaned, discarded) = match ropero_core::model_draft::clean(&text) {
        Ok(result) => result,
        Err(error) => {
            ropero_core::err_line(&format!("error: TOML inválido: {error}"));
            return 2;
        }
    };
    let root_value = match toml::from_str::<toml::Value>(&cleaned) {
        Ok(value) => value,
        Err(error) => {
            ropero_core::err_line(&format!("error: TOML inválido: {error}"));
            return 2;
        }
    };
    let tentative = match parse_model(&cleaned, std::path::Path::new("<stdin>")) {
        Ok(model) => model,
        Err(error) => {
            ropero_core::err_line(&format!("error: modelo inválido: {error}"));
            return 2;
        }
    };
    let destination = scan
        .root
        .join(tentative.model.engine.as_str())
        .join(format!("{}.toml", tentative.model.name));
    let normalized = ropero_core::model_draft::normalize_engine(root_value, &tentative);
    let cleaned = match toml::to_string_pretty(&normalized) {
        Ok(text) => text,
        Err(error) => {
            ropero_core::err_line(&format!("error: no se pudo normalizar el TOML: {error}"));
            return 1;
        }
    };
    let model = match parse_model(&cleaned, &destination) {
        Ok(model) => model,
        Err(error) => {
            ropero_core::err_line(&format!("error: modelo inválido: {error}"));
            return 2;
        }
    };
    let seen_names = scan
        .catalog
        .models
        .iter()
        .map(|model| model.model.name.clone())
        .collect();
    let errors = ropero_core::validate(
        &model,
        &ValidationContext {
            source: destination.display().to_string(),
            source_text: Some(cleaned.clone()),
            seen_names,
        },
    );
    if !errors.is_empty() {
        for error in errors {
            ropero_core::err_line(&format!("error: {}", error.message()));
        }
        return 2;
    }
    if destination.exists() {
        ropero_core::err_line(&format!("error: ya existe {}", destination.display()));
        return 2;
    }
    let Some(parent) = destination.parent() else {
        ropero_core::err_line("error: ruta de destino no válida");
        return 1;
    };
    if let Err(error) = std::fs::create_dir_all(parent) {
        ropero_core::err_line(&format!(
            "error: no se pudo crear {}: {error}",
            parent.display()
        ));
        return 1;
    }
    let temporary = destination.with_extension(format!("toml.{}.tmp", std::process::id()));
    if let Err(error) = std::fs::write(&temporary, cleaned.as_bytes())
        .and_then(|()| std::fs::hard_link(&temporary, &destination))
    {
        let _ = std::fs::remove_file(&temporary);
        ropero_core::err_line(&format!("error: no se pudo registrar el modelo: {error}"));
        return 1;
    }
    let _ = std::fs::remove_file(&temporary);
    if discarded > 0 {
        ropero_core::out_line(&format!(
            "aviso: se descartaron {discarded} campo(s) que Ropero no utiliza"
        ));
    }
    ropero_core::out_line(&format!("modelo registrado: {}", destination.display()));
    0
}

fn toml_from_flags(matches: &ArgMatches) -> Result<String, String> {
    let required = |key: &str| {
        matches
            .get_one::<String>(key)
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("falta --{key}"))
    };
    let name = required("name")?;
    let engine = required("engine")?;
    let format = required("format")?;
    let artifact = required("artifact")?;
    let context = matches
        .get_one::<String>("context")
        .map_or(Ok(0), |value| {
            value
                .parse::<u32>()
                .map_err(|_| "contexto inválido".to_string())
        })?;
    let text = ropero_core::model_draft::from_fields(
        name,
        engine,
        format,
        artifact,
        (context > 0).then_some(context),
        matches.get_one::<String>("repo").map(String::as_str),
        matches.get_one::<String>("alias").map(String::as_str),
    )?;
    let assignments = matches
        .get_many::<String>("set")
        .map(|values| values.cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    ropero_core::model_draft::apply_sets(&text, &assignments)
}
