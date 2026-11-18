//! Operaciones delegadas a FastFlowLM (FLM).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ropero_core::{EngineManifest, ModelSpec};

/// Entorno que necesitan las instalaciones AMD XRT ubicadas bajo `/opt`.
///
/// Las distribuciones que instalan XRT en `/usr` ya publican sus bibliotecas
/// mediante el loader y no necesitan esta adaptación. AMD instala su stack
/// completo en `/opt/xilinx/xrt`; su `setup.sh` no se ejecuta por defecto al
/// arrancar una shell no interactiva, así que Ropero debe propagar esas rutas.
pub fn xrt_environment(vars: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let Some(root) = xrt_root(vars) else {
        return BTreeMap::new();
    };

    let mut result = BTreeMap::new();
    result.insert("XILINX_XRT".to_string(), root.display().to_string());
    for (name, addition) in [
        ("PATH", root.join("bin")),
        ("LD_LIBRARY_PATH", root.join("lib")),
        ("PKG_CONFIG_PATH", root.join("lib/pkgconfig")),
        ("PYTHONPATH", root.join("python")),
    ] {
        let Some(value) = prepend_path(&addition, vars.get(name).map(String::as_str)) else {
            continue;
        };
        result.insert(name.to_string(), value);
    }
    result
}

/// Añade XRT de AMD al entorno capturado por la CLI para que doctor, detección
/// de dependencias e instalación vean `xrt-smi` bajo `/opt`.
pub fn augment_environment(vars: &mut BTreeMap<String, String>) {
    for (name, value) in xrt_environment(vars) {
        vars.insert(name, value);
    }
}

/// Encuentra la raíz del runtime de XRT de AMD, respetando primero
/// `XILINX_XRT` y luego su ubicación estándar.
pub fn xrt_root(vars: &BTreeMap<String, String>) -> Option<PathBuf> {
    vars.get("XILINX_XRT")
        .map(PathBuf::from)
        .filter(|root| xrt_root_valid(root))
        .or_else(|| {
            let root = PathBuf::from("/opt/xilinx/xrt");
            xrt_root_valid(&root).then_some(root)
        })
}

fn xrt_root_valid(root: &Path) -> bool {
    root.join("setup.sh").is_file() && root.join("bin/xrt-smi").is_file()
}

fn prepend_path(addition: &Path, current: Option<&str>) -> Option<String> {
    if !addition.is_dir() {
        return current.map(str::to_string);
    }
    let current_paths = current.map(std::env::split_paths).into_iter().flatten();
    let mut paths = vec![addition.to_path_buf()];
    paths.extend(current_paths.filter(|path| path != addition));
    std::env::join_paths(paths)
        .ok()
        .map(|value| value.to_string_lossy().into_owned())
}

/// Añade las rutas de XRT a un comando hijo sin depender de que el usuario
/// haya hecho `source /opt/xilinx/xrt/setup.sh` en su shell.
pub fn configure_xrt_command(command: &mut Command, vars: &BTreeMap<String, String>) {
    command.envs(xrt_environment(vars));
}

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
pub fn run_command(
    cmd: &[String],
    extra_arg: Option<&str>,
    env: &ropero_core::Environment,
) -> Result<CommandResult, String> {
    let Some((program, args)) = cmd.split_first() else {
        return Err("comando delegado vacío".to_string());
    };
    let resolved_program = crate::install::which_en(env, program).unwrap_or_else(|| {
        let managed = crate::install::bin_dir(env).join(program);
        if managed.is_file() {
            managed
        } else {
            PathBuf::from(program)
        }
    });
    let mut command = Command::new(&resolved_program);
    command.args(args);
    if let Some(path) = env.get("PATH") {
        command.env("PATH", path);
    }
    command.envs(xrt_environment(&env.vars));
    if let Some(arg) = extra_arg {
        command.arg(arg);
    }
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let output = command
        .output()
        .map_err(|e| format!("no se pudo ejecutar '{}': {e}", resolved_program.display()))?;
    let status = output.status.code().unwrap_or(1);
    Ok(CommandResult {
        status,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

/// `flm pull <tag>` (o comando delegado equivalente).
pub fn pull(
    manifest: &EngineManifest,
    tag: &str,
    env: &ropero_core::Environment,
) -> Result<CommandResult, String> {
    let cmd = manifest
        .install
        .as_ref()
        .map(|i| &i.pull)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "el motor no declara delegación de instalación (pull)".to_string())?;
    run_command(cmd, Some(tag), env)
}

/// `flm remove <tag>` (o comando delegado equivalente).
pub fn remove(
    manifest: &EngineManifest,
    tag: &str,
    env: &ropero_core::Environment,
) -> Result<CommandResult, String> {
    let cmd = manifest
        .install
        .as_ref()
        .map(|i| &i.remove)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "el motor no declara delegación de eliminación (remove)".to_string())?;
    run_command(cmd, Some(tag), env)
}

/// Consulta el almacén delegado (`flm list --json`).
pub fn list(
    manifest: &EngineManifest,
    env: &ropero_core::Environment,
) -> Result<CommandResult, String> {
    let cmd = manifest
        .install
        .as_ref()
        .map(|i| &i.list)
        .filter(|c| !c.is_empty())
        .ok_or_else(|| "el motor no declara delegación de listado".to_string())?;
    run_command(cmd, None, env)
}

/// Intenta determinar si algún tag del modelo está presente según `flm list --json`.
///
/// Si la consulta no es fiable, devuelve `None` (estado desconocido). La ausencia
/// confirmada del tag en una respuesta fiable devuelve `Some(false)`.
pub fn installed_status(
    manifest: &EngineManifest,
    spec: &ModelSpec,
    env: &ropero_core::Environment,
) -> Option<bool> {
    let model_tags = tags(spec);
    if model_tags.is_empty() {
        return Some(false);
    }
    let result = list(manifest, env).ok()?;
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
    argv.extend(defaults_without_model_overrides(defaults, model_argv));
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

/// Retira de los defaults las opciones que el modelo declara explícitamente.
/// Así Ropero aplica el override antes de llamar a FLM, sin depender de cómo
/// esa CLI resuelva dos valores para la misma opción.
fn defaults_without_model_overrides(defaults: &[String], model_argv: &[String]) -> Vec<String> {
    let overrides = model_argv
        .iter()
        .map(|argument| {
            argument
                .split_once('=')
                .map_or(argument.as_str(), |(key, _)| key)
        })
        .filter(|argument| argument.starts_with("--"))
        .map(str::to_string)
        .collect::<std::collections::BTreeSet<_>>();
    let mut kept = Vec::with_capacity(defaults.len());
    let mut index = 0;
    while index < defaults.len() {
        let argument = &defaults[index];
        let key = argument
            .split_once('=')
            .map_or(argument.as_str(), |(key, _)| key);
        let replaced = overrides.contains(key);
        let has_value = !argument.contains('=')
            && defaults
                .get(index + 1)
                .is_some_and(|value| !value.starts_with("--"));
        if !replaced {
            kept.push(argument.clone());
            if has_value && let Some(value) = defaults.get(index + 1) {
                kept.push(value.clone());
            }
        }
        index += if has_value { 2 } else { 1 };
    }
    kept
}

/// Ejecuta `flm pull` para todos los tags relevantes del modelo.
/// No requiere que el tag aparezca en `flm list` para intentar el pull.
pub fn pull_model(
    manifest: &EngineManifest,
    spec: &ModelSpec,
    env: &ropero_core::Environment,
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
        let res = pull(manifest, &tag, env)?;
        let ok = res.ok();
        results.push(res);
        if !ok {
            break;
        }
    }
    Ok(results)
}

/// Ejecuta `flm remove` para el tag primario del modelo.
pub fn remove_model(
    manifest: &EngineManifest,
    spec: &ModelSpec,
    env: &ropero_core::Environment,
) -> Result<CommandResult, String> {
    let tag = primary_tag(spec).ok_or_else(|| {
        format!(
            "el modelo '{}' no declara tag FLM ([model].path o --decision)",
            spec.model.name
        )
    })?;
    remove(manifest, &tag, env)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ropero_core::{parse_engine_manifest, parse_model};
    use std::path::Path;

    fn flm_manifest() -> EngineManifest {
        let toml = r#"
id = "fastflowllm"
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
        parse_engine_manifest(toml, Path::new("fastflowllm.toml")).expect("manifest")
    }

    #[test]
    fn serve_argv_pone_tag_antes_de_flags_y_defaults() {
        let manifest = flm_manifest();
        let spec = parse_model(
            r#"
[model]
name = "lfm3"
engine = "fastflowllm"
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
    fn args_del_modelo_sustituyen_los_defaults_sin_repetir_la_opcion() {
        let manifest = flm_manifest();
        let spec = parse_model(
            r#"
[model]
name = "lfm3"
engine = "fastflowllm"
context = 131072
format = "q4nx"
path = "lfm3:1.7b"
[files]
[args]
base = ["--q-len", "8"]
"#,
            Path::new("lfm3.toml"),
        )
        .expect("model");
        let argv = serve_argv(&manifest, &spec, &spec.args.base, None).expect("argv");
        let positions = argv
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.as_str() == "--q-len")
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 1);
        assert_eq!(argv[positions[0] + 1], "8");
        assert!(argv.windows(2).any(|pair| pair == ["--host", "127.0.0.1"]));
    }

    #[test]
    fn serve_argv_deriva_tag_de_decision() {
        let manifest = flm_manifest();
        let spec = parse_model(
            r#"
[model]
name = "d3mini"
engine = "fastflowllm"
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

    #[test]
    fn xrt_amd_se_anade_al_entorno_de_instalacion_y_arranque() {
        let root = std::env::temp_dir().join(format!("ropero-xrt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for directory in ["bin", "lib", "lib/pkgconfig", "python"] {
            std::fs::create_dir_all(root.join(directory)).expect("crear directorio XRT");
        }
        std::fs::write(root.join("setup.sh"), "").expect("crear setup de prueba");
        std::fs::write(root.join("bin/xrt-smi"), "").expect("crear xrt-smi de prueba");

        let vars = BTreeMap::from([
            ("XILINX_XRT".to_string(), root.display().to_string()),
            ("PATH".to_string(), "/usr/bin".to_string()),
            ("LD_LIBRARY_PATH".to_string(), "/usr/lib".to_string()),
        ]);
        let runtime = xrt_environment(&vars);

        assert_eq!(runtime.get("XILINX_XRT"), vars.get("XILINX_XRT"));
        assert!(runtime["PATH"].starts_with(&root.join("bin").display().to_string()));
        assert!(runtime["LD_LIBRARY_PATH"].starts_with(&root.join("lib").display().to_string()));
        assert_eq!(xrt_root(&vars), Some(root.clone()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn los_comandos_del_almacen_flm_reciben_ld_library_path_de_xrt() {
        use std::os::unix::fs::PermissionsExt;

        let base =
            std::env::temp_dir().join(format!("ropero-flm-command-env-{}", std::process::id()));
        let root = base.join("xrt");
        let bin = base.join("flm-probe");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(root.join("bin")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(root.join("setup.sh"), "").unwrap();
        std::fs::write(root.join("bin/xrt-smi"), "").unwrap();
        std::fs::write(
            &bin,
            "#!/bin/sh\nprintf '%s|%s' \"$XILINX_XRT\" \"$LD_LIBRARY_PATH\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("XILINX_XRT".into(), root.display().to_string());
        vars.insert("LD_LIBRARY_PATH".into(), "/existing/lib".into());
        let env = ropero_core::Environment::new(vars, &base, &base);
        let result = run_command(&[bin.display().to_string()], None, &env).unwrap();
        assert!(result.ok());
        assert_eq!(
            result.stdout,
            format!("{}|{}/lib:/existing/lib", root.display(), root.display())
        );
        let _ = std::fs::remove_dir_all(base);
    }
}
