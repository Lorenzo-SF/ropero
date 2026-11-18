use std::collections::BTreeMap;
use std::fs;

use ropero_core::{EngineId, Environment};

#[test]
fn escanea_tomls_desde_ruta_por_engine_y_aplica_overlay() {
    let root = std::env::temp_dir().join("ropero-scan-engine-config");
    let _ = fs::remove_dir_all(&root);
    let models = root.join("models");
    let custom = root.join("custom/llama");
    fs::create_dir_all(models.join("llama-server")).unwrap();
    fs::create_dir_all(&custom).unwrap();
    fs::create_dir_all(root.join("engines")).unwrap();
    fs::write(
        root.join("engines/llama-server.toml"),
        include_str!("../../../engines/llama-server.toml"),
    )
    .unwrap();
    fs::write(
        models.join("llama-server/ignored.toml"),
        include_str!("../../../models/llama-server/groot-o.toml"),
    )
    .unwrap();
    fs::write(
        custom.join("custom.toml"),
        include_str!("../../../models/llama-server/groot-o.toml"),
    )
    .unwrap();

    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let env = Environment::new(BTreeMap::new(), &home, &root);
    let config = ropero_core::config::parse_config(
        &format!(
            "[engines.llama-server]\nmodels_toml_dir = \"{}\"\nport_min = 9100\nport_max = 9199\ncolor = \"morado\"\n",
            custom.display()
        ),
        &env,
    )
    .unwrap();
    let env = env.with_config(config);

    let scan = ropero_runtime::scan(&root, &env).unwrap();
    assert_eq!(
        scan.engines.len(),
        9,
        "los motores soportados vienen integrados, incluido Strata"
    );
    assert_eq!(
        scan.files, 1,
        "el catálogo por defecto debe quedar sustituido"
    );
    assert_eq!(scan.catalog.models.len(), 1);
    let engine = scan
        .engines
        .iter()
        .find(|engine| engine.id == EngineId::LlamaServer)
        .unwrap();
    assert_eq!(engine.color.as_str(), "morado");
    assert_eq!(engine.port_range.min, 9100);
    assert_eq!(engine.port_range.max, 9199);
    assert!(scan.config_error.is_none());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn validate_detecta_un_formato_que_el_engine_no_declara() {
    let root = std::env::temp_dir().join("ropero-scan-engine-format");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("models/llama-server")).unwrap();
    fs::create_dir_all(root.join("engines")).unwrap();
    fs::write(
        root.join("models/llama-server/model.toml"),
        include_str!("../../../models/llama-server/groot-o.toml"),
    )
    .unwrap();
    let incompatible = include_str!("../../../engines/llama-server.toml")
        .replace("formats = [\"gguf\"]", "formats = [\"safetensors\"]");
    fs::write(root.join("engines/llama-server.toml"), incompatible).unwrap();

    let home = root.join("home");
    fs::create_dir_all(&home).unwrap();
    let env = Environment::new(BTreeMap::new(), &home, &root);
    let scan = ropero_runtime::scan(&root, &env).unwrap();
    assert!(
        scan.problems
            .iter()
            .any(|problem| problem.contains("formato 'gguf' que no declara")),
        "problemas={:?}",
        scan.problems
    );
    let _ = fs::remove_dir_all(root);
}
