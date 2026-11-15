//! La **verificación mínima** de un GGUF (la fase 4, R-GGUF-15.4).
//!
//! Comprueba que el GGUF **existe**, que **no está vacío**, que tiene un
//! **tamaño mínimo razonable** y que la **cabecera dice GGUF**. Es
//! deliberadamente **mínima**: la lectura completa de la metadata (los
//! KVs, el `general.file_type`, el base_model…) es la tarea T-33 y queda
//! fuera de esta fase — salvo el `general.file_type`, que [`file_type`] sí
//! lee, con la lectura acotada. Lo que esta verificación compra es la
//! decisión de la limpieza (R-GGUF-17): sólo si el GGUF pasa se borran los
//! pesos, y para eso basta saber que el fichero está íntegro por arriba (la
//! cabecera correcta y un tamaño > 0 descartan el truncado a cero bytes,
//! que es el modo de fallo corriente del disco lleno).

use std::io::Read;
use std::path::Path;

/// El tamaño mínimo razonable: la cabecera con sus metadatos mínimos y
/// algo de tensor. Un GGUF de menos de 1 KiB no es un GGUF de modelo.
pub const TAMAÑO_MINIMO: u64 = 1024;

/// La **magic** que lleva la cabecera de todo GGUF.
pub const MAGIC: &[u8; 4] = b"GGUF";

/// Lo máximo que se lee para buscar `general.file_type`: la metadata está al
/// principio, así que un MiB basta y no hace falta abrir el GGUF entero (los
/// GGUF son de decenas de GB).
pub const LECTURA_METADATA: u64 = 1_048_576;

// Los tipos de valor de la metadata, según la tabla de `gguf.h`.
const UINT32: u32 = 6;
const STRING: u32 = 12;
const ARRAY: u32 = 13;

/// El `general.file_type` del GGUF — el `llama_ftype` de su metadata — o
/// `None` si el fichero no lo dice.
///
/// Es lo que permite saber si un GGUF **ya está** en el quant pedido
/// (R-GGUF-7, R-GGUF-8): la clase se lee de la cabecera, no del nombre, que
/// puede decir BF16 y contener un IQ2_M. Se lee sólo el principio del
/// fichero ([`LECTURA_METADATA`]) y con la profundidad acotada: una metadata
/// corrupta o un tamaño desmesurado no hace leer el fichero entero, y no se
/// inventa un valor (devuelve `None` y quien llama cae al nombre).
pub fn file_type(gguf: &Path) -> Option<u32> {
    let datos = lee_hasta(gguf, LECTURA_METADATA)?;
    if datos.len() < 12 || &datos[..4] != MAGIC {
        return None;
    }
    let versión = u32_le(&datos, 4)?;
    if versión == 0 || versión > 3 {
        return None;
    }
    // En la v1 los contadores y las longitudes son de 32 bits; en la v2 los
    // contadores pasan a 64; en la v3 también las longitudes (R-GGUF-7).
    let contadores: usize = if versión == 1 { 4 } else { 8 };
    let longitudes: usize = if versión >= 3 { 8 } else { 4 };

    let mut p: usize = 8;
    p = p.checked_add(contadores)?; // n_tensors
    let n_metadata = leer(&datos, &mut p, contadores)?;

    for _ in 0..n_metadata {
        let longitud = leer(&datos, &mut p, longitudes)?;
        let clave = datos.get(p..p.checked_add(longitud)?)?;
        p = p.checked_add(longitud)?;
        let tipo = u32_le(&datos, p)?;
        p = p.checked_add(4)?;
        if tipo == UINT32 {
            let valor = u32_le(&datos, p)?;
            p = p.checked_add(4)?;
            if clave == b"general.file_type" {
                return Some(valor);
            }
            continue;
        }
        salta_valor(&datos, &mut p, tipo, longitudes, 0)?;
    }
    None
}

/// Salta el valor de un KV. Las arrays se recorren con la **profundidad
/// acotada** (una array de arrays anidada hasta el infinito no puede colgar
/// la lectura).
fn salta_valor(
    datos: &[u8],
    p: &mut usize,
    tipo: u32,
    longitudes: usize,
    profundidad: usize,
) -> Option<()> {
    const PROFUNDIDAD_MAXIMA: usize = 4;
    let fijo = match tipo {
        0 | 1 | 2 | 11 => 1, // unused, int8, uint8, bool
        3 | 4 => 2,          // int16, uint16
        5 | 6 | 9 => 4,      // int32, uint32, float32
        7 | 8 | 10 => 8,     // int64, uint64, float64
        STRING => {
            let n = leer(datos, p, longitudes)?;
            *p = (*p).checked_add(longitudes)?.checked_add(n)?;
            return Some(());
        }
        ARRAY => {
            if profundidad >= PROFUNDIDAD_MAXIMA {
                return None;
            }
            let tipo_de_elemento = u32_le(datos, *p)?;
            *p = (*p).checked_add(4)?;
            let n = leer(datos, p, longitudes)?;
            for _ in 0..n {
                salta_valor(datos, p, tipo_de_elemento, longitudes, profundidad + 1)?;
            }
            return Some(());
        }
        _ => return None,
    };
    *p = (*p).checked_add(fijo)?;
    Some(())
}

/// Un entero (de `ancho` bytes, little-endian) convertido a `usize`.
fn leer(datos: &[u8], p: &mut usize, ancho: usize) -> Option<usize> {
    let valor = if ancho == 8 {
        u64_le(datos, *p)?
    } else {
        u32_le(datos, *p)? as u64
    };
    *p = (*p).checked_add(ancho)?;
    usize::try_from(valor).ok()
}

fn u32_le(datos: &[u8], p: usize) -> Option<u32> {
    let trozo: [u8; 4] = datos.get(p..p.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(trozo))
}

fn u64_le(datos: &[u8], p: usize) -> Option<u64> {
    let trozo: [u8; 8] = datos.get(p..p.checked_add(8)?)?.try_into().ok()?;
    Some(u64::from_le_bytes(trozo))
}

/// Los primeros `máximo` bytes del fichero.
fn lee_hasta(gguf: &Path, máximo: u64) -> Option<Vec<u8>> {
    let fichero = std::fs::File::open(gguf).ok()?;
    let mut datos = Vec::new();
    fichero.take(máximo).read_to_end(&mut datos).ok()?;
    Some(datos)
}

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
