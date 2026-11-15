//! ropero-forge: la **forja** de GGUFs (F6 · Forja).
//!
//! `ropero gguffer` es la **única** implementación de fabricación de GGUFs
//! (R-GGUF-1): la comparten el CLI y la ruta `[build]` de los TOML. La
//! fabricación son **seis fases** en orden fijo (R-GGUF-15):
//!
//! 1. **fuente**: descargar/clonar el repo (o usar el GGUF traído);
//! 2. **conversión**: convertir a GGUF (o cuantizar el GGUF existente);
//! 3. **cuantización**: imatrix → cuantizador, con un log aparte;
//! 4. **verificación** de los GGUFs;
//! 5. **limpieza**;
//! 6. **procedencia**: el sidecar `<out>.fuente.json`.
//!
//! ## Cómo está partido
//!
//! | Módulo | Qué es | Toca el disco |
//! |---|---|---|
//! | [`peticion`] | la petición y sus validaciones duras | no |
//! | [`quants`] | la tabla nombre ↔ número de `llama_ftype` | no |
//! | [`toolchain`] | la precondición del motor (R-GGUF-54…57) | buscar, sí |
//! | [`plan`] | el plan de fabricación y la viabilidad | no |
//! | [`fases`] | la máquina de las seis fases y el checkpoint | no |
//! | [`limpieza`] | la política de limpieza (R-GGUF-16…18) | no |
//! | [`procedencia`] | el sidecar de procedencia (R-GGUF-19) | no |
//! | [`ejecutar`] | la tirada: las seis fases de verdad | sí |
//! | [`verificar`] | la verificación mínima de un GGUF | sí |
//!
//! Lo puro va delante a propósito: el plan, la viabilidad, la política de
//! limpieza y la procedencia se pueden afirmar en un test sin que se baje
//! un byte. La E/S es la parte pequeña, y es la que usan los dobles.
//!
//! ## Las dos reglas que no se negocian
//!
//! - **Al fallar la cuantización NO se borra nada** (R-GGUF-16): el clon y
//!   el GGUF intermedio se quedan, y el error dice cómo reintentar sólo la
//!   cuantización, que es la parte cara.
//! - **Todo GGUF producido deja `<out>.fuente.json`** (R-GGUF-19): un GGUF
//!   sin saber cómo se hizo no se puede publicar.

use std::fmt;

pub mod ejecutar;
pub mod fases;
pub mod limpieza;
pub mod peticion;
pub mod plan;
pub mod procedencia;
pub mod quants;
pub mod toolchain;
pub mod verificar;

pub use ejecutar::{Informe, forja};
pub use fases::{Checkpoint, Fase};
pub use peticion::{Fuente, Imatrix, Intermedio, Peticion};
pub use plan::{Clase, Plan, Viabilidad};
pub use procedencia::{MtpInfo, Procedencia};
pub use toolchain::{Motor, Toolchain};

/// Un aviso de la forja: va a **stderr** (stdout es para los datos).
pub fn avisa(mensaje: &str) {
    crate::err_line(&format!("aviso: {mensaje}"));
}

/// Escribe una línea en stderr, tolerando el pipe roto.
///
/// Es el mismo primitivo del core, duplicado aquí para que la forja no
/// dependa de la capa de presentación más de lo que debe.
fn err_line(texto: &str) {
    use std::io::Write;
    let _ = std::io::stderr().write_all(format!("{texto}\n").as_bytes());
}

/// Los errores de la forja.
///
/// Todos dicen **qué hacer** (P-13), y el código de salida lo decide la
/// CLI: un [`Error::Uso`] es un **2**; el resto, un **1**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Error de uso: la petición está mal escrita.
    ///
    /// La CLI lo traduce a código de salida **2**; los demás errores son
    /// de ejecución y salen con **1**.
    Uso(String),
    /// Un quant pedido no existe en la tabla de `llama_ftype`.
    QuantInvalido {
        /// El nombre que se recibió.
        quant: String,
    },
    /// Falta el motor llama.cpp: la precondición del gguffer (R-GGUF-54).
    SinMotor {
        /// Lo que se buscó y no está.
        falta: String,
        /// Las variables que apuntan a los motores, con su default.
        donde: String,
    },
    /// El motor está, pero le falta una herramienta (el cuantizador, el
    /// unidor de shards).
    MotorIncompleto {
        /// Qué herramienta falta.
        falta: String,
        /// Dónde se buscó.
        donde: String,
    },
    /// La fuente no está (el directorio no existe, el GGUF no está).
    FuenteNoEsta {
        /// Cómo se escribió la fuente.
        fuente: String,
        /// La ruta que no está.
        ruta: String,
    },
    /// Falló la descarga de los pesos.
    Descarga {
        /// El repo que se iba a bajar.
        repo: String,
        /// El código de salida de `hf`, si lo tuvo.
        codigo: Option<i32>,
    },
    /// La cuantización pedida no es viable (R-GGUF-8): los pesos ya vienen
    /// en un quant distinto, y el motor no re-cuantiza.
    NoViable {
        /// Lo que se pidió.
        pedido: String,
        /// Lo que ya traen los pesos.
        tienen: String,
        /// La receta: qué repo usar (el hermano sin cuantizar), si se
        /// sabe.
        sugerencia: Vec<String>,
    },
    /// Falta el imatrix para la cuantización que se va a hacer.
    FaltaImatrix {
        /// Los quants que la necesitan.
        faltan: Vec<String>,
    },
    /// Falló la conversión a GGUF.
    Conversion {
        /// La cola del log del conversor: el último error, tal cual.
        cola: Vec<String>,
        /// Cómo reintentar sólo la conversión (R-GGUF-16).
        reintento: String,
    },
    /// Falló la cuantización (R-GGUF-16: no se borra nada).
    Cuantizacion {
        /// El quant que falló.
        quant: String,
        /// La cola del log del cuantizador (el log aparte de la fase 3).
        cola: Vec<String>,
        /// Cómo reintentar sólo la cuantización.
        reintento: String,
        /// El error de imatrix, **traducido** (R-GGUF-14), si lo fue.
        traduccion: Option<String>,
    },
    /// Un GGUF no superó la verificación.
    Verificacion {
        /// El GGUF que falló.
        gguf: String,
        /// Por qué falló.
        motivo: String,
        /// Dónde quedaron los pesos, que **no se borran** (R-GGUF-17).
        pesos: String,
    },
    /// No se pudo escribir el sidecar de procedencia (R-GGUF-19).
    Procedencia {
        /// Dónde se intentó escribir.
        dest: String,
        /// El detalle del error.
        detalle: String,
    },
    /// Un error de disco o de entorno.
    Disco {
        /// Qué se estaba haciendo.
        que: String,
        /// El detalle del error.
        detalle: String,
    },
}

impl Error {
    /// El **código de salida** que le corresponde (0 ok, 1 error, 2 uso).
    pub fn codigo(&self) -> i32 {
        match self {
            Error::Uso(_) => 2,
            _ => 1,
        }
    }

    /// La **pista**: qué hacer para arreglarlo (P-13).
    ///
    /// Va detrás del mensaje, en stderr.
    pub fn pista(&self) -> Option<String> {
        match self {
            Error::QuantInvalido { .. } => {
                Some("usa 'ropero gguffer --list-quants' para ver la lista de quants".to_string())
            }
            Error::SinMotor { .. } => Some(
                "instala y compila un motor (ropero install engines), o exporta \
                 LLAMA_DIR=<tu llama.cpp>"
                    .to_string(),
            ),
            Error::MotorIncompleto { .. } => {
                Some("compila el motor (cmake -B build && cmake --build build)".to_string())
            }
            Error::NoViable { sugerencia, .. } => Some(if sugerencia.is_empty() {
                "busca un repo hermano del mismo modelo en BF16 o F16 (sin cuantizar)".to_string()
            } else {
                format!("usa el repo SIN cuantizar: {}", sugerencia.join(", "))
            }),
            Error::FaltaImatrix { .. } => Some(
                "para cuantizar hace falta la importance matrix del modelo base:\n  \
                 --imatrix FICHERO                            (el fichero, aquí)\n  \
                 --imatrix-repo REPO --imatrix-file FICHERO   (a bajarlo de ahí)"
                    .to_string(),
            ),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Uso(mensaje) => write!(f, "{mensaje}"),
            Error::QuantInvalido { quant } => {
                write!(f, "quant no válido: {quant}")
            }
            Error::SinMotor { falta, donde } => write!(
                f,
                "no hay motor llama.cpp disponible: falta {falta}\n  {donde}"
            ),
            Error::MotorIncompleto { falta, donde } => {
                write!(f, "al motor le falta una herramienta: {falta}\n  {donde}")
            }
            Error::FuenteNoEsta { fuente, ruta } => {
                write!(f, "{fuente}: no está ({ruta})")
            }
            Error::Descarga { repo, codigo } => match codigo {
                Some(codigo) => write!(f, "falló la descarga de {repo} (hf salió con {codigo})"),
                None => write!(f, "falló la descarga de {repo}"),
            },
            Error::NoViable { pedido, tienen, .. } => write!(
                f,
                "los pesos ya vienen en {tienen} y pides {pedido}: \
                 el motor no puede re-cuantizar \
                 (re-cuantizar es dequantizar y re-cuantizar: se pierde calidad sin ganar nada)"
            ),
            Error::FaltaImatrix { faltan } => write!(
                f,
                "vas a cuantizar a {} y no hay imatrix",
                faltan.join(", ")
            ),
            Error::Conversion { .. } => write!(f, "falló la conversión a GGUF"),
            Error::Cuantizacion { quant, .. } => write!(f, "falló la cuantización a {quant}"),
            Error::Verificacion { gguf, motivo, .. } => {
                write!(f, "el GGUF no supera la verificación: {gguf}\n  {motivo}")
            }
            Error::Procedencia { dest, detalle } => {
                write!(f, "no se pudo escribir la procedencia ({dest}): {detalle}")
            }
            Error::Disco { que, detalle } => write!(f, "no se pudo {que}: {detalle}"),
        }
    }
}

impl std::error::Error for Error {}
