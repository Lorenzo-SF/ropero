//! Configuración del id público usado por MLX-Serve.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use ropero_core::{EngineId, Environment, ModelFormat, ModelSpec, resolve_paths};

/// Guarda el alias declarado en el archivo global de ajustes de MLX-Serve.
///
/// MLX-Serve indexa `~/.mlx-serve/model-settings.json` por ruta del modelo.
/// Se llama sólo después de comprobar que el modelo ya está descargado.
pub fn aplica_alias(spec: &ModelSpec, env: &Environment) -> Result<(), String> {
    if spec.model.engine != EngineId::MlxServe || spec.model.format != ModelFormat::Safetensors {
        return Ok(());
    }
    let Some(alias) = spec.model.alias.as_deref() else {
        return Ok(());
    };
    valida_alias(alias)?;
    let model_path = resolve_paths(spec, env).model_path.ok_or_else(|| {
        "MLX-Serve necesita una ruta local de modelo para configurar el alias".to_string()
    })?;
    let model_path = PathBuf::from(&model_path);
    if !model_path.is_dir() {
        return Err(format!(
            "no existe el directorio del modelo MLX: {}",
            model_path.display()
        ));
    }
    let settings_path = env.home.join(".mlx-serve").join("model-settings.json");
    actualiza_settings(&settings_path, &model_path.to_string_lossy(), alias)
}

fn valida_alias(alias: &str) -> Result<(), String> {
    if alias.is_empty()
        || alias.len() > 128
        || alias == "mlx-serve"
        || alias.chars().any(char::is_whitespace)
        || alias.as_bytes().contains(&0x7f)
        || alias
            .chars()
            .any(|c| matches!(c, '@' | '/' | '\\' | '"' | '\''))
    {
        return Err(format!("alias MLX-Serve no válido: '{alias}'"));
    }
    Ok(())
}

fn actualiza_settings(path: &Path, model_path: &str, alias: &str) -> Result<(), String> {
    let existing_permissions = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!(
                "no se actualiza el enlace simbólico de ajustes {}",
                path.display()
            ));
        }
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("no se pudo consultar {}: {error}", path.display())),
    };
    let mut settings: serde_json::Value = if path.exists() {
        let contents = fs::read_to_string(path)
            .map_err(|e| format!("no se pudo leer {}: {e}", path.display()))?;
        serde_json::from_str(&contents)
            .map_err(|e| format!("JSON inválido en {}: {e}", path.display()))?
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };
    let Some(models) = settings.as_object_mut() else {
        return Err(format!("{} debe contener un objeto JSON", path.display()));
    };
    let model_settings = models
        .entry(model_path.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let Some(model_settings) = model_settings.as_object_mut() else {
        return Err(format!(
            "los ajustes de '{model_path}' en {} deben ser un objeto JSON",
            path.display()
        ));
    };
    model_settings.insert("alias".into(), serde_json::Value::String(alias.into()));

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
    }

    let temporary = temp_path(path);
    let contents = serde_json::to_vec_pretty(&settings)
        .map_err(|e| format!("no se pudo serializar {}: {e}", path.display()))?;
    let mut file = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
    {
        Ok(file) => file,
        Err(error) => {
            return Err(format!("no se pudo crear {}: {error}", temporary.display()));
        }
    };
    if let Some(permissions) = existing_permissions
        && let Err(error) = file.set_permissions(permissions)
    {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "no se pudieron conservar los permisos de {}: {error}",
            path.display()
        ));
    }
    if let Err(error) = file.write_all(&contents).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = fs::remove_file(&temporary);
        return Err(format!(
            "no se pudo escribir {}: {error}",
            temporary.display()
        ));
    }
    drop(file);
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(format!("no se pudo actualizar {}: {error}", path.display()));
    }
    Ok(())
}

fn temp_path(path: &Path) -> PathBuf {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    path.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropero_core::parse_model;
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("ropero-mlx-alias-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn actualiza_alias_y_preserva_ajustes_existentes() {
        let dir = temp_dir();
        let path = dir.join("model-settings.json");
        fs::write(
            &path,
            r#"{"/models/qwen":{"temperature":0.4,"keep":true},"/models/other":{"alias":"other"}}"#,
        )
        .unwrap();
        actualiza_settings(&path, "/models/qwen", "qwen-public").unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["/models/qwen"]["alias"], "qwen-public");
        assert_eq!(value["/models/qwen"]["temperature"], 0.4);
        assert_eq!(value["/models/qwen"]["keep"], true);
        assert_eq!(value["/models/other"]["alias"], "other");
        fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn conserva_permisos_restrictivos_del_archivo_de_settings() {
        use std::os::unix::fs::PermissionsExt;

        let dir = temp_dir();
        let path = dir.join("model-settings.json");
        fs::write(&path, r#"{"/models/qwen":{"alias":"old"}}"#).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        actualiza_settings(&path, "/models/qwen", "new").unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn rechaza_alias_que_mlx_no_acepta() {
        for alias in [
            "",
            "mlx-serve",
            "qwen public",
            "x/y",
            "x@y",
            "x\\y",
            "x\"y",
            "x\u{7f}y",
        ] {
            assert!(valida_alias(alias).is_err(), "{alias:?}");
        }
        assert!(valida_alias(&"a".repeat(129)).is_err());
        assert!(valida_alias("qwen-public").is_ok());
    }

    #[test]
    fn aplica_alias_al_directorio_de_modelo_resuelto() {
        let root = temp_dir();
        let model_dir = root.join("checkpoint");
        fs::create_dir_all(&model_dir).unwrap();
        let source = format!(
            "[model]\nname='demo'\nengine='mlx-serve'\ncontext=4096\nformat='safetensors'\npath='{}'\nalias='demo-public'\n[files]\n[args]\n",
            model_dir.display()
        );
        let spec = parse_model(&source, Path::new("demo.toml")).unwrap();
        let env = Environment::new(BTreeMap::new(), &root, &root);
        aplica_alias(&spec, &env).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(".mlx-serve/model-settings.json")).unwrap())
                .unwrap();
        assert_eq!(
            value[model_dir.to_string_lossy().as_ref()]["alias"],
            "demo-public"
        );
        fs::remove_dir_all(root).unwrap();
    }
}
