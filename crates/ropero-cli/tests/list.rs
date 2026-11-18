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
    let out = Command::new(env!("CARGO_BIN_EXE_ropero"))
        .args(args)
        .env("ROPERO_MODELS_DIR", fixture_root())
        .env("GGUF_DIR", fixture_root().join("gguf"))
        .env("SAFETENSORS_DIR", fixture_root().join("safetensors"))
        .env("LOG_DIR", fixture_root().join("logs"))
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

/// está instalado, su grupo no se despliega.
#[test]
fn list_agrupa_por_motor() {
    let (code, stdout, stderr) = run(&["models"]);
    assert_eq!(code, 0, "stderr={}", stderr);
    // Los motores del fixture salen, cada uno en su color de la leyenda.
    assert!(stdout.contains("turbo-llama"), "stdout={}", stdout);
    assert!(stdout.contains("llama-server"), "stdout={}", stdout);
    // Y un motor no instalado dice por qué.
    assert!(
        stdout.contains("no está instalado"),
        "el grupo plegado no dice la razón: stdout={}",
        stdout
    );
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
        stdout.contains("no disponible"),
        "lo no disponible va marcado: stdout={}",
        stdout
    );
}

/// leyenda.
#[test]
fn list_pinta_con_el_color_de_la_leyenda() {
    let (code, stdout, stderr) = run(&["models"]);
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
        linea.contains("modelo") && linea.contains("tamaño") && linea.contains("artefacto")
    };
    let (code, stdout, stderr) = run(&["models", "--all"]);
    assert_eq!(code, 0, "stderr={stderr}");
    // En el fixture hay tres motores con modelos y la cabecera no se repite.
    let cabeceras: Vec<&str> = stdout.lines().filter(|l| es_cabecera(l)).collect();
    assert_eq!(cabeceras.len(), 1, "stdout={stdout}");
    // La regla va sangrada: se cuenta la línea de guiones, no sus espacios.
    let regla = stdout
        .lines()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty() && l.chars().all(|c| c == '─')
        })
        .count();
    assert_eq!(
        regla, 3,
        "una sola regla por grupo de motor, stdout={stdout}"
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
    let tamaños: Vec<String> = stdout
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
    let posiciones: Vec<usize> = stdout
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
        .filter_map(|l| l.find("no disponible"))
        .collect();
    assert!(posiciones.len() >= 2, "stdout={stdout}");
    assert!(
        posiciones.windows(2).all(|w| w[0] == w[1]),
        "la columna del estado se mueve: {posiciones:?}"
    );
}
