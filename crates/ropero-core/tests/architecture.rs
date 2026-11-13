use std::fs;
use std::path::PathBuf;

fn collect_rs(dir: &PathBuf) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs(&path));
            } else if path.extension().map(|ext| ext == "rs").unwrap_or(false) {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn ropero_core_has_no_forbidden_io_or_network() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in collect_rs(&src) {
        let content = fs::read_to_string(&file).expect("read source file");
        for token in ["std::process", "std::fs", "tokio", "reqwest"] {
            assert!(
                !content.contains(token),
                "forbidden token `{token}` found in {}",
                file.display()
            );
        }
    }
}

#[test]
fn ropero_core_does_not_depend_on_workspace_crates() {
    let cargo_toml = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml).expect("read Cargo.toml");
    let forbidden = [
        "ropero-runtime",
        "ropero-download",
        "ropero-forge",
        "ropero-server",
        "ropero-cli",
        "tokio",
        "reqwest",
    ];
    for token in forbidden {
        assert!(
            !content.contains(token),
            "ropero-core Cargo.toml must not mention `{token}`"
        );
    }
}

#[test]
fn ropero_cli_under_400_lines() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let cli_src = repo_root.join("crates").join("ropero-cli").join("src");
    let mut lines = 0usize;
    for file in collect_rs(&cli_src) {
        let content = fs::read_to_string(&file).expect("read cli source");
        lines += content.lines().count();
    }
    assert!(
        lines < 400,
        "ropero-cli has {lines} lines, expected fewer than 400"
    );
}

/// `ropero-core` no expone la misma función libre bajo varios nombres.
///
/// Los alias de una función (`artifact_location`, `expected_location`,
/// `location`, `where_lives`, `path_for`…) inflan la API sin añadir nada y
/// hacen imposible saber cuál es la buena: el mismo cuerpo con otro
/// nombre. Aquí se comprueba que no reaparezcan.
///
/// El examen es el de las **funciones libres** (las de módulo): un método
/// puede llamarse igual que otro si vive en otro tipo (`as_str` existe en
/// `ModelFormat` y en `EngineId`, y no son el mismo código).
#[test]
fn ropero_core_no_expone_alias_redundantes() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    // Nombres que ya son alias de otra función libre del core. Añadir uno
    // nuevo es ruido: se comprueba con el nombre, que es lo que un lector
    // ve.
    let alias_conocidos = [
        "artifact_location",
        "expected_location",
        "where_lives",
        "path_for",
        "artifact_path",
    ];
    let mut libres: Vec<(String, PathBuf)> = Vec::new();
    for file in collect_rs(&src) {
        let content = fs::read_to_string(&file).expect("read source file");
        for linea in content.lines() {
            let trimmed = linea.trim();
            // Una función libre empieza en el margen: `pub fn <nombre>(`.
            if linea.starts_with("pub fn ") {
                let nombre: String = trimmed["pub fn ".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !nombre.is_empty() {
                    libres.push((nombre, file.clone()));
                }
            }
        }
    }
    // 1. El nombre de una función libre es único en todo el core.
    let mut nombres: Vec<&str> = libres.iter().map(|(n, _)| n.as_str()).collect();
    nombres.sort_unstable();
    for par in nombres.windows(2) {
        assert_ne!(
            par[0], par[1],
            "la función libre '{0}' está repetida en ropero-core: la API no lleva \
             alias redundantes de la misma función",
            par[0]
        );
    }
    // 2. Y no hay alias de una función que ya existe como método.
    for alias in alias_conocidos {
        let hits: Vec<&PathBuf> = libres
            .iter()
            .filter(|(n, _)| n == alias)
            .map(|(_, f)| f)
            .collect();
        assert!(
            hits.is_empty(),
            "'{alias}' es un alias redundante de una función que ya existe como \
             método: {:?}",
            hits
        );
    }
}
