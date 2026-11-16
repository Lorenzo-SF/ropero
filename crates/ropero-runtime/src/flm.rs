//! Operaciones delegadas a FastFlowLM (FLM).

use std::process::{Command, Stdio};

use ropero_core::{EngineManifest, ModelSpec};

/// Resultado de ejecutar un comando delegado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CommandResult {
    pub fn ok(&self) -> bool {
        self.status == 0
    }
}

/// Si el motor declara delegación de almacén FLM.
pub fn delegated(manifest: &EngineManifest) -> bool {
    manifest.install.is_some() || manifest.serve.is_some()
}

/// Tags FLM del modelo (primario y auxiliares, p. ej. `--decision`).
pub fn tags(spec: &ModelSpec) -> Vec<String> {
    ropero_core::flm::tags(spec)
}

/// Tag primario FLM del modelo.
pub fn primary_tag(spec: &ModelSpec) -> Option<String> {
    ropero_core::flm::primary_tag(spec)
}

/// Ejecuta un comando delegado y captura salida.
pub fn run_command(cmd: &[String], extra_arg: Option<&str>) -> Result<CommandResult, String> {
    let Some((program, args)) = cmd.split_first() else {
        return Err("comando delegado vacío".to_string());
    };
    let mut command = Command::new(program);
    command.args(args);
    if let Some(arg) = extra_arg {
        command.arg(arg);
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|e| format!("no se pudo ejecutar '{}': {e}", program))?;
    let status = output.status.code().unwrap_or(1);
    Ok(CommandResult {
        status,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// `flm pull <tag>` (o comando delegado equivalente).
pub fn pull(manifest: &EngineManifest, tag: &str) -> Result<CommandResult, String> {
    let cmd = manifest
        .install
        .as_ref()
        .map(|i| &i.pull)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "el motor no declara delegación de instalación (pull)".to_string())?;
    run_command(cmd, Some(tag))
}

/// `flm remove <tag>` (o comando delegado equivalente).
pub fn remove(manifest: &EngineManifest, tag: &str) -> Result<CommandResult, String> {
    let cmd = manifest
        .install
        .as_ref()
        .map(|i| &i.remove)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "el motor no declara delegación de eliminación (remove)".to_string())?;
    run_command(cmd, Some(tag))
}

/// Consulta el almacén delegado (`flm list --json`).
pub fn list(manifest: &EngineManifest) -> Result<CommandResult, String> {
    let cmd = manifest
        .install
        .as_ref()
        .map(|i| &i.list)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "el motor no declara delegación de listado".to_string())?;
    run_command(cmd, None)
}

/// Intenta determinar si algún tag del modelo está presente según `flm list --json`.
///
/// Si la consulta no es fiable, devuelve `None` (estado desconocido). La ausencia
/// confirmada del tag en una respuesta fiable devuelve `Some(false)`.
pub fn installed_status(manifest: &EngineManifest, spec: &ModelSpec) -> Option<bool> {
    let model_tags = tags(spec);
    if model_tags.is_empty() {
        return Some(false);
    }
    let result = list(manifest).ok()?;
    if !result.ok() {
        return None;
    }
    installed_from_json(&result.stdout, &model_tags)
}

/// Lee el estado de los tags declarados de la respuesta JSON de FLM.
fn installed_from_json(output: &str, tags: &[String]) -> Option<bool> {
    let json: serde_json::Value = serde_json::from_str(output).ok()?;
    let models = json.get("models")?.as_array()?;
    for tag in tags {
        let model = models.iter().find(|model| {
            ["model", "name"]
                .iter()
                .filter_map(|key| model.get(*key).and_then(serde_json::Value::as_str))
                .any(|listed| listed == tag)
        })?;
        if !model.get("installed")?.as_bool()? {
            return Some(false);
        }
    }
    Some(true)
}

/// Compone el argv de `flm serve <tag> ...` (sin el programa `flm`).
///
/// El puerto lo añade el lanzador (`--port <n>`), como en el resto de motores.
/// `--ctx-len` se añade si `ctx` es Some y no está ya en defaults/args del modelo.
pub fn serve_argv(
    manifest: &EngineManifest,
    spec: &ModelSpec,
    model_argv: &[String],
    ctx: Option<u32>,
) -> Result<Vec<String>, String> {
    let tag = primary_tag(spec).ok_or_else(|| {
        format!(
            "el modelo '{}' no declara tag FLM ([model].path o --decision)",
            spec.model.name
        )
    })?;
    let mut argv = vec!["serve".to_string(), tag];
    let defaults = manifest
        .serve
        .as_ref()
        .map(|s| s.defaults.as_slice())
        .unwrap_or(&[]);
    argv.extend(defaults.iter().cloned());
    if let Some(ctx) = ctx {
        let has_ctx = defaults.iter().any(|a| a == "--ctx-len")
            || model_argv.iter().any(|a| a == "--ctx-len");
        if !has_ctx {
            argv.push("--ctx-len".to_string());
            argv.push(ctx.to_string());
        }
    }
    argv.extend(model_argv.iter().cloned());
    Ok(argv)
}

/// Ejecuta `flm pull` para todos los tags relevantes del modelo.
/// No requiere que el tag aparezca en `flm list` para intentar el pull.
pub fn pull_model(
    manifest: &EngineManifest,
    spec: &ModelSpec,
) -> Result<Vec<CommandResult>, String> {
    let tags = tags(spec);
    if tags.is_empty() {
        return Err(format!(
            "el modelo '{}' no declara tag FLM ([model].path o --decision)",
            spec.model.name
        ));
    }
    let mut results = Vec::new();
    for tag in tags {
        let res = pull(manifest, &tag)?;
        let ok = res.ok();
        results.push(res);
        if !ok {
            break;
        }
    }
    Ok(results)
}

/// Ejecuta `flm remove` para el tag primario del modelo.
pub fn remove_model(manifest: &EngineManifest, spec: &ModelSpec) -> Result<CommandResult, String> {
    let tag = primary_tag(spec).ok_or_else(|| {
        format!(
            "el modelo '{}' no declara tag FLM ([model].path o --decision)",
            spec.model.name
        )
    })?;
    remove(manifest, &tag)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropero_core::{parse_engine_manifest, parse_model};
    use std::path::Path;

    fn flm_manifest() -> EngineManifest {
        let toml = r#"
id = "fastflowlm"
label = "FLM (NPU)"
color = "rojo"
description = "FastFlowLM"
maturity = "proven"
formats = ["q4nx"]
port_min = 8400
port_max = 8499
deps = ["git"]
[detect]
binaries = ["flm"]
paths = ["/usr/local/bin/flm"]
[serve]
defaults = ["--host", "127.0.0.1", "--q-len", "20"]
[install]
pull = ["flm", "pull"]
remove = ["flm", "remove"]
list = ["flm", "list", "--json"]
"#;
        parse_engine_manifest(toml, Path::new("fastflowlm.toml")).expect("manifest")
    }

    #[test]
    fn serve_argv_pone_tag_antes_de_flags_y_defaults() {
        let manifest = flm_manifest();
        let spec = parse_model(
            r#"
[model]
name = "lfm3"
engine = "fastflowlm"
context = 131072
format = "q4nx"
path = "lfm3:1.7b"
[files]
[args]
base = ["--prefill-chunk-len", "8192"]
"#,
            Path::new("lfm3.toml"),
        )
        .expect("model");
        let argv = serve_argv(
            &manifest,
            &spec,
            &["--prefill-chunk-len".into(), "8192".into()],
            Some(4096),
        )
        .expect("argv");
        assert_eq!(
            argv,
            vec![
                "serve",
                "lfm3:1.7b",
                "--host",
                "127.0.0.1",
                "--q-len",
                "20",
                "--ctx-len",
                "4096",
                "--prefill-chunk-len",
                "8192",
            ]
        );
    }

    #[test]
    fn serve_argv_deriva_tag_de_decision() {
        let manifest = flm_manifest();
        let spec = parse_model(
            r#"
[model]
name = "d3mini"
engine = "fastflowlm"
context = 32768
format = "q4nx"
path = ""
[files]
[args]
base = ["--decision", "d3:mini"]
"#,
            Path::new("d3mini.toml"),
        )
        .expect("model");
        let argv = serve_argv(
            &manifest,
            &spec,
            &["--decision".into(), "d3:mini".into()],
            None,
        )
        .expect("argv");
        assert_eq!(argv[0], "serve");
        assert_eq!(argv[1], "d3:mini");
    }

    #[test]
    fn installed_status_respeta_el_booleano_del_objeto_json() {
        let tags = vec!["lfm3:1.7b".to_string()];
        assert_eq!(
            installed_from_json(
                r#"{"models":[{"model":"lfm3:1.7b","installed":false}]}"#,
                &tags,
            ),
            Some(false)
        );
        assert_eq!(
            installed_from_json(
                r#"{"models":[{"model":"lfm3:1.7b","installed":true}]}"#,
                &tags,
            ),
            Some(true)
        );
    }

    #[test]
    fn installed_status_es_desconocido_si_no_aparece_el_tag() {
        let tags = vec!["usuario:tag-no-catalogado".to_string()];
        assert_eq!(
            installed_from_json(
                r#"{"models":[{"model":"d3:mini","installed":true}]}"#,
                &tags
            ),
            None
        );
    }
}
