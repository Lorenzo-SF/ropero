//! El **reporte** de `ropero gguffer`: lo que el comando dice.
//!
//! Un reporte son **líneas** (stdout) y, si algo falla, el **error** que lo
//! dice (stderr). No hay más: la CLI imprime las líneas y traduce el error
//! a su código de salida (0 ok, 1 error, 2 uso), pero **no decide** qué
//! líneas son ni qué error es (R-ARQ-1).
//!
//! Hay tres formas de reporte:
//!
//! | Forma | Cuándo |
//! |---|---|
//! | la tirada | `--repo`/`--path`/`--gguf`: las seis fases de verdad |
//! | la consulta | `--list-quants`: qué quants hay, sin fabricar nada |
//! | el diagnóstico | `--diagnostico`: qué motor hay y qué falta |

use crate::Error;

/// Lo que dice un comando de la forja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reporte {
    /// Las líneas del informe (van a stdout).
    pub lineas: Vec<String>,
    /// El error, si lo hay (va a stderr, con su pista).
    pub error: Option<Error>,
}

impl Reporte {
    /// Un reporte que **no** tiene error: sólo líneas.
    pub fn nuevo(lineas: Vec<String>) -> Reporte {
        Reporte {
            lineas,
            error: None,
        }
    }

    /// Un reporte que **sólo** tiene error.
    pub fn error(error: Error) -> Reporte {
        Reporte {
            lineas: Vec::new(),
            error: Some(error),
        }
    }

    /// Un reporte con líneas **y** con error: es el caso del diagnóstico
    /// sin motor (el informe se enseña entero, y además se dice que falta
    /// lo que falta).
    pub fn con_error(lineas: Vec<String>, error: Error) -> Reporte {
        Reporte {
            lineas,
            error: Some(error),
        }
    }
}
