//! Ayudas para modelos servidos por FastFlowLM (FLM).
//!
//! FLM es dueño de su almacén de tags: ropero sólo ejecuta el tag que declara
//! el modelo y delega las operaciones (`pull`, `remove`, `serve`, `list`).

use crate::ModelSpec;

/// El tag de decisión declarado en `[args].base` (`--decision <tag>`), si lo hay.
pub fn decision_tag(spec: &ModelSpec) -> Option<String> {
    let mut args = spec.args.base.iter();
    while let Some(arg) = args.next() {
        if arg == "--decision" {
            return args.next().filter(|tag| !tag.is_empty()).cloned();
        }
    }
    None
}

/// El tag primario de FLM para un modelo.
///
/// Prioridad: `[model].alias` (tag registrado que FLM expone), luego
/// `[model].path`; en su defecto, el tag de `--decision` en `[args].base`.
pub fn primary_tag(spec: &ModelSpec) -> Option<String> {
    if let Some(alias) = spec
        .model
        .alias
        .as_deref()
        .filter(|alias| !alias.is_empty())
    {
        return Some(alias.to_string());
    }
    if !spec.model.path.is_empty() {
        Some(spec.model.path.clone())
    } else {
        decision_tag(spec)
    }
}

/// Todos los tags FLM relevantes para instalar/consultar el modelo.
///
/// Incluye `[model].path` (si hay) y el tag de `--decision` (si hay y es
/// distinto), en ese orden y sin duplicados.
pub fn tags(spec: &ModelSpec) -> Vec<String> {
    let mut tags = Vec::new();
    if let Some(alias) = spec
        .model
        .alias
        .as_deref()
        .filter(|alias| !alias.is_empty())
    {
        tags.push(alias.to_string());
    } else if !spec.model.path.is_empty() {
        tags.push(spec.model.path.clone());
    }
    if let Some(decision) = decision_tag(spec)
        && !tags.iter().any(|t| t == &decision)
    {
        tags.push(decision);
    }
    tags
}

/// Si el modelo usa el almacén delegado de FLM (formato `q4nx`).
pub fn is_flm_model(spec: &ModelSpec) -> bool {
    spec.model.format == crate::ModelFormat::Q4nx
        || spec.model.engine == crate::EngineId::FastFlowLm
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_model;
    use std::path::Path;

    const D3: &str = r#"
[model]
name    = "d3mini"
engine  = "fastflowllm"
context = 32768
format  = "q4nx"
path    = ""

[files]

[args]
base = ["--decision", "d3:mini", "--prefill-chunk-len", "8192"]
"#;

    const LFM3: &str = r#"
[model]
name    = "lfm3"
engine  = "fastflowllm"
context = 131072
format  = "q4nx"
path    = "lfm3:1.7b"

[files]

[args]
base = ["--decision", "d3:mini"]
"#;

    #[test]
    fn primary_tag_deriva_de_decision_si_no_hay_path() {
        let spec = parse_model(D3, Path::new("d3mini.toml")).expect("parse");
        assert_eq!(primary_tag(&spec).as_deref(), Some("d3:mini"));
        assert_eq!(decision_tag(&spec).as_deref(), Some("d3:mini"));
        assert_eq!(tags(&spec), vec!["d3:mini".to_string()]);
    }

    #[test]
    fn primary_tag_usa_path_y_conserva_decision_auxiliar() {
        let spec = parse_model(LFM3, Path::new("lfm3.toml")).expect("parse");
        assert_eq!(primary_tag(&spec).as_deref(), Some("lfm3:1.7b"));
        assert_eq!(decision_tag(&spec).as_deref(), Some("d3:mini"));
        assert_eq!(
            tags(&spec),
            vec!["lfm3:1.7b".to_string(), "d3:mini".to_string()]
        );
    }

    #[test]
    fn alias_flm_es_el_tag_publico_y_no_el_nombre_interno() {
        let mut spec = parse_model(LFM3, Path::new("lfm3.toml")).expect("parse");
        spec.model.name = "mi-lfm".to_string();
        spec.model.alias = Some("lfm3:1.7b".to_string());
        assert_eq!(primary_tag(&spec).as_deref(), Some("lfm3:1.7b"));
        assert_eq!(
            tags(&spec),
            vec!["lfm3:1.7b".to_string(), "d3:mini".to_string()]
        );
    }
}
