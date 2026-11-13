use crate::args::{ArgvError, ResolveContext};

pub fn resolve_placeholder(name: &str, ctx: &ResolveContext) -> Result<String, ArgvError> {
    match name {
        "gguf" => ctx
            .files
            .gguf
            .clone()
            .ok_or(ArgvError::MissingFile { key: "gguf".into() }),
        "mtp" => ctx
            .files
            .mtp
            .clone()
            .ok_or(ArgvError::MissingFile { key: "mtp".into() }),
        "mmproj" => ctx.files.mmproj.clone().ok_or(ArgvError::MissingFile {
            key: "mmproj".into(),
        }),
        "chat_template" => ctx
            .files
            .chat_template
            .clone()
            .ok_or(ArgvError::MissingFile {
                key: "chat_template".into(),
            }),
        "ctx" => Ok(ctx.context.to_string()),
        "gguf_dir" => Ok(ctx.files.gguf_dir.clone()),
        "safetensors_dir" => Ok(ctx.files.safetensors_dir.clone()),
        "model_path" => Ok(ctx.files.model_path.clone()),
        _ => {
            if let Some(var) = name.strip_prefix("env:") {
                // soportar env:VAR y env:VAR:-def
                if let Some((v, def)) = var.split_once(":-") {
                    Ok(ctx.env.get(v).cloned().unwrap_or_else(|| def.to_string()))
                } else {
                    ctx.env.get(var).cloned().ok_or(ArgvError::MissingEnv {
                        var: var.to_string(),
                    })
                }
            } else {
                Err(ArgvError::MissingFile {
                    key: name.to_string(),
                })
            }
        }
    }
}
