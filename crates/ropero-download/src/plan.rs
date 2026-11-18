//! El **plan** de la descarga: qué se baja, a dónde y con qué argumentos.
//!
//! ## Por qué es puro
//!
//! Este módulo **no toca el disco**: calcula el plan a partir del modelo
//! y del entorno, y lo devuelve como dato (doc B §3.2.3). La E/S —lanzar
//! `hf`, crear el staging, mover ficheros— vive en [`crate::download`].
//!
//! La razón de la separación es la puerta de **T-26**: *«con `hf` falso:
//! se llama con los argumentos exactos»*. Los argumentos son parte del
//! plan, así que se pueden comparar **sin descargar un byte**.
//!
//! ## El destino
//!
//! El destino lo manda el **formato** (R-MOD-6), que es lo que dice
//! [`ropero_core::resolve_paths`]:
//!
//! - **GGUF** → `$GGUF_DIR/<modelo>/<[files].gguf>`;
//! - **safetensors** → `$SAFETENSORS_DIR/<[model].path>`.
//!
//! ## El staging
//!
//! El GGUF se baja a un **staging propio**, dentro del destino y en el
//! mismo filesystem, para que el `mv` sea atómico (R-MOD-15). Ese
//! directorio **no se puede saber al calcular el plan** (es único en cada
//! descarga), así que en los argumentos del plan va el marcador
//! [`STAGING`] y quien ejecuta lo sustituye por la ruta real.

use std::path::{Path, PathBuf};

use ropero_core::real_name;
use ropero_core::{Download, Environment, ModelFormat, ModelSpec, resolve_paths};

use crate::Error;
use crate::rutas::ruta_dentro;

/// El marcador que ocupa el staging en los argumentos del plan.
///
/// Se sustituye por la ruta real del staging en el momento de descargar.
pub const STAGING: &str = "<staging>";

/// Los ficheros que **siempre** se excluyen de un checkpoint.
///
/// Es la basura que no hace falta (los `.md`, las capturas, el
/// `.gitattributes`). Los del modelo se suman a estos.
pub const EXCLUDES_POR_DEFECTO: &[&str] = &[
    ".gitattributes",
    "*.md",
    ".DS_Store",
    "*.png",
    "*.jpg",
    "*.gif",
];

/// El marcador de un checkpoint completo.
///
/// Se toca **sólo** si los pesos están de verdad (R-MOD-21): un
/// `.download_complete` sin pesos es «un directorio roto, no un modelo
/// instalado».
pub const MARCADOR: &str = ".download_complete";

/// Lo que se baja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Formato {
    /// Un GGUF: **un fichero**.
    Gguf,
    /// Un checkpoint: **un directorio**.
    Safetensors,
    /// Pack nativo `.basalt`, descargado como un único fichero.
    BasaltPack,
}

/// Un auxiliar que se baja junto al artefacto principal.
///
/// Los auxiliares son los ficheros extra que el arranque referencia
/// (`${mtp}`, `${mmproj}`, `${chat_template}`): van al **mismo directorio**
/// que el GGUF (R-MOD-23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuxPlan {
    /// El tipo del auxiliar (`mtp`, `mmproj`, `chat_template`…).
    pub kind: String,
    /// El repo donde vive (puede ser **otro** que el del modelo).
    pub repo: String,
    /// El fichero **dentro** del repo.
    pub file: String,
    /// El nombre final en disco.
    pub name: String,
    /// La ruta final.
    pub dest: PathBuf,
    /// El tamaño (**informativo**, R-MOD-20).
    pub size: String,
    /// Los argumentos exactos de `hf` (con el staging como marcador).
    pub hf_args: Vec<String>,
}

/// El plan de la descarga de un modelo.
///
/// Es **dato**: no sabe nada de `hf`, del disco ni del terminal. Lo que
/// sabe es *qué* hay que bajar, *dónde* y *con qué argumentos*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// El nombre real del modelo.
    pub modelo: String,
    /// Lo que se baja.
    pub formato: Formato,
    /// El repo de HuggingFace.
    pub repo: String,
    /// El fichero dentro del repo (vacío en un checkpoint: es el repo
    /// entero).
    pub file: String,
    /// El directorio de destino.
    pub dest_dir: PathBuf,
    /// El directorio donde van los **auxiliares** (R-MOD-23).
    ///
    /// Es el directorio del GGUF del modelo, también en un checkpoint: es
    /// donde los referencia el arranque (`${mtp}`, `${mmproj}`,
    /// `${chat_template}`).
    pub aux_dir: PathBuf,
    /// La ruta final del artefacto principal.
    pub dest: PathBuf,
    /// El tamaño (**informativo**, R-MOD-20).
    pub size: String,
    /// Los patrones que se excluyen (sólo en un checkpoint).
    pub excludes: Vec<String>,
    /// Los auxiliares a bajar.
    pub aux: Vec<AuxPlan>,
    /// Los argumentos exactos de `hf` para el artefacto principal.
    pub hf_args: Vec<String>,
}

/// Calcula el plan de descarga de un modelo.
///
/// Es **puro**: no lee disco, no consulta el entorno y no lanza procesos.
/// El entorno llega por parámetro (doc B §3.2.3).
pub fn plan_de_descarga(spec: &ModelSpec, env: &Environment) -> Result<Plan, Error> {
    let modelo = real_name(spec);
    let Some(download) = spec.download.as_ref() else {
        return Err(Error::SinDescarga { modelo });
    };
    if download.repo.is_empty() {
        return Err(Error::DatosIncompletos {
            modelo,
            falta: "repo".to_string(),
        });
    }
    let paths = resolve_paths(spec, env);
    match spec.model.format {
        ModelFormat::Gguf => plan_gguf(download, &paths, env, modelo),
        ModelFormat::Safetensors => plan_safetensors(spec, download, env, modelo),
        ModelFormat::Basalt if spec.model.engine == ropero_core::EngineId::Basalt => {
            plan_basalt(spec, download, env, modelo)
        }
        otro => Err(Error::FormatoNoDescargable {
            modelo,
            formato: otro.as_str().to_string(),
        }),
    }
}

fn plan_basalt(
    spec: &ModelSpec,
    download: &Download,
    env: &Environment,
    modelo: String,
) -> Result<Plan, Error> {
    if download.file.is_empty() {
        return Err(Error::DatosIncompletos {
            modelo,
            falta: "file".to_string(),
        });
    }
    let root = env.engine_artifact_path(spec.model.engine, "ARTIFACTS_DIR", "models/basalt");
    let dest = ruta_dentro(&root, &spec.model.path, "[model].path", &modelo)?;
    let dest_dir = dest
        .parent()
        .ok_or_else(|| Error::DatosIncompletos {
            modelo: modelo.clone(),
            falta: "[model].path debe nombrar un fichero .basalt".to_string(),
        })?
        .to_path_buf();
    let hf_args = vec![
        "download".to_string(),
        download.repo.clone(),
        download.file.clone(),
        "--local-dir".to_string(),
        STAGING.to_string(),
    ];
    Ok(Plan {
        modelo,
        formato: Formato::BasaltPack,
        repo: download.repo.clone(),
        file: download.file.clone(),
        dest_dir: dest_dir.clone(),
        aux_dir: dest_dir,
        dest,
        size: download.size.clone(),
        excludes: Vec::new(),
        aux: Vec::new(),
        hf_args,
    })
}

/// El plan de un GGUF.
fn plan_gguf(
    download: &Download,
    paths: &ropero_core::ResolvedPaths,
    env: &Environment,
    modelo: String,
) -> Result<Plan, Error> {
    if download.file.is_empty() {
        return Err(Error::DatosIncompletos {
            modelo,
            falta: "file".to_string(),
        });
    }
    let dest_dir = env.env_path("GGUF_DIR", "models/gguf").join(&modelo);
    // El destino es donde lo busca el runtime: `$GGUF_DIR/<modelo>/<[files].gguf>`
    // (R-MOD-6), con su ruta **completa**: un `carpeta/mi.gguf` se descarga
    // en `…/<modelo>/carpeta/mi.gguf` (y el subdirectorio se crea).
    let (relativo, campo) = match paths.gguf.as_deref() {
        Some(gguf) => {
            let ruta = Path::new(gguf);
            let relativo = ruta.strip_prefix(&dest_dir).map_err(|_| Error::RutaFuera {
                modelo: modelo.clone(),
                campo: "[files].gguf".to_string(),
                valor: gguf.to_string(),
            })?;
            (relativo.to_string_lossy().to_string(), "[files].gguf")
        }
        // Sin `[files].gguf` se usa el nombre de la descarga (como antes).
        None => {
            if download.name.is_empty() {
                (download.file.clone(), "[download].file")
            } else {
                (download.name.clone(), "[download].name")
            }
        }
    };
    let dest = ruta_dentro(&dest_dir, &relativo, campo, &modelo)?;
    // El fichero del repo tampoco puede salirse: el `mv` de R-MOD-16.2 lo
    // mueve desde el directorio del modelo.
    let _ = ruta_dentro(&dest_dir, &download.file, "[download].file", &modelo)?;
    if !download.name.is_empty() {
        let _ = ruta_dentro(&dest_dir, &download.name, "[download].name", &modelo)?;
    }
    let hf_args = vec![
        "download".to_string(),
        download.repo.clone(),
        download.file.clone(),
        "--local-dir".to_string(),
        STAGING.to_string(),
    ];
    let aux = plan_aux(download, &dest_dir, &modelo)?;
    Ok(Plan {
        modelo,
        formato: Formato::Gguf,
        repo: download.repo.clone(),
        file: download.file.clone(),
        aux_dir: dest_dir.clone(),
        dest_dir,
        dest,
        size: download.size.clone(),
        excludes: Vec::new(),
        aux,
        hf_args,
    })
}

/// El plan de un checkpoint.
fn plan_safetensors(
    spec: &ModelSpec,
    download: &Download,
    env: &Environment,
    modelo: String,
) -> Result<Plan, Error> {
    if spec.model.path.is_empty() {
        return Err(Error::DatosIncompletos {
            modelo,
            falta: "[model].path".to_string(),
        });
    }
    // El checkpoint va a `$SAFETENSORS_DIR/<[model].path>` (R-MOD-6), y su
    // ruta tampoco puede salirse de ese directorio.
    let dest_dir = ruta_dentro(
        &env.env_path("SAFETENSORS_DIR", "models/safetensors"),
        &spec.model.path,
        "[model].path",
        &modelo,
    )?;
    let mut excludes: Vec<String> = EXCLUDES_POR_DEFECTO
        .iter()
        .map(|e| (*e).to_string())
        .collect();
    for excluido in &download.excludes {
        if !excluido.is_empty() {
            excludes.push(excluido.clone());
        }
    }
    let mut hf_args = vec![
        "download".to_string(),
        download.repo.clone(),
        "--local-dir".to_string(),
        dest_dir.to_string_lossy().to_string(),
    ];
    for excluido in &excludes {
        hf_args.push("--exclude".to_string());
        hf_args.push(excluido.clone());
    }
    // Los auxiliares van al directorio del GGUF, también en un checkpoint:
    // es donde los referencia el arranque (R-MOD-23).
    let aux_dir = env.env_path("GGUF_DIR", "models/gguf").join(&modelo);
    let aux = plan_aux(download, &aux_dir, &modelo)?;
    Ok(Plan {
        modelo,
        formato: Formato::Safetensors,
        repo: download.repo.clone(),
        file: String::new(),
        aux_dir,
        dest: dest_dir.clone(),
        dest_dir,
        size: download.size.clone(),
        excludes,
        aux,
        hf_args,
    })
}

/// El plan de los auxiliares.
///
/// Sólo bajan los de `use = "run"`: los de `build` (imatrix, tokenizer)
/// son cosa del gguffer (R-MOD-23).
fn plan_aux(download: &Download, dest_dir: &Path, modelo: &str) -> Result<Vec<AuxPlan>, Error> {
    let mut out = Vec::new();
    for (kind, aux) in &download.aux {
        if aux.file.is_empty() {
            return Err(Error::DatosIncompletos {
                modelo: modelo.to_string(),
                falta: format!("file (aux {kind})"),
            });
        }
        if uso_de_aux(kind, &aux.use_) != "run" {
            continue;
        }
        let repo = if aux.repo.is_empty() {
            download.repo.clone()
        } else {
            aux.repo.clone()
        };
        // El auxiliar va al **mismo directorio** que el GGUF (R-MOD-23), y
        // su ruta tampoco puede salirse de ahí.
        let (nombre, campo) = if aux.name.is_empty() {
            (aux.file.clone(), format!("[download.aux.{kind}].file"))
        } else {
            (aux.name.clone(), format!("[download.aux.{kind}].name"))
        };
        let dest = ruta_dentro(dest_dir, &nombre, &campo, modelo)?;
        let _ = ruta_dentro(
            dest_dir,
            &aux.file,
            &format!("[download.aux.{kind}].file"),
            modelo,
        )?;
        let hf_args = vec![
            "download".to_string(),
            repo.clone(),
            aux.file.clone(),
            "--local-dir".to_string(),
            STAGING.to_string(),
        ];
        out.push(AuxPlan {
            kind: kind.clone(),
            repo,
            file: aux.file.clone(),
            name: nombre,
            dest,
            size: aux.size.clone(),
            hf_args,
        });
    }
    Ok(out)
}

/// El uso de un auxiliar: `run` o `build`.
///
/// El que declara el TOML manda; si no, manda el del tipo (R-DATA-4):
/// `run` para los de runtime (`mtp`, `mmproj`, `chat_template`) y `build`
/// para los de fabricación (`imatrix`, `tokenizer`).
fn uso_de_aux(kind: &str, declarado: &str) -> &'static str {
    match declarado {
        "run" => "run",
        "build" => "build",
        _ => match kind {
            "imatrix" | "tokenizer" => "build",
            _ => "run",
        },
    }
}
