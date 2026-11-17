//! Captura interactiva para registrar un modelo desde la terminal.

pub fn wizard_toml() -> Result<String, String> {
    fn ask(label: &str, optional: bool) -> Result<Option<String>, String> {
        eprint!("{label}{}: ", if optional { " (opcional)" } else { "" });
        use std::io::Write;
        std::io::stderr().flush().map_err(|e| e.to_string())?;
        let mut value = String::new();
        std::io::stdin()
            .read_line(&mut value)
            .map_err(|e| e.to_string())?;
        let value = value.trim().to_string();
        if value.is_empty() && !optional {
            return Err(format!("el campo {label} es obligatorio"));
        }
        Ok((!value.is_empty()).then_some(value))
    }
    let name = ask("Nombre del modelo", false)?.unwrap();
    let engine = ask("Engine", false)?.unwrap();
    let format = ask("Formato (gguf/safetensors/q4nx/strata/ninfer)", false)?.unwrap();
    let artifact = ask("Fichero o ruta/tag", false)?.unwrap();
    let repo = ask("Repo de descarga (OWNER/NAME)", true)?;
    let context = ask("Contexto en tokens", true)?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| "contexto inválido".to_string())
        })
        .transpose()?;
    let alias = ask("Alias público", true)?;
    let mut text = ropero_core::model_draft::from_fields(
        &name,
        &engine,
        &format,
        &artifact,
        context,
        repo.as_deref(),
        alias.as_deref(),
    )?;
    let mut assignments = Vec::new();
    loop {
        eprint!("Campo TOML adicional (ej. levels.list=[\"low\",\"high\"], vacío termina): ");
        use std::io::Write;
        std::io::stderr().flush().map_err(|e| e.to_string())?;
        let mut assignment = String::new();
        if std::io::stdin()
            .read_line(&mut assignment)
            .map_err(|e| e.to_string())?
            == 0
        {
            break;
        }
        let assignment = assignment.trim();
        if assignment.is_empty() {
            break;
        }
        assignments.push(assignment.to_owned());
        text = ropero_core::model_draft::apply_sets(&text, &assignments)?;
        assignments.clear();
    }
    Ok(text)
}
