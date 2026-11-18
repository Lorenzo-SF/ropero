//! Construcción y normalización de TOML para registrar modelos nuevos.

use crate::ModelSpec;

/// Construye el esqueleto del TOML con los campos principales del modelo.
pub fn from_fields(
    name: &str,
    engine: &str,
    format: &str,
    artifact: &str,
    context: Option<u32>,
    repo: Option<&str>,
    alias: Option<&str>,
) -> Result<String, String> {
    let mut doc = toml::map::Map::new();
    let mut model = toml::map::Map::new();
    model.insert("name".into(), toml::Value::String(name.into()));
    model.insert("engine".into(), toml::Value::String(engine.into()));
    model.insert("format".into(), toml::Value::String(format.into()));
    if let Some(context) = context {
        model.insert("context".into(), toml::Value::Integer(context.into()));
    }
    if let Some(alias) = alias {
        model.insert("alias".into(), toml::Value::String(alias.into()));
    }
    doc.insert("model".into(), toml::Value::Table(model));
    let mut files = toml::map::Map::new();
    let mut model_path = None;
    match format {
        "gguf" => {
            files.insert("gguf".into(), toml::Value::String(artifact.into()));
        }
        "safetensors" | "q4nx" | "strata" | "ninfer" | "basalt" => model_path = Some(artifact),
        _ => {
            return Err(
                "formato desconocido; usa gguf, safetensors, q4nx, strata, ninfer o basalt".into(),
            );
        }
    }
    if !files.is_empty() {
        doc.insert("files".into(), toml::Value::Table(files));
    }
    if let Some(path) = model_path {
        doc.get_mut("model")
            .and_then(toml::Value::as_table_mut)
            .ok_or_else(|| "falta la tabla model".to_string())?
            .insert("path".into(), toml::Value::String(path.into()));
    }
    if let Some(repo) = repo.filter(|repo| !repo.is_empty()) {
        doc.insert(
            "download".into(),
            toml::Value::try_from(
                [
                    ("repo", repo),
                    (
                        "file",
                        if format == "basalt" {
                            std::path::Path::new(artifact)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or(artifact)
                        } else {
                            artifact
                        },
                    ),
                ]
                .into_iter()
                .collect::<std::collections::BTreeMap<_, _>>(),
            )
            .map_err(|e| e.to_string())?,
        );
    }
    toml::to_string_pretty(&toml::Value::Table(doc)).map_err(|e| e.to_string())
}

/// Aplica asignaciones TOML (`seccion.campo=valor`) al esqueleto del modelo.
/// Los valores usan sintaxis TOML, lo que admite strings, arrays y tablas.
pub fn apply_sets(text: &str, assignments: &[String]) -> Result<String, String> {
    let mut root: toml::Value = toml::from_str(text).map_err(|error| error.to_string())?;
    let root = root
        .as_table_mut()
        .ok_or_else(|| "el TOML debe tener una tabla raíz".to_string())?;
    for assignment in assignments {
        let (path, raw_value) = assignment
            .split_once('=')
            .ok_or_else(|| format!("asignación inválida `{assignment}`; usa CAMPO=VALOR"))?;
        let keys: Vec<&str> = path.trim().split('.').map(str::trim).collect();
        if keys.is_empty() || keys.iter().any(|key| key.is_empty()) {
            return Err(format!("ruta TOML inválida en `{assignment}`"));
        }
        let raw_value = raw_value.trim();
        if raw_value.is_empty() {
            return Err(format!("falta el valor de `{}`", path.trim()));
        }
        let parsed: toml::Table = toml::from_str(&format!("value = {raw_value}"))
            .map_err(|error| format!("valor TOML inválido en `{assignment}`: {error}"))?;
        let value = parsed
            .get("value")
            .cloned()
            .ok_or_else(|| format!("valor TOML inválido en `{assignment}`"))?;
        let mut table = &mut *root;
        for key in &keys[..keys.len() - 1] {
            let entry = table
                .entry((*key).to_owned())
                .or_insert_with(|| toml::Value::Table(toml::Table::new()));
            table = entry
                .as_table_mut()
                .ok_or_else(|| format!("`{key}` no es una tabla en `{assignment}`"))?;
        }
        table.insert(keys[keys.len() - 1].to_owned(), value);
    }
    toml::to_string_pretty(&toml::Value::Table(root.clone())).map_err(|error| error.to_string())
}

/// Elimina claves ajenas al esquema de Ropero y devuelve el TOML limpio y el
/// número de campos descartados.
pub fn clean(text: &str) -> Result<(String, usize), toml::de::Error> {
    let mut value = toml::from_str::<toml::Value>(text)?;
    let mut discarded = 0;
    let Some(root) = value.as_table_mut() else {
        return Ok((text.to_string(), 0));
    };
    discarded += retain_keys(
        root,
        &[
            "model", "files", "download", "build", "levels", "flags", "options", "aliases",
            "limits", "args",
        ],
    );
    for (table, allowed) in [
        (
            "model",
            &[
                "name", "engine", "context", "alias", "port", "format", "path",
            ][..],
        ),
        ("files", &["gguf", "mtp", "mmproj", "chat_template"][..]),
        (
            "download",
            &[
                "repo", "gguf", "file", "name", "size", "manual", "excludes", "aux",
            ][..],
        ),
        (
            "build",
            &[
                "repo",
                "gguf",
                "gguf_file",
                "quant",
                "imatrix",
                "imatrix_repo",
                "imatrix_file",
                "out",
                "out_principal",
                "from_subdir",
                "include",
                "intermedio",
                "keep_source",
                "keep_intermedio",
                "no_mtp",
                "no_dflash",
                "force",
                "size",
            ][..],
        ),
        ("levels", &["list", "default"][..]),
        ("flags", &["list"][..]),
        ("options", &["list", "default"][..]),
        ("limits", &["min_ram_gb", "min_ram_note"][..]),
        ("args", &["base", "level", "flag", "option"][..]),
    ] {
        if let Some(table) = root.get_mut(table).and_then(toml::Value::as_table_mut) {
            discarded += retain_keys(table, allowed);
        }
    }
    if let Some(download) = root.get_mut("download").and_then(toml::Value::as_table_mut)
        && let Some(aux) = download.get_mut("aux").and_then(toml::Value::as_table_mut)
    {
        for entry in aux
            .iter_mut()
            .map(|(_, value)| value)
            .filter_map(toml::Value::as_table_mut)
        {
            discarded += retain_keys(entry, &["file", "name", "repo", "size", "use"]);
        }
    }
    if let Some(args) = root.get_mut("args").and_then(toml::Value::as_table_mut) {
        for section in ["level", "flag", "option"] {
            if let Some(groups) = args.get_mut(section).and_then(toml::Value::as_table_mut) {
                for entry in groups
                    .iter_mut()
                    .map(|(_, value)| value)
                    .filter_map(toml::Value::as_table_mut)
                {
                    let allowed: &[&str] = match section {
                        "level" => &["base", "chat_template_kwargs", "reasoning_effort"],
                        "flag" => &["base", "requires"],
                        _ => &["base", "values", "replace"],
                    };
                    discarded += retain_keys(entry, allowed);
                }
            }
        }
        if let Some(options) = args.get_mut("option").and_then(toml::Value::as_table_mut) {
            for entry in options
                .iter_mut()
                .map(|(_, value)| value)
                .filter_map(toml::Value::as_table_mut)
            {
                if let Some(values) = entry.get_mut("values").and_then(toml::Value::as_table_mut) {
                    for value in values
                        .iter_mut()
                        .map(|(_, value)| value)
                        .filter_map(toml::Value::as_table_mut)
                    {
                        discarded += retain_keys(value, &["base"]);
                    }
                }
            }
        }
    }
    Ok((
        toml::to_string_pretty(&value).unwrap_or_default(),
        discarded,
    ))
}

/// Escribe el identificador canónico del engine en el modelo normalizado.
pub fn normalize_engine(mut value: toml::Value, model: &ModelSpec) -> toml::Value {
    if let Some(table) = value.get_mut("model").and_then(toml::Value::as_table_mut) {
        table.insert(
            "engine".into(),
            toml::Value::String(model.model.engine.as_str().into()),
        );
    }
    value
}

fn retain_keys(table: &mut toml::map::Map<String, toml::Value>, allowed: &[&str]) -> usize {
    let unknown = table
        .keys()
        .filter(|key| !allowed.contains(&key.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    for key in &unknown {
        table.remove(key);
    }
    unknown.len()
}

#[cfg(test)]
mod tests {
    use super::{apply_sets, from_fields};

    #[test]
    fn los_campos_avanzados_admiten_valores_toml_y_crean_tablas() {
        let text = from_fields(
            "demo",
            "llama-server",
            "gguf",
            "demo.gguf",
            None,
            None,
            None,
        )
        .unwrap();
        let text = apply_sets(
            &text,
            &[
                "levels.list = [\"low\", \"high\"]".into(),
                "args.base = [\"--ctx-size\", \"4096\"]".into(),
            ],
        )
        .unwrap();
        let value: toml::Value = toml::from_str(&text).unwrap();
        assert_eq!(value["levels"]["list"][1].as_str(), Some("high"));
        assert_eq!(value["args"]["base"][0].as_str(), Some("--ctx-size"));
    }

    #[test]
    fn rechaza_asignaciones_y_rutas_que_no_son_tablas() {
        let text = from_fields(
            "demo",
            "llama-server",
            "gguf",
            "demo.gguf",
            None,
            None,
            None,
        )
        .unwrap();
        assert!(apply_sets(&text, &["levels.list".into()]).is_err());
        assert!(apply_sets(&text, &["model.name.child=1".into()]).is_err());
    }
}
