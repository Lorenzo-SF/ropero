//! Comprobación del catálogo real de la v2 (`models/`, en la raíz del
//! workspace).
//!
//! El catálogo de la v2 es **propio**: vive en `<repo-root>/models` (doc B
//! §2.4) y está versionado en el repo. Por eso este test **no** está
//! marcado `#[ignore]`: corre siempre, en la batería por defecto y en CI.
//!
//! La v1 (`ropero-code/`) es sólo referencia: la v2 se desarrolla como si
//! esa carpeta no existiera, y este test no depende de ella.

use std::path::{Path, PathBuf};

/// Nº de modelos activos esperados en el catálogo real.
const ACTIVOS_ESPERADOS: usize = 20;
/// Nº de modelos retirados (`fired/`) esperados en el catálogo real.
const FIRED_ESPERADOS: usize = 27;
/// Nº total de modelos del catálogo real (activos + retirados).
const TOTAL_ESPERADO: usize = ACTIVOS_ESPERADOS + FIRED_ESPERADOS;

/// Base del catálogo real de la v2: `<repo-root>/models`.
fn base_real() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("models")
}

/// Recoge los TOML de un directorio, ordenados (orden explícito: el del
/// sistema de ficheros no se supone estable).
fn tomls(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("toml"))
            .collect(),
        Err(_) => Vec::new(),
    };
    out.sort();
    out
}

/// Recoge los TOML de `<base>/<motor>` y de `<base>/<motor>/fired/`.
fn recoge(base: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut active = Vec::new();
    let mut fired = Vec::new();
    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(base) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => Vec::new(),
    };
    dirs.sort();
    for dir in dirs {
        active.extend(tomls(&dir));
        fired.extend(tomls(&dir.join("fired")));
    }
    (active, fired)
}

#[test]
fn catalogo_real_tiene_47_modelos() {
    let base = base_real();
    assert!(
        base.is_dir(),
        "no existe el catálogo de la v2 en {}",
        base.display()
    );
    let (active, fired) = recoge(&base);
    assert_eq!(
        active.len(),
        ACTIVOS_ESPERADOS,
        "modelos activos en {}",
        base.display()
    );
    assert_eq!(
        fired.len(),
        FIRED_ESPERADOS,
        "modelos retirados en {}",
        base.display()
    );
    assert_eq!(
        active.len() + fired.len(),
        TOTAL_ESPERADO,
        "total de modelos en {}",
        base.display()
    );
}

/// El `_template.toml` del catálogo real debe existir y declarar `[model]`.
#[test]
fn catalogo_real_tiene_plantilla() {
    let base = base_real();
    assert!(
        base.is_dir(),
        "no existe el catálogo de la v2 en {}",
        base.display()
    );
    let template = base.join("_template.toml");
    assert!(template.is_file(), "falta {}", template.display());
    let text = std::fs::read_to_string(&template).expect("leer la plantilla");
    assert!(text.contains("[model]"), "la plantilla no declara [model]");
}

/// El catálogo real usa la nomenclatura canónica de ADR-004: el fork es
/// `turbo-llama` y el oficial es `llama-server`.
///
/// Antes del renombrado, `models/llama/` (el fork) declaraba `llama-server`
/// y `models/llama-official/` (el oficial) declaraba `llama-server-official`.
/// Este test fija la cuenta: 30 TOML del fork y 4 del oficial.
#[test]
fn catalogo_real_usa_la_nomenclatura_de_adr_004() {
    let base = base_real();
    let (active, fired) = recoge(&base);
    let mut por_motor: std::collections::BTreeMap<String, usize> = Default::default();
    for path in active.iter().chain(fired.iter()) {
        let text = std::fs::read_to_string(path).expect("leer el TOML del modelo");
        let spec = ropero_core::parse_model(&text, path)
            .unwrap_or_else(|e| panic!("no se pudo parsear {}: {}", path.display(), e));
        assert!(
            spec.notices.is_empty(),
            "{} avisa de un alias legacy: {:?}",
            path.display(),
            spec.notices
        );
        *por_motor
            .entry(spec.model.engine.as_str().to_string())
            .or_default() += 1;
    }
    assert_eq!(
        por_motor.get("turbo-llama").copied(),
        Some(30),
        "el fork (turbo-llama) tiene 30 TOML: {:?}",
        por_motor
    );
    assert_eq!(
        por_motor.get("llama-server").copied(),
        Some(4),
        "el oficial (llama-server) tiene 4 TOML: {:?}",
        por_motor
    );
    assert!(
        !por_motor.contains_key("llama-server-official"),
        "ya no existe el id 'llama-server-official': {:?}",
        por_motor
    );
}

/// Los directorios del catálogo real son los de ADR-004: `turbo-llama/` y
/// `llama-server/`.
#[test]
fn catalogo_real_tiene_los_directorios_de_adr_004() {
    let base = base_real();
    assert!(
        base.join("turbo-llama").is_dir(),
        "falta models/turbo-llama/"
    );
    assert!(
        base.join("llama-server").is_dir(),
        "falta models/llama-server/"
    );
    assert!(
        !base.join("llama").exists(),
        "models/llama/ ya no existe (es turbo-llama/)"
    );
    assert!(
        !base.join("llama-official").exists(),
        "models/llama-official/ ya no existe (es llama-server/)"
    );
}

/// Todos los TOML del catálogo de la v2 pasan el validador.
///
/// Es el mismo contrato que `ropero validate` sobre `models/`: 45 modelos
/// válidos, sin violaciones.
#[test]
fn catalogo_real_valida_sin_violaciones() {
    let base = base_real();
    assert!(
        base.is_dir(),
        "no existe el catálogo de la v2 en {}",
        base.display()
    );
    let (active, fired) = recoge(&base);
    let mut specs = Vec::new();
    let mut texts = std::collections::BTreeMap::new();
    for path in active.iter().chain(fired.iter()) {
        let text = std::fs::read_to_string(path).expect("leer el TOML del modelo");
        let spec = ropero_core::parse_model(&text, path)
            .unwrap_or_else(|e| panic!("no se pudo parsear {}: {}", path.display(), e));
        specs.push(spec);
        texts.insert(path.clone(), text);
    }
    assert_eq!(
        specs.len(),
        TOTAL_ESPERADO,
        "modelos del catálogo de la v2 en {}",
        base.display()
    );
    let catalog = ropero_core::Catalog {
        models: specs.iter().take(ACTIVOS_ESPERADOS).cloned().collect(),
        fired: specs.iter().skip(ACTIVOS_ESPERADOS).cloned().collect(),
    };
    let errors = ropero_core::validate_catalog(&catalog, &texts);
    assert!(
        errors.is_empty(),
        "el catálogo de la v2 tiene {} violaciones: {:?}",
        errors.len(),
        errors
            .iter()
            .map(|e| format!("{}:{} {}", e.path(), e.line().unwrap_or(0), e.message()))
            .collect::<Vec<_>>()
    );
}
