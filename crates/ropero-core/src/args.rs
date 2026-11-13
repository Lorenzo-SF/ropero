use std::collections::BTreeMap;

use crate::model::ModelSpec;
use crate::placeholders::resolve_placeholder;

#[derive(Debug, Clone, PartialEq)]
pub struct LaunchRequest {
    pub level: Option<String>,
    pub flags: Vec<String>,
    pub options: Vec<(String, String)>,
}

/// Petición de lanzamiento interpretada desde los tokens de la línea de
/// órdenes (`--nivel`, `--flag`, `--opcion=valor`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedRequest {
    pub level: Option<String>,
    pub flags: Vec<String>,
    pub options: Vec<(String, String)>,
    /// Nivel anterior, cuando el nivel se repite con otro valor (gana el
    /// último).
    pub repeated_level: Option<String>,
}

/// Interpreta los tokens de la línea de órdenes según el vocabulario del
/// modelo: un token que es un nivel declarado es un nivel, `clave=valor` es
/// una opción y el resto son flags.
pub fn parse_request(spec: &ModelSpec, tokens: &[String]) -> ParsedRequest {
    let mut req = ParsedRequest::default();
    for tok in tokens {
        let t = tok.trim_start_matches("--");
        if let Some((key, value)) = t.split_once('=') {
            req.options.push((key.to_string(), value.to_string()));
        } else if spec.levels.list.iter().any(|l| l == t) {
            if req.level.as_deref().is_some_and(|l| l != t) {
                req.repeated_level = req.level.clone();
            }
            req.level = Some(t.to_string());
        } else {
            req.flags.push(t.to_string());
        }
    }
    req
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolveContext {
    pub context: u32,
    pub files: ResolvedFiles,
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFiles {
    pub gguf: Option<String>,
    pub mtp: Option<String>,
    pub mmproj: Option<String>,
    pub chat_template: Option<String>,
    pub gguf_dir: String,
    pub safetensors_dir: String,
    pub model_path: String,
}

/// Resuelve el contexto de lanzamiento de un modelo.
///
/// Traduce el modelo (nombre real, formato, ficheros declarados) y el
/// entorno (`GGUF_DIR`, `SAFETENSORS_DIR`, `ROPERO_<MODELO>_GGUF`,
/// `ROPERO_<MODELO>_CTX`) a las rutas ya expandidas que usa [`build_argv`].
pub fn resolve_context(spec: &ModelSpec) -> ResolveContext {
    use crate::ModelFormat;

    let real = crate::catalog::real_name(spec);
    let gguf_dir = crate::catalog::env_path("GGUF_DIR", "models/gguf");
    let safetensors_dir = crate::catalog::env_path("SAFETENSORS_DIR", "models/safetensors");
    let gguf_name = std::env::var(format!("ROPERO_{}_GGUF", crate::catalog::slug(&real)))
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            spec.files
                .gguf
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
        });
    let gguf = gguf_name.map(|n| gguf_dir.join(&real).join(n).to_string_lossy().to_string());
    let mtp = spec
        .files
        .mtp
        .as_ref()
        .map(|p| gguf_dir.join(&real).join(p).to_string_lossy().to_string());
    let mmproj = spec
        .files
        .mmproj
        .as_ref()
        .map(|p| gguf_dir.join(&real).join(p).to_string_lossy().to_string());
    let chat_template = spec
        .files
        .chat_template
        .as_ref()
        .map(|p| gguf_dir.join(&real).join(p).to_string_lossy().to_string());
    let model_path = if spec.model.format == ModelFormat::Safetensors && !spec.model.path.is_empty()
    {
        let p = std::path::PathBuf::from(&spec.model.path);
        if p.is_absolute() {
            p.to_string_lossy().to_string()
        } else {
            safetensors_dir
                .join(&spec.model.path)
                .to_string_lossy()
                .to_string()
        }
    } else {
        spec.model.path.clone()
    };
    ResolveContext {
        context: crate::catalog::effective_context(spec),
        files: ResolvedFiles {
            gguf,
            mtp,
            mmproj,
            chat_template,
            gguf_dir: gguf_dir.to_string_lossy().to_string(),
            safetensors_dir: safetensors_dir.to_string_lossy().to_string(),
            model_path,
        },
        env: std::env::vars().collect(),
    }
}

#[derive(Debug)]
pub enum ArgvError {
    UnknownLevel {
        level: String,
        declared: Vec<String>,
    },
    UnknownFlag {
        flag: String,
        declared: Vec<String>,
    },
    UnknownOption {
        option: String,
        declared: Vec<String>,
    },
    MissingFile {
        key: String,
    },
    MissingEnv {
        var: String,
    },
}

fn file_available(ctx: &ResolveContext, key: &str) -> bool {
    match key {
        "gguf" => ctx.files.gguf.is_some(),
        "mtp" => ctx.files.mtp.is_some(),
        "mmproj" => ctx.files.mmproj.is_some(),
        "chat_template" => ctx.files.chat_template.is_some(),
        _ => true,
    }
}

fn slug_upper(s: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.push(c.to_ascii_uppercase());
            prev_underscore = false;
        } else if !prev_underscore && !out.is_empty() {
            out.push('_');
            prev_underscore = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn expand(text: &str, ctx: &ResolveContext, value: Option<&str>) -> Result<String, ArgvError> {
    if !text.contains("${") {
        return Ok(text.to_string());
    }
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out = String::new();
    let mut i = 0usize;
    while i < n {
        if chars[i] == '$' && i + 1 < n && chars[i + 1] == '{' {
            let mut j = i + 2;
            while j < n && chars[j] != '}' {
                j += 1;
            }
            if j < n {
                let name: String = chars[i + 2..j].iter().collect();
                let resolved = if name == "value" {
                    value.unwrap_or("").to_string()
                } else {
                    resolve_placeholder(&name, ctx)?
                };
                out.push_str(&resolved);
                i = j + 1;
            } else {
                out.push(chars[i]);
                i += 1;
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    Ok(out)
}

fn escape_json(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn toml_value_to_json(v: &toml::Value, ctx: &ResolveContext) -> Result<String, ArgvError> {
    match v {
        toml::Value::String(s) => {
            let exp = expand(s, ctx, None)?;
            Ok(format!("\"{}\"", escape_json(&exp)))
        }
        toml::Value::Integer(i) => Ok(i.to_string()),
        toml::Value::Float(f) => Ok(f.to_string()),
        toml::Value::Boolean(b) => Ok(b.to_string()),
        toml::Value::Array(arr) => {
            let mut parts = Vec::new();
            for item in arr {
                parts.push(toml_value_to_json(item, ctx)?);
            }
            Ok(format!("[{}]", parts.join(",")))
        }
        toml::Value::Table(t) => {
            let mut parts = Vec::new();
            for (k, val) in t {
                parts.push(format!(
                    "\"{}\":{}",
                    escape_json(k),
                    toml_value_to_json(val, ctx)?
                ));
            }
            Ok(format!("{{{}}}", parts.join(",")))
        }
        toml::Value::Datetime(d) => Ok(format!("\"{}\"", escape_json(&d.to_string()))),
    }
}

fn chat_template_kwargs_json(
    cfg: &crate::model::Level,
    ctx: &ResolveContext,
) -> Result<String, ArgvError> {
    // cfg.chat_template_kwargs is BTreeMap<String, toml::Value>
    let mut parts = Vec::new();
    for (k, v) in &cfg.chat_template_kwargs {
        parts.push(format!(
            "\"{}\":{}",
            escape_json(k),
            toml_value_to_json(v, ctx)?
        ));
    }
    Ok(format!("{{{}}}", parts.join(",")))
}

pub fn build_argv(
    spec: &ModelSpec,
    request: &LaunchRequest,
    ctx: &ResolveContext,
) -> Result<Vec<String>, ArgvError> {
    // Declared levels
    let levels_list = &spec.levels.list;

    // Resolve effective level
    let mut level = request.level.clone();
    if level.is_none()
        && !levels_list.is_empty()
        && let Some(def) = &spec.levels.default
        && levels_list.contains(def)
    {
        level = Some(def.clone());
    }
    if let Some(lv) = &level
        && (levels_list.is_empty() || !levels_list.contains(lv))
    {
        return Err(ArgvError::UnknownLevel {
            level: lv.clone(),
            declared: levels_list.clone(),
        });
    }

    // Declared flags (combined from metadata and args)
    let mut declared_flags: Vec<String> = spec.args.flag.keys().cloned().collect();
    for k in &spec.flags.list {
        if !declared_flags.contains(k) {
            declared_flags.push(k.clone());
        }
    }
    declared_flags.sort();

    // Declared options (combined)
    let mut declared_options: Vec<String> = spec.args.option.keys().cloned().collect();
    for k in &spec.options.list {
        if !declared_options.contains(k) {
            declared_options.push(k.clone());
        }
    }
    declared_options.sort();

    // Validate flags
    for name in &request.flags {
        let known = spec.args.flag.contains_key(name) || spec.flags.list.contains(name);
        if !known {
            return Err(ArgvError::UnknownFlag {
                flag: name.clone(),
                declared: declared_flags.clone(),
            });
        }
        if let Some(cfg) = spec.args.flag.get(name) {
            for req in &cfg.requires {
                if !file_available(ctx, req) {
                    return Err(ArgvError::MissingFile { key: req.clone() });
                }
            }
        }
    }

    // Validate options
    for (key, _) in &request.options {
        let known = spec.args.option.contains_key(key) || spec.options.list.contains(key);
        if !known {
            return Err(ArgvError::UnknownOption {
                option: key.clone(),
                declared: declared_options.clone(),
            });
        }
    }

    let mut argv: Vec<String> = Vec::new();

    // 1. [args].base
    for item in &spec.args.base {
        argv.push(expand(item, ctx, None)?);
    }

    // 2. [args.level.<nivel>]
    if let Some(lv) = &level
        && let Some(cfg) = spec.args.level.get(lv)
    {
        for item in &cfg.base {
            argv.push(expand(item, ctx, None)?);
        }
        if !cfg.chat_template_kwargs.is_empty() {
            argv.push("--chat-template-kwargs".to_string());
            argv.push(chat_template_kwargs_json(cfg, ctx)?);
        } else if !cfg.reasoning_effort.is_empty() {
            let effort = expand(&cfg.reasoning_effort, ctx, None)?;
            argv.push("--chat-template-kwargs".to_string());
            argv.push(format!(
                "{{\"reasoning_effort\":\"{}\"}}",
                escape_json(&effort)
            ));
        }
    }

    // 3. [args.flag.<flag>] por cada flag pedido
    for name in &request.flags {
        if let Some(cfg) = spec.args.flag.get(name) {
            for item in &cfg.base {
                argv.push(expand(item, ctx, None)?);
            }
        }
    }

    // 4. [args.option.<opción>]
    // Resolve which options are active: explicit request + env defaults.
    let mut resolved: Vec<(String, String)> = Vec::new();
    for (k, v) in &request.options {
        resolved.push((k.clone(), v.clone()));
    }
    for key in spec.args.option.keys() {
        if resolved.iter().any(|(k, _)| k == key) {
            continue;
        }
        let slug = slug_upper(key);
        let env_key = format!("ROPERO_MODEL_OPT_{}", slug);
        if let Some(val) = ctx.env.get(&env_key)
            && !val.is_empty()
        {
            resolved.push((key.clone(), val.clone()));
        }
    }

    for key in spec.args.option.keys() {
        let entry = resolved.iter().find(|(k, _)| k == key);
        if let Some((_, value)) = entry {
            let cfg = match spec.args.option.get(key) {
                Some(cfg) => cfg,
                None => continue,
            };
            let mut items = Vec::with_capacity(cfg.base.len());
            for item in &cfg.base {
                items.push(expand(item, ctx, Some(value))?);
            }
            match &cfg.replace {
                // `replace` SUSTITUYE la pareja del base en vez de añadirse
                // detrás (doc B §3.2.3, R-DATA-16): la pareja nueva ocupa el
                // sitio de la vieja, para que el argv no lleve `--temp 0.7 …
                // --temp 0.9` y el motor no gane "por casualidad".
                Some(token) => {
                    let at = argv.iter().position(|a| a == token);
                    match at {
                        Some(index) => {
                            let width = pair_width(&argv, index);
                            argv.splice(index..index + width, items);
                        }
                        // El base no declara ese token: no es error (el
                        // modelo puede no llevar ese flag en su base); la
                        // pareja nueva se añade al final.
                        None => argv.extend(items),
                    }
                }
                None => argv.extend(items),
            }
        }
    }

    // Also handle options declared only in metadata but present in args? already covered.

    Ok(argv)
}

/// Anchura de la pareja que empieza en `index`: el token y su valor.
///
/// El base es una lista plana de tokens, así que no se puede saber si el
/// token lleva valor o no: la regla es la de la v1
/// (`ropero-code/bin/python/modelconf.py:817-826`): se quitan el token y el
/// que le sigue. Si el token es el último, sólo se quita él.
fn pair_width(argv: &[String], index: usize) -> usize {
    if index + 1 < argv.len() { 2 } else { 1 }
}

impl std::fmt::Display for ArgvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgvError::UnknownLevel { level, declared } => {
                write!(
                    f,
                    "nivel no declarado: {}; declarados: {:?}",
                    level, declared
                )
            }
            ArgvError::UnknownFlag { flag, declared } => {
                write!(f, "flag no declarado: {}; declarados: {:?}", flag, declared)
            }
            ArgvError::UnknownOption { option, declared } => {
                write!(
                    f,
                    "opción no declarada: {}; declaradas: {:?}",
                    option, declared
                )
            }
            ArgvError::MissingFile { key } => {
                write!(f, "fichero no disponible: {}", key)
            }
            ArgvError::MissingEnv { var } => {
                write!(f, "variable de entorno no definida: {}", var)
            }
        }
    }
}

impl std::error::Error for ArgvError {}
