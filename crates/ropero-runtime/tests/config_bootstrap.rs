//! Tests de bootstrap de configuración (E/S): idempotencia, overrides y parse.

use std::collections::BTreeMap;
use std::path::PathBuf;

use ropero_core::Environment;
use ropero_core::config::archivo_config;

fn env_with(vars: &[(&str, &str)], home: &PathBuf, cwd: &PathBuf) -> Environment {
    let mapa: BTreeMap<String, String> = vars
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    Environment::new(mapa, home, cwd)
}

#[test]
fn bootstrap_escribe_defaults_puros_y_override_solo_en_memoria() {
    let base = std::env::temp_dir().join("ropero-bootstrap-defaults-puros");
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("home");
    let cwd = base.join("cwd");
    let xdg = base.join("xdg");
    let override_dir = base.join("override-models");
    std::fs::create_dir_all(&cwd).unwrap();
    std::fs::create_dir_all(&override_dir).unwrap();

    let e1 = env_with(
        &[
            ("XDG_CONFIG_HOME", xdg.to_str().unwrap()),
            ("ROPERO_MODELS_DIR", override_dir.to_str().unwrap()),
        ],
        &home,
        &cwd,
    );
    let (estado, cfg, _) = ropero_runtime::prepara_config(&e1).expect("bootstrap");
    assert!(estado.existe);
    assert_eq!(cfg.models_dir.valor, override_dir);
    let archivo = archivo_config(&e1);
    let contenido = std::fs::read_to_string(&archivo).expect("config escrita");
    assert!(
        !contenido.contains(override_dir.to_str().unwrap()),
        "el archivo no debe persistir el override: {contenido}"
    );
    let defecto_models = xdg.join("ropero").join("models");
    let config_dir = xdg.join("ropero");
    assert!(config_dir.join("logs").is_dir());
    assert!(config_dir.join("docs/tests/indice.md").is_file());
    assert!(config_dir.join("docs/tests/test-autocritica.md").is_file());
    assert!(defecto_models.is_dir());
    let template = defecto_models.join("template.toml");
    assert!(template.is_file(), "debe instalar la plantilla de usuario");
    assert!(
        std::fs::read_to_string(&template)
            .unwrap()
            .contains("[model]"),
        "la plantilla debe tener el esquema de modelo"
    );
    assert!(
        contenido.contains(defecto_models.to_str().unwrap()),
        "el archivo debe contener el models_dir por defecto: {contenido}"
    );

    // Sin variable, la config efectiva vuelve al default del archivo (puro).
    let e2 = env_with(&[("XDG_CONFIG_HOME", xdg.to_str().unwrap())], &home, &cwd);
    let (_estado2, cfg2, _) = ropero_runtime::prepara_config(&e2).expect("segunda carga");
    assert_eq!(cfg2.models_dir.valor, defecto_models);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn bootstrap_idempotente_y_no_sobrescribe() {
    let base = std::env::temp_dir().join("ropero-bootstrap-idempotente");
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("home");
    let cwd = base.join("cwd");
    let xdg = base.join("xdg");
    std::fs::create_dir_all(&cwd).unwrap();
    let e = env_with(&[("XDG_CONFIG_HOME", xdg.to_str().unwrap())], &home, &cwd);
    let (_, _, _) = ropero_runtime::prepara_config(&e).expect("primero");
    let archivo = archivo_config(&e);
    let original = std::fs::read_to_string(&archivo).unwrap();
    let template = xdg.join("ropero/models/template.toml");
    std::fs::write(&template, "template personalizado\n").unwrap();
    let marcador = "\n# marcador de usuario\n";
    std::fs::write(&archivo, format!("{original}{marcador}")).unwrap();
    let mod1 = std::fs::metadata(&archivo).unwrap().modified().unwrap();
    let (_, _, _) = ropero_runtime::prepara_config(&e).expect("segundo");
    let despues = std::fs::read_to_string(&archivo).unwrap();
    assert!(
        despues.contains("# marcador de usuario"),
        "no debe sobrescribir"
    );
    let mod2 = std::fs::metadata(&archivo).unwrap().modified().unwrap();
    assert_eq!(mod1, mod2, "no debe reescribir el archivo existente");
    assert_eq!(
        std::fs::read_to_string(&template).unwrap(),
        "template personalizado\n",
        "no debe sobrescribir la plantilla existente"
    );
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn config_ilegible_no_se_oculta() {
    let base = std::env::temp_dir().join("ropero-bootstrap-ilegible");
    let _ = std::fs::remove_dir_all(&base);
    let home = base.join("home");
    let cwd = base.join("cwd");
    let xdg = base.join("xdg");
    std::fs::create_dir_all(&cwd).unwrap();
    let e = env_with(&[("XDG_CONFIG_HOME", xdg.to_str().unwrap())], &home, &cwd);
    ropero_runtime::prepara_config(&e).expect("bootstrap inicial");
    let archivo = archivo_config(&e);
    std::fs::write(&archivo, "esto = [no es toml valido\n").unwrap();
    let err = ropero_runtime::prepara_config(&e).unwrap_err();
    assert!(err.contains("configuración ilegible"), "{err}");
    let (estado, _, _) = ropero_runtime::carga_config(&e).expect("carga tolerante");
    assert!(estado.error.is_some(), "debe reportar error");
    let _ = std::fs::remove_dir_all(&base);
}
