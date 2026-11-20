//! Descargas delegadas a herramientas declaradas por cada motor.

use ropero_core::{EngineId, EngineManifest, ModelSpec};

/// Ejecuta la operación `pull` del engine con el modelo y sus argumentos extra.
pub fn pull_model(
    manifest: &EngineManifest,
    spec: &ModelSpec,
    env: &ropero_core::Environment,
) -> Result<Vec<crate::flm::CommandResult>, String> {
    let command = manifest
        .install
        .as_ref()
        .map(|install| install.pull.as_slice())
        .filter(|command| !command.is_empty())
        .ok_or_else(|| {
            format!(
                "el engine '{}' no declara una herramienta de descarga",
                manifest.id.as_str()
            )
        })?;
    let tool = spec
        .tool
        .as_ref()
        .ok_or_else(|| "el modelo no declara [tool]".to_string())?;
    let repo = spec
        .download
        .as_ref()
        .map(|download| download.repo.as_str())
        .unwrap_or("");
    let file = spec
        .download
        .as_ref()
        .map(|download| download.file.as_str())
        .unwrap_or("");
    let dir = env
        .engine_artifact_path(spec.model.engine, "ARTIFACTS_DIR", "models")
        .join(ropero_core::real_name(spec));
    let mut args: Vec<String> = Vec::new();
    let tags = if manifest.id == EngineId::FastFlowLm {
        crate::flm::tags(spec)
    } else if !repo.is_empty() {
        vec![repo.to_string()]
    } else if !spec.model.path.is_empty() {
        vec![spec.model.path.clone()]
    } else {
        vec![spec.model.name.clone()]
    };
    if tags.is_empty() {
        return Err(format!(
            "el modelo '{}' no proporciona el identificador que necesita el engine",
            spec.model.name
        ));
    }
    if manifest.id == EngineId::Basalt {
        if file.is_empty() {
            return Err("los modelos Basalt con [tool] deben declarar [download].file".into());
        }
        args.push(file.to_string());
        args.push("--local-dir".into());
        args.push(dir.to_string_lossy().into_owned());
    } else if manifest.id == EngineId::Ignis {
        args.push("--out".into());
        args.push(dir.to_string_lossy().into_owned());
    }
    let params = tool
        .params
        .iter()
        .map(|value| {
            value
                .replace("${model}", &spec.model.name)
                .replace("${repo}", repo)
                .replace("${file}", file)
                .replace("${dir}", &dir.to_string_lossy())
        })
        .collect::<Vec<_>>();
    if params
        .iter()
        .any(|value| value.contains(['\n', '\r', '\0']))
    {
        return Err("[tool].params no puede contener caracteres de control".into());
    }
    args.extend(params);
    let mut results = Vec::new();
    for tag in tags {
        let mut command_args = vec![tag.as_str()];
        command_args.extend(args.iter().map(String::as_str));
        let result = crate::flm::run_command_args(command, command_args, env)?;
        let success = result.status == 0;
        results.push(result);
        if !success {
            break;
        }
    }
    Ok(results)
}
