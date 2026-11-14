//! Validación pura de la TOML de modelo conforme a §4.1.1.
//!
//! Este módulo no lee ficheros: recibe el modelo ya parseado y el contexto
//! necesario (ruta, texto fuente y nombres vistos) desde la capa de E/S.

use std::collections::BTreeSet;
use std::path::Path;

use crate::ModelFormat;
use crate::model::ModelSpec;

/// Identificador estable de cada regla de §4.1.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationRule {
    NameKebabCase,
    NameUnique,
    NameMatchesFile,
    EngineInvalid,
    SafetensorsRequiresPath,
    NonSafetensorsForbidsPath,
    GgufRequiresFile,
    DownloadRequiresRepo,
    DownloadAndBuild,
    UndeclaredFilePlaceholder,
    LevelDefaultMissing,
    AliasLevelMissing,
    OptionDefaultMissing,
    FlagRequiresMissingFile,
    MinRamMustBePositive,
    UnknownKey,
    ContextInvalid,
    /// El TOML no se puede deserializar (error de sintaxis o de tipo).
    ///
    /// No es una regla de §4.1.1: es el fallo que impide siquiera leer el
    /// fichero. Se distingue de `UnknownKey` para que la regla que se
    /// reporta sea la que de verdad se rompió.
    ParseInvalid,
}

impl ValidationRule {
    pub fn exit_code(self) -> i32 {
        match self {
            Self::DownloadAndBuild => 2,
            _ => 1,
        }
    }
}

/// Contexto externo de una validación.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    pub source: String,
    pub source_text: Option<String>,
    pub seen_names: BTreeSet<String>,
}

/// Fuente de un modelo para validar un catálogo completo.
///
/// `texts` asocia la ruta de origen con el TOML leído. La función es pura:
/// no lee ficheros y devuelve los errores en orden de catálogo.
pub fn validate_catalog(
    catalog: &crate::Catalog,
    texts: &std::collections::BTreeMap<std::path::PathBuf, String>,
) -> Vec<ValidationError> {
    let mut seen = BTreeSet::new();
    let mut errors = Vec::new();
    for (spec, _) in crate::catalog_rows(catalog, true) {
        let context = ValidationContext {
            source: spec.source.display().to_string(),
            source_text: texts.get(&spec.source).cloned(),
            seen_names: seen.clone(),
        };
        errors.extend(validate(spec, &context));
        seen.insert(spec.model.name.clone());
    }
    errors
}

/// Violación concreta de una regla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    rule: ValidationRule,
    key: String,
    path: String,
    message: String,
    line: Option<usize>,
}

impl ValidationError {
    pub fn rule(&self) -> ValidationRule {
        self.rule
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn line(&self) -> Option<usize> {
        self.line
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn exit_code(&self) -> i32 {
        self.rule.exit_code()
    }
}

/// Clasifica un error de parseo como violación de §4.1.1.
///
/// Un TOML con una clave que el esquema no conoce (`deny_unknown_fields`) o
/// con un motor que no existe no llega a `validate()`: muere al deserializar.
/// Por eso sus reglas —`UnknownKey` y `EngineInvalid`— se construyen aquí,
/// a partir del error de parseo, y no dentro de [`validate`].
///
/// El error conserva la **ruta y la línea** (doc B §4.1.1): la línea la
/// calculó el parseo (el `span` del error de `toml` sobre el texto fuente).
pub fn parse_error(error: &crate::ModelError) -> ValidationError {
    match error {
        crate::ModelError::Parse { path, source, line } => {
            let Some(key) = unknown_key_name(source) else {
                return ValidationError {
                    rule: ValidationRule::ParseInvalid,
                    key: String::new(),
                    path: path.display().to_string(),
                    message: format!("no se pudo leer el TOML: {}", source.message()),
                    line: *line,
                };
            };
            ValidationError {
                rule: ValidationRule::UnknownKey,
                key: key.clone(),
                path: path.display().to_string(),
                message: format!(
                    "clave desconocida '{key}': quítala o renómbrala a una que exista en el esquema"
                ),
                line: *line,
            }
        }
        crate::ModelError::UnknownEngine { path, source, line } => ValidationError {
            rule: ValidationRule::EngineInvalid,
            key: "model.engine".to_string(),
            path: path.display().to_string(),
            message: source.to_string(),
            line: *line,
        },
    }
}

/// El nombre de la clave desconocida, si el error es de ese tipo.
///
/// `toml` lo dice como `unknown field \`clave\`, expected \`…\``. Si el error
/// es otro (sintaxis, tipo), no hay clave: `None`.
fn unknown_key_name(err: &toml::de::Error) -> Option<String> {
    let msg = err.message();
    let rest = msg.strip_prefix("unknown field `")?;
    let (name, _) = rest.split_once('`')?;
    Some(name.to_string())
}

/// Valida las reglas semánticas de §4.1.1.
///
/// La clave desconocida y el motor inválido se detectan al deserializar; sus
/// variantes de regla se construyen en [`parse_error`], que clasifica el
/// error de parseo.
pub fn validate(spec: &ModelSpec, ctx: &ValidationContext) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let name = &spec.model.name;

    if !is_kebab_case(name) {
        errors.push(error(
            ValidationRule::NameKebabCase,
            "model.name",
            "[model].name debe estar en kebab-case; usa sólo minúsculas, números, guiones y guiones bajos",
            ctx,
            "model.name",
        ));
    }
    if ctx.seen_names.contains(name) {
        errors.push(error(
            ValidationRule::NameUnique,
            "model.name",
            "[model].name debe ser único global; cambia el nombre o elimina el duplicado",
            ctx,
            "model.name",
        ));
    }
    if file_stem(ctx.source.as_str()) != Some(name.as_str()) {
        errors.push(error(
            ValidationRule::NameMatchesFile,
            "model.name",
            "[model].name debe coincidir con el nombre del fichero; renombra el fichero o el modelo",
            ctx,
            "model.name",
        ));
    }

    match spec.model.format {
        ModelFormat::Safetensors => {
            if spec.model.path.is_empty() {
                errors.push(error(
                    ValidationRule::SafetensorsRequiresPath,
                    "model.path",
                    "[model].format = \"safetensors\" requiere [model].path",
                    ctx,
                    "model.path",
                ));
            }
        }
        _ if spec.model.path.is_empty() => {}
        _ if matches!(spec.model.format, ModelFormat::Q4nx | ModelFormat::Strata) => {}
        _ => {
            errors.push(error(
                ValidationRule::NonSafetensorsForbidsPath,
                "model.path",
                "[model].path sólo se permite con [model].format = \"safetensors\", \"q4nx\" o \"strata\"; quita el campo",
                ctx,
                "model.path",
            ));
        }
    }
    if spec.model.format == ModelFormat::Gguf && spec.files.gguf.is_none() {
        errors.push(error(
            ValidationRule::GgufRequiresFile,
            "files.gguf",
            "[model].format = \"gguf\" requiere [files].gguf",
            ctx,
            "files.gguf",
        ));
    }

    if let Some(download) = &spec.download
        && spec.build.is_none()
    {
        if download.repo.trim().is_empty() {
            errors.push(error(
                ValidationRule::DownloadRequiresRepo,
                "download.repo",
                "[download] requiere repo; declara el repositorio de origen",
                ctx,
                "download.repo",
            ));
        }
        if spec.model.format == ModelFormat::Gguf && download.file.trim().is_empty() {
            errors.push(error(
                ValidationRule::DownloadRequiresRepo,
                "download.file",
                "[download] requiere file cuando el formato es gguf; declara el fichero",
                ctx,
                "download.file",
            ));
        }
    }
    // La tabla [download] puede coexistir con [build] para declarar auxiliares;
    // sólo es un error si ambos describen el artefacto principal. La regla de
    // la especificación exige elegir una vía de instalación.
    if spec.download.is_some()
        && spec.build.is_some()
        && spec.download.as_ref().is_some_and(|download| {
            !download.repo.trim().is_empty()
                || !download.file.trim().is_empty()
                || !download.name.trim().is_empty()
        })
        && spec.build.as_ref().is_some_and(|build| {
            !build.repo.trim().is_empty()
                || !build.gguf_file.trim().is_empty()
                || !build.out.trim().is_empty()
        })
    {
        errors.push(error(
            ValidationRule::DownloadAndBuild,
            "download.build",
            "[download] y [build] no pueden coexistir; conserva sólo uno de los dos métodos",
            ctx,
            "download.build",
        ));
    }

    check_placeholders(spec, ctx, &mut errors);
    if let Some(default) = &spec.levels.default
        && !spec.levels.list.iter().any(|level| level == default)
    {
        errors.push(error(
            ValidationRule::LevelDefaultMissing,
            "levels.default",
            "[levels].default debe estar en [levels].list; añade el nivel o cambia el valor por defecto",
            ctx,
            "levels.default",
        ));
    }
    for (alias, level) in &spec.aliases {
        if !spec.levels.list.iter().any(|candidate| candidate == level) {
            errors.push(error(
                ValidationRule::AliasLevelMissing,
                &format!("aliases.{alias}"),
                &format!(
                    "[aliases].{alias} apunta a un nivel inexistente; usa un nivel de [levels].list"
                ),
                ctx,
                &format!("aliases.{alias}"),
            ));
        }
    }
    if let Some(default) = &spec.options.default
        && !spec.options.list.iter().any(|option| option == default)
    {
        errors.push(error(
            ValidationRule::OptionDefaultMissing,
            "options.default",
            "[options.default] debe tener su clave en [options].list; añade la opción o cambia el valor por defecto",
            ctx,
            "options.default",
        ));
    }
    for (flag, args) in &spec.args.flag {
        for required in &args.requires {
            if !has_file(spec, required) {
                errors.push(error(
                    ValidationRule::FlagRequiresMissingFile,
                    &format!("args.flag.{flag}.requires"),
                    &format!("[args.flag.{flag}].requires referencia una clave inexistente de [files]; quita la referencia o declara el fichero"),
                    ctx,
                    &format!("args.flag.{flag}.requires"),
                ));
            }
        }
    }
    // La regla es **estrictamente mayor que 0** (doc B §4.1.1 / R-DATA-9),
    // como en la v1 (`ropero-code/bin/python/modelconf.py:1563`, que rechaza
    // `<= 0`). El campo es **opcional**: si no se declara, no se comprueba
    // (la v1 sólo mira la clave cuando está presente, línea 1560).
    if let Some(min_ram_gb) = spec.limits.min_ram_gb
        && min_ram_gb <= 0.0
    {
        errors.push(error(
            ValidationRule::MinRamMustBePositive,
            "limits.min_ram_gb",
            "[limits].min_ram_gb debe ser mayor que 0; indica la RAM mínima o quita el campo",
            ctx,
            "limits.min_ram_gb",
        ));
    }
    if spec.model.context != 0 && !spec.model.context.is_multiple_of(32) {
        errors.push(error(
            ValidationRule::ContextInvalid,
            "model.context",
            "[model].context debe ser múltiplo de 32 cuando está declarado; usa un valor válido",
            ctx,
            "model.context",
        ));
    }
    errors
}

/// Localiza la línea 1-based de una clave TOML en su texto fuente.
pub fn find_toml_line(source: &str, key: &str) -> Option<usize> {
    find_toml_line_in(source, key, &[key])
}

/// La línea 1-based donde cae un error de parseo de `toml`.
///
/// El error trae un `span` (rango de bytes en el texto original); de ahí se
/// cuenta la línea. Es lo que permite que el error de parseo lleve
/// `ruta:línea` (doc B §4.1.1) en vez de sólo la ruta.
///
/// PURA: sólo lee los dos argumentos; no toca el disco.
pub fn parse_error_line(source: &str, err: &toml::de::Error) -> Option<usize> {
    let span = err.span()?;
    let start = span.start.min(source.len());
    Some(source[..start].matches('\n').count() + 1)
}

/// Localiza la línea 1-based de la clave `[model].engine`.
///
/// La clave vive dentro de la tabla `[model]`: el nombre corto (`engine`) no
/// basta, porque la misma clave puede existir en otra tabla.
pub fn find_engine_line(source: &str) -> Option<usize> {
    find_toml_line_in(source, "model.engine", &["model"])
}

fn find_toml_line_in(source: &str, key: &str, alternatives: &[&str]) -> Option<usize> {
    if key == "download.build" {
        return find_toml_heading(source, "build")
            .or_else(|| find_toml_heading(source, "download"));
    }
    let leaf = key.rsplit('.').next().unwrap_or(key);
    let mut current_table = String::new();
    source.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            return None;
        }
        let heading = trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'));
        if let Some(heading) = heading {
            current_table = heading.to_string();
            return (heading == key).then_some(index + 1);
        }
        let candidate = trimmed.split_once('=').map(|(left, _)| left.trim());
        let expected_table = key.rsplit_once('.').map(|(table, _)| table).unwrap_or("");
        if candidate == Some(leaf)
            && (current_table.is_empty()
                || current_table == expected_table
                || alternatives
                    .iter()
                    .any(|alternative| *alternative == current_table))
        {
            Some(index + 1)
        } else {
            None
        }
    })
}

fn error(
    rule: ValidationRule,
    key: &str,
    message: &str,
    ctx: &ValidationContext,
    source_key: &str,
) -> ValidationError {
    ValidationError {
        rule,
        key: key.to_string(),
        path: ctx.source.clone(),
        message: message.to_string(),
        line: ctx.source_text.as_deref().and_then(|text| {
            let alternatives: &[&str] = match source_key {
                "download.build" => &["download", "build"],
                key if key.starts_with("aliases.") => &["aliases"],
                key if key.starts_with("args.") => &[key, "args"],
                "model.path" => &["model"],
                "model.name" => &["model"],
                "model.context" => &["model"],
                "files.gguf" => &["files"],
                "download.repo" => &["download"],
                "download.file" => &["download"],
                "levels.default" => &["levels"],
                "options.default" => &["options"],
                "limits.min_ram_gb" => &["limits"],
                _ => &[],
            };
            find_toml_line_in(text, source_key, alternatives)
        }),
    }
}

fn find_toml_heading(source: &str, heading: &str) -> Option<usize> {
    source.lines().enumerate().find_map(|(index, line)| {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            return None;
        }
        (trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
            == Some(heading))
        .then_some(index + 1)
    })
}

fn is_kebab_case(name: &str) -> bool {
    !name.is_empty()
        && name.split('-').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
        })
}

fn file_stem(source: &str) -> Option<&str> {
    Path::new(source).file_stem().and_then(|stem| stem.to_str())
}

fn has_file(spec: &ModelSpec, key: &str) -> bool {
    match key {
        "gguf" => spec.files.gguf.is_some(),
        "mtp" => spec.files.mtp.is_some(),
        "mmproj" => spec.files.mmproj.is_some(),
        "chat_template" => spec.files.chat_template.is_some(),
        _ => false,
    }
}

fn check_placeholders(
    spec: &ModelSpec,
    ctx: &ValidationContext,
    errors: &mut Vec<ValidationError>,
) {
    let groups = [
        ("args.base", spec.args.base.iter().collect::<Vec<_>>()),
        (
            "args.level",
            spec.args
                .level
                .values()
                .flat_map(|level| level.base.iter())
                .collect::<Vec<_>>(),
        ),
        (
            "args.flag",
            spec.args
                .flag
                .values()
                .flat_map(|flag| flag.base.iter())
                .collect::<Vec<_>>(),
        ),
        (
            "args.option",
            spec.args
                .option
                .values()
                .flat_map(|option| option.base.iter())
                .collect::<Vec<_>>(),
        ),
    ];
    for (key, values) in groups {
        for value in values {
            let Some(placeholder) = file_placeholder(value) else {
                continue;
            };
            if !has_file(spec, placeholder) {
                errors.push(error(
                    ValidationRule::UndeclaredFilePlaceholder,
                    key,
                    &format!(
                        "${{{placeholder}}} no está declarado en [files]; declara el fichero o quita el argumento"
                    ),
                    ctx,
                    key,
                ));
            }
        }
    }
}

fn file_placeholder(value: &str) -> Option<&str> {
    ["gguf", "mtp", "mmproj", "chat_template"]
        .into_iter()
        .find(|name| value.contains(&format!("${{{name}}}")))
}
