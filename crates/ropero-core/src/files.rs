//! Las claves de fichero de `[files]` y el **predicado único** que las consulta.
//!
//! `[files]` declara cuatro cosas: `gguf`, `mtp`, `mmproj` y `chat_template`.
//! Dos lugares del dominio las consultan: la validación (§4.1.1) y el montaje
//! del argv (`[args.flag.<f>].requires`).
//!
//! Antes cada uno tenía su propio `match` sobre las mismas cuatro claves, y
//! no se parecían: la validación daba error ante una clave desconocida y el
//! argv la aceptaba en silencio. Aquí vive **una sola vez** el vocabulario de
//! `[files]` y el predicado que pregunta si una tabla tiene una clave:
//!
//! - [`FILE_KEYS`]: el vocabulario.
//! - [`is_file_key`]: ¿la clave es del vocabulario?
//! - [`has_file`]: el predicado único sobre cualquier tabla de ficheros.
//!
//! El llamante decide qué hacer con una clave que no es del vocabulario: eso
//! es política de cada sitio, no del predicado.

use std::path::Path;

use crate::model::Files;

/// Las cuatro claves que `[files]` puede declarar.
pub const FILE_KEYS: &[&str] = &["gguf", "mtp", "mmproj", "chat_template"];

/// ¿`key` nombra una clave de `[files]`?
///
/// Es la pregunta del **vocabulario**: no mira ningún fichero, sólo sabe qué
/// nombres son válidos.
pub fn is_file_key(key: &str) -> bool {
    FILE_KEYS.contains(&key)
}

/// Una tabla de ficheros: lo declarado en `[files]` o lo ya resuelto.
///
/// Es lo que permite que el mismo predicado pregunte por lo **declarado**
/// (validación) y por lo **resuelto** (argv).
pub trait FileTable {
    /// El valor de la clave `key`, si la tabla la tiene.
    ///
    /// Una clave que no existe en la tabla —porque no se declaró, o porque
    /// no es una clave de `[files]`— devuelve `None`.
    fn file(&self, key: &str) -> Option<&Path>;
}

impl FileTable for Files {
    fn file(&self, key: &str) -> Option<&Path> {
        match key {
            "gguf" => self.gguf.as_deref(),
            "mtp" => self.mtp.as_deref(),
            "mmproj" => self.mmproj.as_deref(),
            "chat_template" => self.chat_template.as_deref(),
            _ => None,
        }
    }
}

/// ¿La tabla de ficheros `table` tiene la clave `key`?
///
/// **El predicado único** sobre `[files]`. Tanto la validación como el
/// montaje del argv preguntan aquí; ninguno tiene su propio `match`.
///
/// Una clave que **no** es de `[files]` no está en ninguna tabla: la
/// respuesta es `false`. Quien llame y necesite distinguir "no está" de
/// "ni es una clave", usa antes [`is_file_key`].
pub fn has_file<T: FileTable + ?Sized>(table: &T, key: &str) -> bool {
    table.file(key).is_some()
}
