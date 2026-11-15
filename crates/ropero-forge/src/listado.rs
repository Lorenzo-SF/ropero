//! La **lista de quants** (`--list-quants`): los del motor y los ya
//! publicados de ese modelo.
//!
//! Son dos listas, y juntas contestan la pregunta que hace alguien que
//! va a fabricar:
//!
//! - los **quants del motor**: los que el motor publica
//!   (`llama-quantize --help`), con su número de `llama_ftype` — que es
//!   justo el `general.file_type` del GGUF (ver [`crate::quants`]);
//! - los **ya publicados** de ese modelo: los GGUFs que hay en el
//!   directorio de salida, que se **bajan** en vez de fabricarse.
//!
//! ## Sin motor
//!
//! Sin motor no hay lista de verdad: lo que se enseña es la **tabla de
//! reserva** (la que trae la forja), y el reporte dice **qué hacer** para
//! tener la del motor. La tabla de reserva no miente: son los mismos
//! números, pero los **publicados** por el motor pueden cambiar.

use std::path::{Path, PathBuf};

use ropero_core::Environment;

use crate::peticion::{Flags, Fuente};
use crate::plan;
use crate::quants;
use crate::reporte::Reporte;
use crate::toolchain;

/// Las líneas de `--list-quants`, con el error de la precondición si el
/// motor no está (R-GGUF-54…57).
pub fn listar(env: &Environment, flags: &Flags) -> Reporte {
    let mut lineas = Vec::new();
    match toolchain::quants_del_motor(env) {
        Some((ruta, del_motor)) => {
            lineas.push(format!("Quants del motor ({}):", ruta.to_string_lossy()));
            for (num, nombre) in del_motor {
                lineas.push(format!("{num:>3}  {nombre}"));
            }
        }
        None => {
            lineas.push(
                "Quants conocidos (tabla de reserva: sin motor no se ven los del motor):"
                    .to_string(),
            );
            for linea in quants::lista() {
                lineas.push(linea);
            }
        }
    }
    if let Some(fuente) = fuente_de(flags) {
        let nombre = plan::nombre_de_fuente(&fuente);
        lineas.push(String::new());
        lineas.push(format!("Quants ya publicados de {nombre}:"));
        let publicados = publicados(env, &nombre);
        if publicados.is_empty() {
            lineas.push("  (ninguno)".to_string());
        } else {
            for (quant, fichero) in publicados {
                lineas.push(format!("  {quant:<12} {fichero}"));
            }
        }
    }
    match toolchain::buscar(env, true) {
        Ok(_) => Reporte::nuevo(lineas),
        Err(error) => Reporte::con_error(lineas, error),
    }
}

/// La fuente de las flags, si la hay.
fn fuente_de(flags: &Flags) -> Option<Fuente> {
    if let Some(repo) = &flags.repo {
        return Some(Fuente::Repo(repo.clone()));
    }
    if let Some(path) = &flags.path {
        return Some(Fuente::Path(PathBuf::from(path)));
    }
    flags.gguf.clone().map(Fuente::Gguf)
}

/// Los quants **ya publicados** de un modelo: los GGUFs que hay en el
/// directorio de salida, tanto en su nivel plano como en el subdirectorio del
/// modelo (`<GGUF_DIR>/<modelo>/`), que es donde los deja el downloader
/// (R-MOD-6). Mirar sólo el nivel plano hacía que un modelo ya descargado
/// saliera como «(ninguno)».
///
/// Se leen del disco: son las cuantizaciones que ya están hechas, que es lo
/// que se puede **bajar** en vez de fabricar. La línea enseña el fichero con
/// su carpeta, que es la ruta por la que se baja.
fn publicados(env: &Environment, nombre: &str) -> Vec<(String, String)> {
    let dir = env.env_path("GGUF_DIR", "models/gguf");
    let Ok(entradas) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if ruta.is_file() {
            if let Some(quant) = cuantizacion_de(&ruta, nombre) {
                out.push((quant, nombre_de_file(&ruta)));
            }
            continue;
        }
        // El primer nivel de subdirectorios: la carpeta del modelo.
        let Ok(sub) = std::fs::read_dir(&ruta) else {
            continue;
        };
        let carpeta = nombre_de_file(&ruta);
        for entrada in sub.flatten() {
            let ruta = entrada.path();
            if !ruta.is_file() {
                continue;
            }
            if let Some(quant) = cuantizacion_de(&ruta, nombre) {
                out.push((quant, format!("{carpeta}/{}", nombre_de_file(&ruta))));
            }
        }
    }
    out.sort();
    out
}

/// El quant de un GGUF publicado, por lo que **dice** su nombre: el trozo
/// después del último separador (`modelo-Q4_K_M.gguf` → `Q4_K_M`), o el quant
/// que lleva en el propio nombre del modelo (`modelo-NVFP4.gguf` → `NVFP4`).
///
/// Sólo cuenta si el fichero es de ese modelo: la línea no debe mezclar
/// modelos.
fn cuantizacion_de(ruta: &Path, nombre: &str) -> Option<String> {
    let fichero = nombre_de_file(ruta);
    let stem = fichero.strip_suffix(".gguf")?;
    let es_del_modelo = stem == nombre
        || stem.starts_with(&format!("{nombre}-"))
        || ruta
            .parent()
            .map(|padre| nombre_de_file(padre) == nombre)
            .unwrap_or(false);
    if !es_del_modelo {
        return None;
    }
    let quant = stem.rsplit('-').next()?;
    quants::numero_de(quant)?;
    Some(quant.to_string())
}

/// El nombre del fichero (o del directorio), sin la ruta.
fn nombre_de_file(ruta: &Path) -> String {
    ruta.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
