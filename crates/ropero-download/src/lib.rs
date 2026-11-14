//! ropero-download: la **descarga real** de los modelos con `hf`.
//!
//! ## Qué es este crate
//!
//! Es la capa de E/S de las descargas (doc B §3.2.3): el único downloader
//! es **`hf` (HuggingFace CLI)**, no hay `curl` de reserva (R-MOD-14) y no
//! hay verificación por hash ni por checksum (R-MOD-20).
//!
//! ## Cómo está partido
//!
//! | Módulo | Qué es | Toca el disco |
//! |---|---|---|
//! | [`plan`] | el **plan**: qué se baja, a dónde y con qué argumentos | no |
//! | [`download`] | la **ejecución**: `hf`, el staging y la verificación | sí |
//!
//! El plan es **puro** a propósito: los argumentos exactos de `hf` y las
//! rutas se calculan sin tocar nada, así que se pueden comparar en un test
//! (la puerta de T-26) sin descargar un byte.
//!
//! ## El contrato
//!
//! La secuencia de una descarga de GGUF es la de **R-MOD-16**, y es
//! **exactamente**:
//!
//! 1. si el nombre final existe y no está vacío → «ya descargado»;
//! 2. si no existe el final pero existe **con el nombre del repo** → `mv`
//!    y se da por buena;
//! 3. **`hf` ANTES de preguntar** —«prometer una descarga y luego confesar
//!    que no se puede es la peor secuencia posible»—;
//! 4. la confirmación, con el tamaño;
//! 5. `hf download` al staging;
//! 6. si `hf` dice OK pero el fichero no aparece, se busca por `basename`;
//! 7. el rename, comparando **rutas absolutas**;
//! 8. `rm -rf` del staging.
//!
//! Y el GGUF usa **staging propio** (R-MOD-15: `mktemp -d` **dentro** del
//! destino, para que el `mv` sea atómico), mientras que el checkpoint
//! **no** (R-MOD-17: `hf` es reanudable con `--local-dir` fijo, y un
//! staging mataría la reanudación).
//!
//! La verificación es **por contenido real** (R-MOD-18) y se **re-verifica**
//! aunque `hf` salga con 0 (R-MOD-19): *un exit 0 de un downloader no es un
//! modelo instalado*.

use std::fmt;

pub mod download;
pub mod plan;
pub mod rutas;

pub use download::{
    Modo, Resultado, descarga, descarga_con, linea, linea_error, pregunta_de_descarga,
};
pub use plan::{AuxPlan, Formato, Plan, plan_de_descarga};
pub use rutas::ruta_dentro;

/// Los errores de la descarga.
///
/// Todos dicen **qué hacer** (P-13): qué falta, o cómo se arregla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// El modelo no declara `[download]`: no hay nada que bajar.
    SinDescarga {
        /// El modelo.
        modelo: String,
    },
    /// El formato del modelo no se baja con `hf`.
    FormatoNoDescargable {
        /// El modelo.
        modelo: String,
        /// El formato que tiene.
        formato: String,
    },
    /// Al modelo le faltan datos en su TOML para poder bajarlo.
    DatosIncompletos {
        /// El modelo.
        modelo: String,
        /// Lo que falta.
        falta: String,
    },
    /// No está `hf` en el `PATH`.
    FaltaHf {
        /// El modelo que se iba a bajar.
        modelo: String,
    },
    /// `hf` ha fallado.
    HfFalló {
        /// El modelo que se iba a bajar.
        modelo: String,
        /// El código de salida de `hf` (si lo tuvo).
        codigo: Option<i32>,
    },
    /// `hf` ha salido con 0 pero el fichero no está.
    HfSinFichero {
        /// El modelo que se iba a bajar.
        modelo: String,
        /// El fichero que se esperaba.
        file: String,
    },
    /// Una ruta del catálogo **se sale** del directorio de destino.
    ///
    /// Es un defecto del catálogo: un `..` sube y una ruta absoluta
    /// absorbe el base (ver [`crate::rutas`]).
    RutaFuera {
        /// El modelo.
        modelo: String,
        /// El campo del TOML (p. ej. `[download].file`).
        campo: String,
        /// El valor recibido.
        valor: String,
    },
    /// `hf` ha terminado pero el destino está incompleto.
    Incompleto {
        /// El modelo que se iba a bajar.
        modelo: String,
        /// El destino que ha quedado incompleto.
        destino: String,
        /// Lo que falta (una línea por cosa).
        falta: Vec<String>,
    },
    /// Un error de disco.
    Disco {
        /// El modelo que se iba a bajar.
        modelo: String,
        /// Lo que se estaba haciendo.
        que: String,
        /// El detalle del error.
        detalle: String,
    },
}

impl Error {
    /// La **pista**: qué hacer para arreglarlo (P-13).
    ///
    /// Es una línea aparte porque el mensaje de una línea va en stdout (el
    /// informe del modelo) y la pista en stderr.
    pub fn pista(&self) -> Option<String> {
        match self {
            Error::FaltaHf { .. } => Some(
                "instálalo con: uv tool install 'huggingface_hub[cli]'\n  \
                 (o con el instalador de Python de tu sistema)"
                    .to_string(),
            ),
            Error::HfFalló { .. } => {
                Some("hf reanuda solo: vuelve a ejecutar el install y continúa".to_string())
            }
            Error::HfSinFichero { .. } => {
                Some("hf dijo que sí pero no dejó el fichero: revisa el repo".to_string())
            }
            Error::RutaFuera {
                modelo,
                campo,
                valor,
            } => Some(format!(
                "arregla el TOML de '{modelo}': {campo} = \"{valor}\" tiene que estar \
                 dentro del directorio del modelo (sin '..' ni ruta absoluta)"
            )),
            Error::DatosIncompletos { modelo, falta } => {
                Some(format!("arregla el TOML de '{modelo}': le falta {falta}"))
            }
            Error::FormatoNoDescargable { modelo, formato } => Some(format!(
                "'{modelo}' es de formato {formato}: ese formato no lo baja ropero"
            )),
            Error::SinDescarga { modelo } => Some(format!(
                "'{modelo}' no declara [download]: no hay nada que bajar"
            )),
            Error::Incompleto { .. } | Error::Disco { .. } => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::SinDescarga { modelo } => write!(
                f,
                "el modelo '{modelo}' no declara [download]: no hay nada que bajar"
            ),
            Error::FormatoNoDescargable { modelo, formato } => write!(
                f,
                "el modelo '{modelo}' es de formato {formato}: ese formato no se baja con hf"
            ),
            Error::DatosIncompletos { modelo, falta } => write!(
                f,
                "al modelo '{modelo}' le falta {falta} en su TOML: no se puede descargar"
            ),
            Error::FaltaHf { modelo } => write!(
                f,
                "no se puede descargar {modelo}: falta 'hf' (huggingface_hub)"
            ),
            Error::HfFalló { modelo, codigo } => match codigo {
                Some(codigo) => write!(f, "falló la descarga de {modelo} (hf salió con {codigo})"),
                None => write!(f, "falló la descarga de {modelo} (hf)"),
            },
            Error::HfSinFichero { modelo, file } => {
                write!(f, "falló {modelo} (hf dijo OK pero no aparece {file})")
            }
            Error::RutaFuera {
                modelo,
                campo,
                valor,
            } => write!(
                f,
                "{campo} = \"{valor}\" no es válido en el modelo '{modelo}': \
                 la ruta se sale del directorio del modelo"
            ),
            Error::Incompleto { destino, falta, .. } => {
                write!(f, "hf terminó pero {destino} está incompleto")?;
                for linea in falta {
                    write!(f, "\n  {linea}")?;
                }
                Ok(())
            }
            Error::Disco {
                modelo,
                que,
                detalle,
            } => write!(f, "no se pudo {que} de {modelo}: {detalle}"),
        }
    }
}

impl std::error::Error for Error {}
