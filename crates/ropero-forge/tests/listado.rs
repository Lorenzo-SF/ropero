//! La lista de quants: los del motor y los **ya publicados** de ese modelo
//! (`--list-quants`, R-GGUF-54…57).
//!
//! Lo que se afirma aquí es la parte de la lista que contesta a la pregunta
//! real: qué está publicado ya de ese modelo, que es lo que se **baja** en vez
//! de fabricarse. Sin motor la lista de quants es la tabla de reserva, así que
//! el reporte lleva el error de la precondición y las líneas se enseñan igual.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use ropero_core::Environment;
use ropero_forge::listado;
use ropero_forge::peticion::Flags;

static CONTADOR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Un entorno con `GGUF_DIR` en un directorio limpio (sin motor: la parte de
/// la lista que se afirma no depende de él).
fn entorno(nombre: &str) -> (PathBuf, Environment) {
    let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base =
        std::env::temp_dir().join(format!("ropero-forge-listado-{}-{n}", std::process::id(),));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(base.join(nombre)).expect("crear el directorio de salida");
    let gguf_dir = base.join(nombre);
    let mut vars = BTreeMap::new();
    vars.insert(
        "GGUF_DIR".to_string(),
        gguf_dir.to_string_lossy().into_owned(),
    );
    let env = Environment::new(
        vars,
        PathBuf::from("/no/existe/home"),
        PathBuf::from("/tmp"),
    );
    (base, env)
}

/// **Hallazgo 3**: «ya publicados» sólo miraba el **nivel plano** de
/// `$GGUF_DIR`, pero el downloader deja los GGUFs en `$GGUF_DIR/<modelo>/`
/// (R-MOD-6) ⇒ decía «(ninguno)» para un modelo ya descargado.
#[test]
fn los_publicados_tambien_estan_en_el_subdirectorio_del_modelo() {
    let (base, env) = entorno("gguf");
    let dir = base.join("gguf");
    fs::create_dir_all(dir.join("modelo")).expect("crear la carpeta del modelo");
    fs::write(dir.join("modelo").join("modelo-Q4_K_M.gguf"), b"GGUF").expect("el gguf publicado");

    let flags = Flags::default().con_repo("acme/modelo");
    let reporte = listado::listar(&env, &flags);
    let publicada = reporte
        .lineas
        .iter()
        .find(|linea| linea.contains(".gguf") && linea.starts_with("  "))
        .cloned()
        .unwrap_or_default();
    assert!(
        publicada.contains("modelo/modelo-Q4_K_M.gguf"),
        "no encuentra el GGUF del subdirectorio: {publicada}"
    );
    assert!(
        !reporte.lineas.iter().any(|l| l.contains("(ninguno)")),
        "dice (ninguno) para un modelo ya descargado: {:?}",
        reporte.lineas
    );
    let _ = fs::remove_dir_all(&base);
}

/// **Hallazgo 3**: el artefacto **colocado** (el `<modelo>.gguf`, cuando el
/// nombre ya trae el quant) también es un quant publicado.
#[test]
fn el_artefacto_colocado_tambien_esta_publicado() {
    let (base, env) = entorno("gguf");
    let dir = base.join("gguf");
    fs::write(dir.join("modelo-NVFP4.gguf"), b"GGUF").expect("el gguf colocado");

    let flags = Flags::default().con_repo("acme/modelo-NVFP4");
    let reporte = listado::listar(&env, &flags);
    let publicada = reporte
        .lineas
        .iter()
        .find(|linea| linea.starts_with("  ") && linea.contains("modelo-NVFP4.gguf"))
        .cloned()
        .unwrap_or_default();
    assert!(
        publicada.contains("NVFP4"),
        "no lista el quant del artefacto colocado: {publicada}"
    );
    assert!(
        Path::new(&dir).join("modelo-NVFP4.gguf").is_file(),
        "fixture"
    );
    let _ = fs::remove_dir_all(&base);
}

/// Y en el subdirectorio, el artefacto colocado se lista con su carpeta.
#[test]
fn el_artefacto_colocado_en_el_subdirectorio_se_lista_con_su_carpeta() {
    let (base, env) = entorno("gguf");
    let dir = base.join("gguf");
    fs::create_dir_all(dir.join("modelo-NVFP4")).expect("crear la carpeta del modelo");
    fs::write(dir.join("modelo-NVFP4").join("modelo-NVFP4.gguf"), b"GGUF")
        .expect("el gguf colocado");

    let flags = Flags::default().con_repo("acme/modelo-NVFP4");
    let reporte = listado::listar(&env, &flags);
    assert!(
        reporte
            .lineas
            .iter()
            .any(|l| l.contains("modelo-NVFP4/modelo-NVFP4.gguf")),
        "no lista el artefacto del subdirectorio: {:?}",
        reporte.lineas
    );
    let _ = fs::remove_dir_all(&base);
}
