//! La **precondición del motor** (R-GGUF-54…57): sin llama.cpp no hay
//! conversión, ni cuantización, ni unión de shards.
//!
//! ## Qué busca y dónde
//!
//! El motor se busca en **`$LLAMA_DIR`** (por defecto `~/llama.cpp`, el
//! fork) y en **`$LLAMA_OFFICIAL_DIR`** (por defecto
//! `~/llama.cpp-official`, el oficial): los dos motores no dejan el clon
//! en el mismo sitio, y la v2 mira **los dos** (R-GGUF-57). De ahí salen:
//!
//! | Qué | Dónde |
//! |---|---|
//! | el conversor | `<dir>/convert_hf_to_gguf.py` (**sin alternativa**) |
//! | el cuantizador | el `PATH` → si no, `<dir>/build/bin/llama-quantize` |
//! | el unidor de shards | el `PATH` → si no, `<dir>/build/bin/llama-gguf-split` |
//!
//! ## El conversor no admite sustituto (R-GGUF-55)
//!
//! Sin `convert_hf_to_gguf.py` **no hay conversión**, y por tanto no hay
//! GGUF — con o sin quant. El cuantizador y el unidor sí tienen
//! alternativa en el `PATH`; el conversor, no.
//!
//! ## La precondición va ANTES de descargar (R-GGUF-56, T-29)
//!
//! La función [`ensure_toolchain`] es **pura**: entra lo que hay, sale el
//! motor o el error — que **nombra el motor que falta** y **la variable
//! que lo apunta**. El mensaje falso de la v1 («faltan ficheros en el
//! clon: la descarga está incompleta») empujaba a borrar 60 GB por un
//! problema que no era de la descarga (H-27); este mensaje no.

use std::path::{Path, PathBuf};

use crate::Error;

/// Qué motor se encontró.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Motor {
    /// El fork: `$LLAMA_DIR` (por defecto `~/llama.cpp`).
    Fork { dir: PathBuf },
    /// El oficial: `$LLAMA_OFFICIAL_DIR` (por defecto
    /// `~/llama.cpp-official`).
    Oficial { dir: PathBuf },
}

impl Motor {
    /// Su nombre, para los mensajes.
    pub fn nombre(&self) -> &'static str {
        match self {
            Motor::Fork { .. } => "llama.cpp (el fork)",
            Motor::Oficial { .. } => "llama.cpp (el oficial)",
        }
    }

    /// Su directorio.
    pub fn dir(&self) -> &Path {
        match self {
            Motor::Fork { dir } | Motor::Oficial { dir } => dir,
        }
    }
}

/// El motor que la tirada va a usar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toolchain {
    /// Qué motor es.
    pub motor: Motor,
    /// El conversor: `<motor>/convert_hf_to_gguf.py`.
    pub convert: PathBuf,
    /// El cuantizador: el `PATH` o `<motor>/build/bin/llama-quantize`.
    pub quantize: PathBuf,
    /// El unidor de shards, si está (no es imprescindible: sólo une).
    pub split: Option<PathBuf>,
}

/// Lo que el entorno ofrece al [`ensure_toolchain`]: candidatos ya
/// **comprobados** contra el disco por quien llama.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Busqueda {
    /// El directorio del fork (`$LLAMA_DIR`), si está.
    pub dir_fork: Option<PathBuf>,
    /// El directorio del oficial (`$LLAMA_OFFICIAL_DIR`), si está.
    pub dir_oficial: Option<PathBuf>,
    /// El conversor que se encontró en el fork, si está.
    pub convert_fork: Option<PathBuf>,
    /// El conversor que se encontró en el oficial, si está.
    pub convert_oficial: Option<PathBuf>,
    /// El cuantizador que se encontró en el `PATH`, si está.
    pub quantize_en_path: Option<PathBuf>,
    /// El cuantizador del build del fork, si está.
    pub quantize_fork: Option<PathBuf>,
    /// El cuantizador del build del oficial, si está.
    pub quantize_oficial: Option<PathBuf>,
    /// El unidor de shards que se encontró en el `PATH`, si está.
    pub split_en_path: Option<PathBuf>,
    /// El unidor de shards del build del motor, si está (se rellena
    /// después de elegir el motor, por eso es una sola ruta).
    pub split_en_build: Option<PathBuf>,
}

/// La precondición del motor: **pura**, sin tocar el disco.
///
/// Con la búsqueda hecha ([`Busqueda`]), decide qué motor se usa y qué
/// falta. El orden es: el fork primero (es el que ropero instala por
/// defecto), el oficial después (R-GGUF-57).
pub fn ensure_toolchain(busqueda: &Busqueda) -> Result<Toolchain, Error> {
    // ── el conversor: sin sustituto (R-GGUF-55) ────────────────────────
    let (motor, convert) = if let Some(convert) = busqueda.convert_fork.clone() {
        (
            Motor::Fork {
                dir: busqueda.dir_fork.clone().unwrap_or_default(),
            },
            convert,
        )
    } else if let Some(convert) = busqueda.convert_oficial.clone() {
        (
            Motor::Oficial {
                dir: busqueda.dir_oficial.clone().unwrap_or_default(),
            },
            convert,
        )
    } else {
        return Err(Error::SinMotor {
            falta: "convert_hf_to_gguf.py (el conversor de pesos a GGUF)".to_string(),
            donde: format!(
                "se buscó en $LLAMA_DIR ({}) y en $LLAMA_OFFICIAL_DIR ({})",
                describe_dir(busqueda.dir_fork.as_deref(), "~/llama.cpp"),
                describe_dir(busqueda.dir_oficial.as_deref(), "~/llama.cpp-official"),
            ),
        });
    };

    // ── el cuantizador: el PATH primero, luego el build del motor ──────
    let quantize = if let Some(quantize) = busqueda.quantize_en_path.clone() {
        quantize
    } else {
        let del_build = match motor {
            Motor::Fork { .. } => busqueda.quantize_fork.clone(),
            Motor::Oficial { .. } => busqueda.quantize_oficial.clone(),
        };
        match del_build {
            Some(quantize) => quantize,
            None => {
                return Err(Error::MotorIncompleto {
                    falta: "llama-quantize (el cuantizador)".to_string(),
                    donde: format!(
                        "no está en el PATH ni en {}/build/bin",
                        motor.dir().to_string_lossy()
                    ),
                });
            }
        }
    };

    // ── el unidor de shards: útil, no imprescindible ──────────────────
    let split = busqueda
        .split_en_path
        .clone()
        .or_else(|| busqueda.split_en_build.clone());

    Ok(Toolchain {
        motor,
        convert,
        quantize,
        split,
    })
}

/// Describe un directorio para el mensaje: lo que hay, o su default.
fn describe_dir(dir: Option<&Path>, defecto: &str) -> String {
    match dir {
        Some(dir) => dir.to_string_lossy().into_owned(),
        None => defecto.to_string(),
    }
}
