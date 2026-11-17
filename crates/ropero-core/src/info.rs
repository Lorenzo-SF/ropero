//! Presentación del detalle de un modelo.

use crate::{Environment, ModelSpec};

/// Construye el detalle completo de un modelo en el idioma configurado.
pub fn lines(spec: &ModelSpec, env: &Environment, lang: &str) -> Vec<String> {
    let english = lang.eq_ignore_ascii_case("EN");
    let mut out = vec![format!(
        "{} {}",
        if english { "MODEL" } else { "MODELO" },
        spec.model.name
    )];
    out.push(format!(
        "  {}: {}",
        if english {
            "engine alias"
        } else {
            "alias engine"
        },
        spec.model.alias.as_deref().unwrap_or(&spec.model.name)
    ));
    out.push(format!("  engine: {}", spec.model.engine.as_str()));
    out.push(format!(
        "  {}: {}",
        if english { "format" } else { "formato" },
        spec.model.format
    ));
    out.push(format!(
        "  {}: {}",
        if english { "artifact" } else { "artefacto" },
        crate::listing::artefacto_de(spec)
    ));
    let paths = crate::resolve_paths(spec, env);
    out.extend(crate::path_lines(&paths));
    if let Some(download) = &spec.download {
        out.push(format!(
            "  download: repo={} file={}",
            download.repo, download.file
        ));
        out.push(format!("    gguf: {}", download.gguf));
        out.push(format!("    manual: {}", download.manual));
        for (name, aux) in &download.aux {
            out.push(format!(
                "    aux {name}: repo={} file={} use={}",
                aux.repo, aux.file, aux.use_
            ));
        }
    }
    if let Some(build) = &spec.build {
        out.push(format!(
            "  build: repo={} gguf={} quant={} imatrix_repo={} imatrix_file={} intermedio={}",
            build.repo,
            build.gguf_file,
            build.quant,
            build.imatrix_repo,
            build.imatrix_file,
            build.intermedio
        ));
    }
    if spec.download.is_none() && spec.build.is_none() {
        out.push(if english {
            "  source: local".to_string()
        } else {
            "  origen: local".to_string()
        });
    }
    out.push(format!(
        "  {}: {}",
        if english { "context" } else { "contexto" },
        spec.model.context
    ));
    out.push(format!(
        "  {}: {}",
        if english { "levels" } else { "niveles" },
        spec.levels.list.join(", ")
    ));
    out.push(format!(
        "  {}: {}",
        if english {
            "default level"
        } else {
            "nivel por defecto"
        },
        spec.levels.default.as_deref().unwrap_or("—")
    ));
    for (alias, level) in &spec.aliases {
        out.push(format!(
            "  {} {alias}: {level}",
            if english {
                "level alias"
            } else {
                "alias de nivel"
            }
        ));
    }
    out.push(format!("  flags: {}", spec.flags.list.join(", ")));
    out.push(format!(
        "  {}: {}",
        if english { "options" } else { "opciones" },
        spec.options.list.join(", ")
    ));
    let defaults = spec
        .options
        .default
        .iter()
        .map(|(option, value)| format!("{option}={value}"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push(format!(
        "  {}: {}",
        if english {
            "default options"
        } else {
            "opciones por defecto"
        },
        if defaults.is_empty() {
            "—"
        } else {
            &defaults
        }
    ));
    push_args(&mut out, "base", &spec.args.base);
    for (name, args) in &spec.args.level {
        push_args(
            &mut out,
            &format!("{} {name}", if english { "level" } else { "nivel" }),
            &args.base,
        );
        if !args.reasoning_effort.is_empty() {
            out.push(format!(
                "  {} {name}: {}",
                if english {
                    "reasoning effort"
                } else {
                    "esfuerzo de razonamiento"
                },
                args.reasoning_effort
            ));
        }
        if !args.chat_template_kwargs.is_empty() {
            out.push(format!(
                "  {} {name}: {:?}",
                if english {
                    "chat template kwargs"
                } else {
                    "argumentos de plantilla de chat"
                },
                args.chat_template_kwargs
            ));
        }
    }
    for (name, args) in &spec.args.flag {
        push_args(&mut out, &format!("flag {name}"), &args.base);
        if !args.requires.is_empty() {
            out.push(format!(
                "  {} {name}: {}",
                if english { "requires" } else { "requiere" },
                args.requires.join(", ")
            ));
        }
    }
    for (name, args) in &spec.args.option {
        push_args(
            &mut out,
            &format!("{} {name}", if english { "option" } else { "opción" }),
            &args.base,
        );
        for (value, tokens) in &args.values {
            push_args(
                &mut out,
                &format!(
                    "{} {name}={value}",
                    if english { "option" } else { "opción" }
                ),
                tokens,
            );
        }
    }
    out
}

fn push_args(out: &mut Vec<String>, section: &str, args: &[String]) {
    if !args.is_empty() {
        out.push(format!("  args {section}: {}", args.join(" ")));
    }
}
