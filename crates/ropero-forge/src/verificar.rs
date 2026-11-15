//! La **verificación mínima** de un GGUF (la fase 4, R-GGUF-15.4).
//!
//! Comprueba que el GGUF **existe**, que **no está vacío**, que tiene un
//! **tamaño mínimo razonable** y que la **cabecera dice GGUF**. Es
//! deliberadamente **mínima**: la lectura completa de la metadata (los
//! KVs, el `general.file_type`, el base_model…) es la tarea T-33 y queda
//! fuera de esta fase. Lo que esta verificación compra es la decisión de
//! la limpieza (R-GGUF-17): sólo si el GGUF pasa se borran los pesos, y
//! para eso basta saber que el fichero está íntegro por arriba (la
//! cabecera correcta y un tamaño > 0 descartan el truncado a cero bytes,
//! que es el modo de fallo corriente del disco lleno).

use std::path::Path;

/// El tamaño mínimo razonable: la cabecera con sus metadatos mínimos y
/// algo de tensor. Un GGUF de menos de 1 KiB no es un GGUF de modelo.
pub const TAMAÑO_MINIMO: u64 = 1024;

/// La **magic** que lleva la cabecera de todo GGUF.
pub const MAGIC: &[u8; 4] = b"GGUF";

/// Verifica un GGUF: `Ok(())` si pasa, `Err(motivo)` si no.
///
/// El motivo es lo que el error de la fase 4 dice al usuario: por qué no
/// se verificó (y por qué, por tanto, no se borran los pesos).
pub fn verificar(gguf: &Path) -> Result<(), String> {
    let ruta = gguf.to_string_lossy();
    let datos = match std::fs::metadata(gguf) {
        Ok(datos) => datos,
        Err(detalle) => return Err(format!("no está: {detalle}")),
    };
    if !datos.is_file() {
        return Err("no es un fichero".to_string());
    }
    let tamaño = datos.len();
    if tamaño == 0 {
        return Err("está vacío (0 bytes): la conversión o la cuantización \
                    lo dejó a medias"
            .to_string());
    }
    if tamaño < TAMAÑO_MINIMO {
        return Err(format!(
            "es demasiado pequeño ({tamaño} bytes): un GGUF de modelo no \
             baja de {TAMAÑO_MINIMO}"
        ));
    }
    let cabecera = match std::fs::File::open(gguf) {
        Ok(mut fichero) => {
            let mut cabecera = [0u8; 4];
            use std::io::Read;
            match fichero.read_exact(&mut cabecera) {
                Ok(()) => cabecera,
                Err(detalle) => return Err(format!("no se pudo leer la cabecera: {detalle}")),
            }
        }
        Err(detalle) => return Err(format!("no se pudo abrir: {detalle}")),
    };
    if &cabecera != MAGIC {
        return Err(format!(
            "la cabecera no es GGUF (dice {}): el fichero no es un GGUF",
            String::from_utf8_lossy(&cabecera)
        ));
    }
    let _ = ruta;
    Ok(())
}
