//! Tests de la configuración: parse, precedencia/origen y serialización.

use ropero_core::Environment;
use ropero_core::config::{
    self, Origen, ejemplo_config, escapa_toml, parse_config, resuelve_config,
};
use std::collections::BTreeMap;

fn env(home: &str, cwd: &str, vars: &[(&str, &str)]) -> Environment {
    let mapa: BTreeMap<String, String> = vars
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    Environment::new(mapa, home, cwd)
}

#[test]
fn parse_config_roundtrip_rutas_especiales() {
    let base = std::env::temp_dir().join("ropero-config-roundtrip");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let home = base.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let ruta = base.join("con \"comillas\" y \\ barra y ñ ü 🚀");
    let e = env(home.to_str().unwrap(), base.to_str().unwrap(), &[]);
    let mut cfg = config::config_defectos(&e.vars, &e.home, &e.cwd);
    cfg.models_dir.valor = ruta.clone();
    cfg.gguf_dir.valor = ruta.join("gguf");
    let texto = ejemplo_config(&cfg);
    let parseado = parse_config(&texto, &e).expect("debe parsear ejemplo generado");
    assert_eq!(parseado.models_dir.valor, ruta);
    assert_eq!(parseado.gguf_dir.valor, ruta.join("gguf"));
    assert_eq!(parseado.models_dir.origen, Origen::Archivo);
    assert_eq!(parseado.engines.len(), 9);
    assert!(texto.contains("[models.paths]"));
    assert!(!texto.contains("models_toml_dir ="));
    assert_eq!(texto.matches("gguf_dir =").count(), 1);
    for id in [
        "llama-server",
        "turbo-llama",
        "fastflowllm",
        "mlx-serve",
        "tensorrt-llm",
        "vllm",
        "airllm",
        "strata",
        "ignis",
    ] {
        assert!(
            texto.contains(&format!("[engines.{id}]")),
            "falta tabla de {id}"
        );
    }
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn escapa_toml_casos_basicos() {
    assert_eq!(escapa_toml("a\"b\\c"), "a\\\"b\\\\c");
    assert_eq!(escapa_toml("línea\nuno\tdos"), "línea\\nuno\\tdos");
    assert_eq!(escapa_toml("ñ-漢字-🚀"), "ñ-漢字-🚀");
}

#[test]
fn precedencia_env_sobre_archivo_y_defecto() {
    let base = std::env::temp_dir().join("ropero-config-precedencia");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let home = base.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let archivo_models = base.join("desde-archivo");
    let env_models = base.join("desde-env");
    let toml = format!(
        "models_dir = \"{}\"\ngguf_dir = \"{}\"\n",
        archivo_models.display(),
        base.join("gguf-archivo").display()
    );
    let e = env(
        home.to_str().unwrap(),
        base.to_str().unwrap(),
        &[("ROPERO_MODELS_DIR", env_models.to_str().unwrap())],
    );
    let cfg = resuelve_config(&e, Some(&toml)).expect("config válida");
    assert_eq!(cfg.models_dir.valor, env_models);
    assert_eq!(
        cfg.models_dir.origen,
        Origen::Variable("ROPERO_MODELS_DIR".into())
    );
    assert_eq!(cfg.gguf_dir.valor, base.join("gguf-archivo"));
    assert_eq!(cfg.gguf_dir.origen, Origen::Archivo);
    assert_eq!(cfg.logs_dir.origen, Origen::Defecto);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn parse_config_rechaza_campo_desconocido_y_tipo_invalido() {
    let e = env("/tmp/home-cfg-parse", "/tmp", &[]);
    let err = parse_config("no_existe = \"x\"\n", &e).unwrap_err();
    assert!(err.to_string().contains("campo desconocido"), "{err}");
    let err = parse_config("models_dir = 42\n", &e).unwrap_err();
    assert!(err.to_string().contains("no es una ruta"), "{err}");
}

#[test]
fn config_por_engine_resuelve_rutas_y_preserva_precedencia_global_env() {
    let base = std::env::temp_dir().join("ropero-config-engine-paths");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let home = base.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let text = format!(
        "models_dir = \"{}\"\ngguf_dir = \"{}\"\n\n[engines.llama-server]\nmodels_toml_dir = \"models/custom\"\ngguf_dir = \"~/custom-gguf\"\nsafetensors_dir = \"~/custom-safe\"\nport_min = 8110\nport_max = 8120\ncolor = \"morado\"\n",
        base.join("catalog").display(),
        base.join("gguf-global").display()
    );
    let e = env(home.to_str().unwrap(), base.to_str().unwrap(), &[]);
    let cfg = resuelve_config(&e, Some(&text)).unwrap();
    let id = ropero_core::EngineId::LlamaServer;
    assert_eq!(cfg.models_toml_dir(id).valor, base.join("models/custom"));
    assert_eq!(cfg.gguf_dir_de(id).valor, home.join("custom-gguf"));
    assert_eq!(cfg.safetensors_dir_de(id).valor, home.join("custom-safe"));
    assert_eq!(cfg.engines[&id].port_min, Some(8110));
    assert_eq!(cfg.engines[&id].color.unwrap().as_str(), "morado");

    let e = env(
        home.to_str().unwrap(),
        base.to_str().unwrap(),
        &[("GGUF_DIR", "/global/from-env")],
    );
    let cfg = resuelve_config(&e, Some(&text)).unwrap();
    assert_eq!(
        e.with_config(cfg)
            .engine_artifact_path(id, "GGUF_DIR", "models/gguf"),
        std::path::PathBuf::from("/global/from-env")
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn models_paths_define_las_rutas_globales_y_legacy_por_engine_no_las_duplica() {
    let base = std::env::temp_dir().join("ropero-model-paths-globales");
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    let home = base.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let text = format!(
        "[models.paths]\nmodels_dir = \"{}\"\ngguf_dir = \"{}\"\nsafetensors_dir = \"{}\"\nartifacts_dir = \"{}\"\nbin_dir = \"{}\"\nlogs_dir = \"{}\"\n\n[engines.ignis]\nport_min = 9911\n",
        base.join("catalog").display(),
        base.join("weights").display(),
        base.join("checkpoints").display(),
        base.join("artifacts").display(),
        base.join("bin").display(),
        base.join("logs").display(),
    );
    let e = env(home.to_str().unwrap(), base.to_str().unwrap(), &[]);
    let cfg = resuelve_config(&e, Some(&text)).unwrap();
    assert_eq!(cfg.models_dir.valor, base.join("catalog"));
    assert_eq!(
        cfg.gguf_dir_de(ropero_core::EngineId::Ignis).valor,
        base.join("weights")
    );
    assert_eq!(
        cfg.safetensors_dir_de(ropero_core::EngineId::Ignis).valor,
        base.join("checkpoints")
    );
    assert_eq!(
        cfg.artifacts_dir_de(ropero_core::EngineId::Ignis).valor,
        base.join("artifacts")
    );
    assert_eq!(cfg.bin_dir.valor, base.join("bin"));
    assert_eq!(cfg.logs_dir.valor, base.join("logs"));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn config_por_engine_rechaza_id_campos_colores_y_puertos_invalidos() {
    let e = env("/tmp/home-cfg-engine-invalid", "/tmp", &[]);
    for text in [
        "[engines.nope]\ncolor = \"rojo\"\n",
        "[engines.llama-server]\nrepo = \"owner/repo\"\n",
        "[engines.llama-server]\ncolor = \"\\u001b[31m\"\n",
        "[engines.llama-server]\nport_min = 0\n",
        "[engines.llama-server]\nport_min = 8200\nport_max = 8100\n",
    ] {
        assert!(parse_config(text, &e).is_err(), "aceptó: {text}");
    }
}

#[test]
fn overlay_config_aplica_color_puertos_y_rechaza_solapes() {
    let e = env("/tmp/home-cfg-engine-overlay", "/tmp", &[]);
    let cfg = parse_config(
        "[engines.llama-server]\nport_min = 9990\nport_max = 9999\ncolor = \"morado\"\n",
        &e,
    )
    .unwrap();
    let source = include_str!("../../../engines/llama-server.toml");
    let mut engine = vec![
        ropero_core::parse_engine_manifest(
            source,
            std::path::Path::new("engines/llama-server.toml"),
        )
        .unwrap(),
    ];
    cfg.overlay_engines(&mut engine).unwrap();
    assert_eq!(engine[0].color.as_str(), "morado");
    assert_eq!(engine[0].port_range.min, 9990);
    assert_eq!(engine[0].port_range.max, 9999);

    let text = include_str!("../../../engines/turbo-llama.toml");
    let other =
        ropero_core::parse_engine_manifest(text, std::path::Path::new("engines/turbo-llama.toml"))
            .unwrap();
    engine.push(other);
    let err = cfg.overlay_engines(&mut engine).unwrap_err();
    assert!(err.to_string().contains("comparten"), "{err}");
    assert_eq!(engine[0].port_range.min, 9990, "falló de forma atómica");
}
