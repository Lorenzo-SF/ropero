//! Tests de `ropero models`: los modelos disponibles, agrupados por motor,
//! con la leyenda de colores.
//!
//! Van en su propio fichero porque son los del bloque 2 del ciclo: el
//! listado con grupos y colores es lo que esa bloque trae.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Raíz del catálogo de pruebas: `<cargo-manifest-dir>/tests/fixtures`.
fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Corre `ropero` contra el catálogo de pruebas.
fn run(args: &[&str]) -> (i32, String, String) {
    run_with_models(args, &fixture_root())
}

fn run_with_models(args: &[&str], models: &Path) -> (i32, String, String) {
    run_with_home(args, models, None)
}

fn run_with_home(args: &[&str], models: &Path, home: Option<&Path>) -> (i32, String, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ropero"));
    command
        .args(args)
        .env("ROPERO_MODELS_DIR", models)
        .env("GGUF_DIR", fixture_root().join("gguf"))
        .env("SAFETENSORS_DIR", fixture_root().join("safetensors"))
        .env("LOG_DIR", fixture_root().join("logs"));
    if let Some(home) = home {
        command.env("HOME", home).env_remove("XDG_CONFIG_HOME");
    }
    let out = command.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn model_fired_y_hired_mueve_el_toml_sin_perderlo() {
    let scratch = std::env::temp_dir().join(format!(
        "ropero-cli-model-state-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let models = scratch.join("models");
    copy_tree(&fixture_root().join("models"), &models);
    let active = models.join("turbo-llama/modelo-alpha.toml");
    let fired = models.join("turbo-llama/fired/modelo-alpha.toml");
    let original = std::fs::read(&active).unwrap();

    let (code, _, stderr) = run_with_models(&["model", "modelo-alpha", "--fired"], &models);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(!active.exists());
    assert_eq!(std::fs::read(&fired).unwrap(), original);
    let (code, _, _) = run_with_models(&["model", "modelo-alpha", "--fired"], &models);
    assert_eq!(code, 1, "no debe despedirse dos veces");

    let (code, _, stderr) = run_with_models(&["model", "modelo-alpha", "--hired"], &models);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(!fired.exists());
    assert_eq!(std::fs::read(&active).unwrap(), original);
    let (code, _, _) = run_with_models(&["model", "modelo-alpha", "--hired"], &models);
    assert_eq!(code, 1, "no debe reactivarse dos veces");
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn model_requiere_un_estado_y_all_fired_se_acepta_como_sinonimo_de_fired() {
    for args in [
        &["model", "modelo-alpha"][..],
        &["model", "modelo-alpha", "--fired", "--hired"][..],
        &["models", "add", "--all"][..],
    ] {
        let (code, _, _) = run(args);
        assert_eq!(code, 2, "uso aceptado: {args:?}");
    }
    let (code, stdout, stderr) = run(&["models", "--all", "--fired"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("FIRED MODELS"), "stdout={stdout}");
}

#[test]
fn el_listado_y_la_vista_fired_usan_ingles_por_defecto() {
    let home = std::env::temp_dir().join(format!(
        "ropero-cli-english-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&home).unwrap();
    let (code, stdout, stderr) = run_with_home(&["models", "--all"], &fixture_root(), Some(&home));
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("MODELS"), "stdout={stdout}");
    assert!(
        stdout.contains("model") && stdout.contains("artifact"),
        "stdout={stdout}"
    );
    let (code, fired, stderr) = run_with_home(&["models", "--fired"], &fixture_root(), Some(&home));
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(fired.contains("FIRED MODELS"), "stdout={fired}");
    std::fs::write(home.join(".config/ropero/config.toml"), "lang = \"ES\"\n").unwrap();
    let (code, spanish, stderr) = run_with_home(&["models", "--all"], &fixture_root(), Some(&home));
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(spanish.contains("MODELOS"), "stdout={spanish}");
    assert!(
        spanish.contains("modelo") && spanish.contains("artefacto"),
        "stdout={spanish}"
    );
    let (code, fired, stderr) = run_with_home(&["models", "--fired"], &fixture_root(), Some(&home));
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(fired.contains("MODELOS DESPEDIDOS"), "stdout={fired}");
    let _ = std::fs::remove_dir_all(home);
}

/// está instalado, su grupo no se despliega.
#[test]
fn list_agrupa_por_motor() {
    let (code, stdout, stderr) = run(&["models"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    // El listado normal puede contener sólo los motores detectados en esta máquina.
    assert!(!stdout.contains("not installed"), "stdout={stdout}");
}

#[test]
fn list_fired_muestra_solo_modelos_despedidos() {
    let (code, stdout, stderr) = run(&["models", "--fired"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("modelo-retirado"), "stdout={stdout}");
    assert!(!stdout.contains("modelo-alpha"), "stdout={stdout}");
}

#[test]
fn models_engine_muestra_activos_y_despedidos_y_filtra_fired() {
    let (code, stdout, stderr) = run(&["models", "--engine", "turbo-llama"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(stdout.contains("modelo-alpha"), "stdout={stdout}");
    assert!(stdout.contains("modelo-retirado"), "stdout={stdout}");
    assert!(stdout.contains("[fired]"), "stdout={stdout}");
    let (code, stdout, stderr) = run(&["models", "--fired", "--engine", "turbo-llama"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert!(!stdout.contains("modelo-alpha"), "stdout={stdout}");
    assert!(stdout.contains("modelo-retirado"), "stdout={stdout}");
}

/// instalado: no están disponibles (doc CLI v2 §15).
#[test]
fn list_no_saca_los_no_disponibles() {
    let (code, stdout, stderr) = run(&["models"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(
        !stdout.contains("modelo-alpha"),
        "un modelo de un motor no instalado no está disponible: stdout={}",
        stdout
    );
    assert!(
        !stdout.contains("modelo-beta"),
        "un modelo de un motor no instalado no está disponible: stdout={}",
        stdout
    );
}

/// `ropero models --all` saca también los que no están disponibles, marcados.
#[test]
fn list_all_marca_los_no_disponibles() {
    let (code, stdout, stderr) = run(&["models", "--all"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    assert!(stdout.contains("modelo-alpha"), "stdout={}", stdout);
    assert!(
        stdout.contains("unavailable"),
        "lo no disponible va marcado: stdout={}",
        stdout
    );
}

/// leyenda.
#[test]
fn list_pinta_con_el_color_de_la_leyenda() {
    let (code, stdout, stderr) = run(&["models", "--all"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    // turbo-llama uses the central hexadecimal background color.
    assert!(
        stdout.contains("\x1b[48;2;253;230;138m\x1b[38;2;0;0;0m"),
        "el motor no va en su color: stdout={:?}",
        stdout
    );
}

/// La cabecera sale **una sola vez** con varios motores: el motor es una fila
/// de grupo, no una tabla nueva.
#[test]
fn el_listado_pone_una_sola_cabecera() {
    // La cabecera se reconoce por sus rótulos de columna, no por un nombre de
    // modelo: un modelo llamado `modelo-alpha` empieza igual que la cabecera.
    let es_cabecera = |linea: &str| {
        (linea.contains("model") && linea.contains("size") && linea.contains("artifact"))
            || (linea.contains("modelo") && linea.contains("tamaño") && linea.contains("artefacto"))
    };
    let (code, stdout, stderr) = run(&["models", "--all"]);
    assert_eq!(code, 0, "stderr={stderr}");
    // --all muestra la lista general y, en una segunda sección, los fired.
    let cabeceras: Vec<&str> = stdout.lines().filter(|l| es_cabecera(l)).collect();
    assert_eq!(cabeceras.len(), 2, "stdout={stdout}");
    // La regla va sangrada: se cuenta la línea de guiones, no sus espacios.
    let regla = stdout
        .lines()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty() && l.chars().all(|c| c == '─')
        })
        .count();
    assert_eq!(
        regla, 4,
        "una regla por grupo de motor incluyendo fired, stdout={stdout}"
    );
    assert!(
        stdout.contains("mlx-serve") && stdout.contains("turbo-llama"),
        "stdout={stdout}"
    );
    // Plegado, en el fixture ningún motor tiene modelos: una línea por motor
    // y ninguna cabecera. La cabecera plegada (con `nº` y `motor`) sólo sale
    // cuando hay modelos que enumerar, y entonces es una sola.
    let (code, stdout, stderr) = run(&["models"]);
    assert_eq!(code, 0, "stderr={stderr}");
    assert_eq!(
        stdout.lines().filter(|l| es_cabecera(l)).count(),
        0,
        "stdout={stdout}"
    );
}

/// Ninguna línea del listado lleva espacios al final.
#[test]
fn el_listado_no_deja_espacios_al_final() {
    for args in [&["models"][..], &["models", "--all"][..]] {
        let (code, stdout, stderr) = run(args);
        assert_eq!(code, 0, "stderr={stderr}");
        for linea in stdout.lines() {
            assert_eq!(linea.trim_end(), linea, "linea={linea:?}");
        }
    }
}

/// El tamaño tiene **una sola forma** en toda la salida: `~N.NN UNIDAD`.
///
/// Antes de la regla única salía `~4 GB` en un motor y `18.9 GB` en otro, y
/// la columna del tamaño no se alineaba.
#[test]
fn el_listado_da_una_sola_forma_al_tamano() {
    let (code, stdout, stderr) = run(&["models", "--all"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let general = stdout.split("FIRED MODELS").next().unwrap_or(&stdout);
    assert!(
        stdout.contains("FIRED MODELS"),
        "--all debe incluir fired: {stdout}"
    );
    let tamaños: Vec<String> = general
        .lines()
        .filter_map(|l| {
            let i = l.find(" GB")?;
            let size = &l[..i + 3];
            size.contains('~').then(|| size.to_string())
        })
        .collect();
    assert!(tamaños.len() >= 3, "stdout={stdout}");
    for tamano in &tamaños {
        // Un solo `~` (el `~` del tamaño, no dos) y dos decimales.
        assert_eq!(tamano.matches('~').count(), 1, "tamano={tamano}");
        let numero = tamano[1..tamano.len() - 3].to_string();
        let decimales = numero.split('.').nth(1).expect("el tamaño lleva decimales");
        assert_eq!(decimales.len(), 2, "tamano={tamano}");
    }
    // Y la columna del tamaño cae en la misma posición para todos.
    let posiciones: Vec<usize> = general
        .lines()
        .filter_map(|l| {
            let i = l.find(" GB")?;
            l[..i].contains('~').then_some(i)
        })
        .collect();
    assert!(
        posiciones.windows(2).all(|w| w[0] == w[1]),
        "posiciones={posiciones:?}"
    );
}

/// La columna del estado **no** se mueve: lo no disponible se marca en su
/// columna, no pegado al artefacto.
#[test]
fn la_columna_del_estado_no_se_mueve() {
    let (code, stdout, stderr) = run(&["models", "--all"]);
    assert_eq!(code, 0, "stderr={stderr}");
    let posiciones: Vec<usize> = stdout
        .lines()
        .filter_map(|l| l.find("unavailable"))
        .collect();
    assert!(posiciones.len() >= 2, "stdout={stdout}");
    assert!(
        posiciones.windows(2).all(|w| w[0] == w[1]),
        "la columna del estado se mueve: {posiciones:?}"
    );
}
