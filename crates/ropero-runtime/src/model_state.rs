//! Cambio reversible del estado de un modelo moviendo su TOML entre el catálogo activo y `fired/`.

use std::io::Write;
use std::path::{Path, PathBuf};

/// Nombre público que el engine recibe al servir un modelo.
pub fn serving_name(
    config: &ropero_core::config::Config,
    engine: &str,
    model: &str,
) -> Option<String> {
    let (engine, _) = ropero_core::EngineId::from_str_legacy(engine).ok()?;
    let path = config
        .models_toml_dir(engine)
        .valor
        .join(format!("{model}.toml"));
    let source = std::fs::read_to_string(path).ok()?;
    let value = source.parse::<toml::Table>().ok()?;
    let model_table = value.get("model")?.as_table()?;
    model_table
        .get("alias")
        .or_else(|| model_table.get("path"))?
        .as_str()
        .map(str::to_owned)
}

/// Resultado validado para mover un modelo entre catálogo activo y `fired/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeError {
    Unknown,
    AlreadyActive,
    AlreadyFired,
    NameConflict(String),
    Cancelled,
    Failed(String),
}

impl std::fmt::Display for ChangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unknown => write!(f, "modelo inexistente"),
            Self::AlreadyActive => write!(f, "el modelo ya está activo"),
            Self::AlreadyFired => write!(f, "el modelo ya está despedido"),
            Self::NameConflict(name) => write!(f, "el nombre '{name}' ya lo usa un modelo activo"),
            Self::Cancelled => f.write_str("se canceló la reactivación; el modelo sigue despedido"),
            Self::Failed(error) => f.write_str(error),
        }
    }
}

/// Resuelve interactivamente un conflicto de nombre al reactivar.
pub fn change_interactive(
    catalog: &ropero_core::Catalog,
    catalog_base: &Path,
    name: &str,
    fired: bool,
    english: bool,
) -> Result<PathBuf, ChangeError> {
    let mut new_name: Option<String> = None;
    loop {
        match change(catalog, catalog_base, name, fired, new_name.as_deref()) {
            Err(ChangeError::NameConflict(conflict)) if !fired => {
                if !super::install::hay_tty() {
                    return Err(ChangeError::NameConflict(conflict));
                }
                let prompt = if english {
                    format!(
                        "'{conflict}' is already active. Enter a new unique model name (empty cancels): "
                    )
                } else {
                    format!(
                        "'{conflict}' ya está activo. Introduce un nombre único (vacío cancela): "
                    )
                };
                let Some(answer) = super::install::pregunta(&prompt) else {
                    return Err(ChangeError::Cancelled);
                };
                if answer.is_empty() {
                    return Err(ChangeError::Cancelled);
                }
                new_name = Some(answer);
            }
            result => return result,
        }
    }
}

/// Ejecuta el cambio y presenta su resultado en el idioma configurado.
pub fn run(
    catalog: &ropero_core::Catalog,
    catalog_base: &Path,
    name: &str,
    fired: bool,
    english: bool,
) -> i32 {
    match change_interactive(catalog, catalog_base, name, fired, english) {
        Ok(path) => {
            let message = match (fired, english) {
                (true, true) => "model fired",
                (true, false) => "modelo despedido",
                (false, true) => "model hired",
                (false, false) => "modelo activado",
            };
            ropero_core::out_line(&format!("{message}: {}", path.display()));
            0
        }
        Err(error) => {
            let message = if english {
                match &error {
                    ChangeError::Unknown => "model does not exist".to_string(),
                    ChangeError::AlreadyActive => "model is already active".to_string(),
                    ChangeError::AlreadyFired => "model is already fired".to_string(),
                    ChangeError::NameConflict(name) => format!(
                        "active model name '{name}' already exists; rerun in a terminal to choose a new name"
                    ),
                    ChangeError::Cancelled => "cancelled; model remains fired".to_string(),
                    ChangeError::Failed(error) => error.clone(),
                }
            } else {
                error.to_string()
            };
            ropero_core::err_line(&format!("error: {message}"));
            1
        }
    }
}

/// Comprueba el estado y la unicidad del nombre antes de mover el TOML.
pub fn change(
    catalog: &ropero_core::Catalog,
    catalog_base: &Path,
    name: &str,
    fired: bool,
    new_name: Option<&str>,
) -> Result<PathBuf, ChangeError> {
    let (source_list, other_list) = if fired {
        (&catalog.models, &catalog.fired)
    } else {
        (&catalog.fired, &catalog.models)
    };
    let matches = |model: &ropero_core::ModelSpec| {
        model.model.name == name || ropero_core::real_name(model) == name
    };
    let model = source_list
        .iter()
        .find(|model| matches(model))
        .ok_or_else(|| {
            if other_list.iter().any(matches) {
                if fired {
                    ChangeError::AlreadyFired
                } else {
                    ChangeError::AlreadyActive
                }
            } else {
                ChangeError::Unknown
            }
        })?;
    if !fired {
        let target_name = new_name
            .map(str::to_owned)
            .unwrap_or_else(|| ropero_core::real_name(model));
        if catalog
            .models
            .iter()
            .any(|active| active.source != model.source && active.model.name == target_name)
        {
            return Err(ChangeError::NameConflict(target_name));
        }
    }
    set_fired(&model.source, catalog_base, fired, new_name).map_err(ChangeError::Failed)
}

/// Mueve el TOML a `fired/` o lo devuelve al catálogo activo.
///
/// `new_name` renombra `[model].name` y el fichero al reactivar, preservando comentarios TOML.
pub fn set_fired(
    source: &Path,
    catalog_base: &Path,
    fired: bool,
    new_name: Option<&str>,
) -> Result<PathBuf, String> {
    if fired && new_name.is_some() {
        return Err("sólo se puede cambiar el nombre al reactivar un modelo".into());
    }
    let metadata = std::fs::symlink_metadata(source)
        .map_err(|error| format!("no se pudo inspeccionar {}: {error}", source.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "el TOML del modelo no es un fichero regular: {}",
            source.display()
        ));
    }
    if source
        .extension()
        .is_none_or(|extension| extension != "toml")
    {
        return Err(format!("el origen no es un TOML: {}", source.display()));
    }
    let parent = source
        .parent()
        .ok_or_else(|| format!("el TOML no tiene directorio padre: {}", source.display()))?;
    let is_fired = parent.file_name().is_some_and(|name| name == "fired");
    if is_fired == fired {
        return Err(if fired {
            "el modelo ya está despedido".into()
        } else {
            "el modelo ya está activo".into()
        });
    }
    let root = std::fs::canonicalize(catalog_base)
        .map_err(|error| format!("no se pudo resolver {}: {error}", catalog_base.display()))?;
    let canonical_parent = std::fs::canonicalize(parent)
        .map_err(|error| format!("no se pudo resolver {}: {error}", parent.display()))?;
    if !canonical_parent.starts_with(&root)
        || !source.canonicalize().is_ok_and(|p| p.starts_with(&root))
    {
        return Err("el TOML está fuera de la raíz del catálogo".into());
    }
    let engine_dir = if is_fired {
        parent
            .parent()
            .ok_or_else(|| format!("ruta fired inválida: {}", source.display()))?
    } else {
        parent
    };
    let old_name = source
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("nombre de TOML inválido: {}", source.display()))?;
    let name = new_name.unwrap_or(old_name);
    if !model_name_valid(name) {
        return Err(
            "el nombre debe usar minúsculas, números, puntos, guiones o guiones bajos".into(),
        );
    }
    let destination_dir = if fired {
        engine_dir.join("fired")
    } else {
        engine_dir.to_path_buf()
    };
    std::fs::create_dir_all(&destination_dir)
        .map_err(|error| format!("no se pudo crear {}: {error}", destination_dir.display()))?;
    let destination = destination_dir.join(format!("{name}.toml"));
    if destination.exists() {
        return Err(format!(
            "ya existe el TOML de destino {}",
            destination.display()
        ));
    }
    let rewritten = if let Some(name) = new_name {
        let text = std::fs::read_to_string(source)
            .map_err(|error| format!("no se pudo leer {}: {error}", source.display()))?;
        let mut doc = text
            .parse::<toml_edit::DocumentMut>()
            .map_err(|error| format!("TOML inválido en {}: {error}", source.display()))?;
        let model = doc
            .get_mut("model")
            .and_then(toml_edit::Item::as_table_like_mut)
            .ok_or_else(|| "el TOML no contiene una tabla [model] válida".to_string())?;
        model.insert("name", toml_edit::value(name));
        Some(doc.to_string())
    } else {
        None
    };
    let linked = if let Some(text) = rewritten {
        let temp = destination_dir.join(format!(".{}.tmp-{}", name, std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("no se pudo reservar {}: {error}", temp.display()))?;
        let write = file
            .write_all(text.as_bytes())
            .and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = write {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("no se pudo escribir {}: {error}", temp.display()));
        }
        if let Err(error) = std::fs::set_permissions(&temp, metadata.permissions()) {
            let _ = std::fs::remove_file(&temp);
            return Err(format!("no se pudieron conservar permisos TOML: {error}"));
        }
        let result = std::fs::hard_link(&temp, &destination);
        let _ = std::fs::remove_file(&temp);
        result
    } else {
        std::fs::hard_link(source, &destination)
    };
    linked.map_err(|error| {
        format!(
            "no se pudo mover el TOML a {}: {error}",
            destination.display()
        )
    })?;
    if let Err(error) = std::fs::remove_file(source) {
        let _ = std::fs::remove_file(&destination);
        return Err(format!(
            "no se pudo retirar el origen {}: {error}",
            source.display()
        ));
    }
    Ok(destination)
}

fn model_name_valid(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|part| {
            !part.is_empty()
                && part.chars().all(|ch| {
                    ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '_' | '.')
                })
        })
}

#[cfg(test)]
mod tests {
    use super::{ChangeError, change, set_fired};
    use ropero_core::{Catalog, ModelSpec, parse_model};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    const TOML: &str =
        "# keep this comment\n[model]\nname = \"assistant\"\nengine = \"turbo-llama\"\n\n[files]\n";

    fn base() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "ropero-model-state-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn fixture(root: &Path) -> PathBuf {
        let path = root.join("models/turbo-llama/assistant.toml");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, TOML).unwrap();
        path
    }

    fn spec(name: &str, source: PathBuf) -> ModelSpec {
        parse_model(
            &format!("[model]\nname = \"{name}\"\nengine = \"turbo-llama\"\nformat = \"gguf\"\n\n[files]\ngguf = \"{name}.gguf\"\n"),
            &source,
        )
        .unwrap()
    }

    #[test]
    fn cambio_rechaza_estados_inexistentes_y_duplicados() {
        let root = base();
        let active_path = root.join("models/turbo-llama/assistant.toml");
        std::fs::create_dir_all(active_path.parent().unwrap()).unwrap();
        std::fs::write(&active_path, TOML).unwrap();
        let active = spec("assistant", active_path.clone());
        let catalog = Catalog {
            models: vec![active.clone()],
            fired: vec![],
        };
        assert_eq!(
            change(&catalog, &root.join("models"), "missing", true, None),
            Err(ChangeError::Unknown)
        );
        assert_eq!(
            change(&catalog, &root.join("models"), "assistant", false, None),
            Err(ChangeError::AlreadyActive)
        );
        let fired_path = root.join("models/turbo-llama/fired/assistant.toml");
        std::fs::create_dir_all(fired_path.parent().unwrap()).unwrap();
        std::fs::write(&fired_path, TOML).unwrap();
        let fired = spec("assistant", fired_path);
        let catalog = Catalog {
            models: vec![active],
            fired: vec![fired],
        };
        assert_eq!(
            change(&catalog, &root.join("models"), "assistant", false, None),
            Err(ChangeError::NameConflict("assistant".into()))
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn despide_y_reactive_mueve_el_toml_sin_cambiarlo() {
        let root = base();
        let source = fixture(&root);
        let fired = set_fired(&source, &root.join("models"), true, None).unwrap();
        assert_eq!(fired, root.join("models/turbo-llama/fired/assistant.toml"));
        assert_eq!(std::fs::read_to_string(&fired).unwrap(), TOML);
        let active = set_fired(&fired, &root.join("models"), false, None).unwrap();
        assert_eq!(active, source);
        assert_eq!(std::fs::read_to_string(active).unwrap(), TOML);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reactivar_con_nombre_nuevo_preserva_comentarios_y_renombra_fichero() {
        let root = base();
        let active = fixture(&root);
        let fired = set_fired(&active, &root.join("models"), true, None).unwrap();
        let renamed =
            set_fired(&fired, &root.join("models"), false, Some("assistant-alt")).unwrap();
        let text = std::fs::read_to_string(&renamed).unwrap();
        assert_eq!(renamed.file_name().unwrap(), "assistant-alt.toml");
        assert!(text.contains("# keep this comment"));
        assert!(text.contains("name = \"assistant-alt\""));
        assert!(!fired.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn no_sobrescribe_un_toml_activo_existente() {
        let root = base();
        let source = fixture(&root);
        let fired = set_fired(&source, &root.join("models"), true, None).unwrap();
        let active = root.join("models/turbo-llama/assistant.toml");
        std::fs::write(&active, "# existing\n").unwrap();
        assert!(set_fired(&fired, &root.join("models"), false, None).is_err());
        assert!(fired.exists());
        assert_eq!(std::fs::read_to_string(active).unwrap(), "# existing\n");
        let _ = std::fs::remove_dir_all(root);
    }
}
