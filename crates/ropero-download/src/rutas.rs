//! Las rutas del catálogo, **acotadas** al directorio de destino.
//!
//! ## Por qué hace falta
//!
//! Los valores del TOML (`[download].file`, `[download].name`, los
//! `[download.aux.*]` y `[model].path`) se unen con el directorio de
//! destino con `Path::join`, y ese `join` es **peligroso**:
//!
//! - una ruta **absoluta** **absorbe** el base (`join("/etc/x")` es
//!   `/etc/x`, no `base/etc/x`);
//! - un `..` **sube** (`join("../../../secreto.txt")` sale del base).
//!
//! Con eso, un `[download].file = "../../../secreto.txt"` hace que el
//! `mv` de R-MOD-16.2 se lleve un fichero de **fuera** del directorio de
//! modelos, y un `[download.aux.*].name` fuera **sobrescribe** el fichero
//! de fuera.
//!
//! ## Qué hace este módulo
//!
//! [`ruta_dentro`] **comprueba** la ruta **antes** de unirla: rechaza la
//! raíz absoluta, rechaza los `..` y comprueba que la ruta normalizada
//! empieza por el `base`. El error **nombra el campo y el valor** y dice
//! **qué hacer** (P-13).

use std::path::{Component, Path, PathBuf};

use crate::Error;

/// Une `ruta` a `base` **comprobando** que no se sale de `base`.
///
/// - Una ruta **absoluta** se rechaza: absorbería el `base`.
/// - Un `..` se rechaza: subiría.
/// - La ruta normalizada tiene que **empezar por** `base`.
///
/// La normalización es ** léxica** (sin tocar el disco): se quitan los `.`
/// y se rechazan los `..`.
pub fn ruta_dentro(base: &Path, ruta: &str, campo: &str, modelo: &str) -> Result<PathBuf, Error> {
    let camino = Path::new(ruta);
    let mut normalizada = PathBuf::new();
    for componente in camino.components() {
        match componente {
            // El `.` no cambia nada.
            Component::CurDir => {}
            // El `..` sube: no se permite.
            Component::ParentDir => return Err(ruta_fuera(campo, modelo, ruta)),
            // Un nombre normal: se acumula.
            Component::Normal(parte) => normalizada.push(parte),
            // La raíz (o un prefijo de Windows) absorbe el base.
            Component::RootDir | Component::Prefix(_) => {
                return Err(ruta_fuera(campo, modelo, ruta));
            }
        }
    }
    let dentro = base.join(&normalizada);
    if !dentro.starts_with(base) {
        return Err(ruta_fuera(campo, modelo, ruta));
    }
    Ok(dentro)
}

/// El error de una ruta que se sale del directorio de destino.
fn ruta_fuera(campo: &str, modelo: &str, valor: &str) -> Error {
    Error::RutaFuera {
        modelo: modelo.to_string(),
        campo: campo.to_string(),
        valor: valor.to_string(),
    }
}
