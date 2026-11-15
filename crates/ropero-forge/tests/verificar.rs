//! La lectura de la **cabecera** de un GGUF (hallazgo 2, R-GGUF-7).
//!
//! `general.file_type` es el `llama_ftype` de los pesos: es lo que dice si un
//! GGUF ya está cuantizado, y por eso la clase se lee **de la cabecera** y no
//! del nombre (`modelo-BF16.gguf` con cabecera IQ2_M es un IQ2_M).

use std::fs;
use std::path::{Path, PathBuf};

use ropero_forge::verificar;

static CONTADOR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Un GGUF de fixture en un directorio limpio: cabecera + sus KVs.
fn gguf(partes: &[&[u8]]) -> PathBuf {
    let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base =
        std::env::temp_dir().join(format!("ropero-forge-verificar-{}-{n}", std::process::id(),));
    let ruta = base.join("modelo.gguf");
    fs::create_dir_all(&base).expect("crear el fixture");
    let mut datos = Vec::new();
    for parte in partes {
        datos.extend_from_slice(parte);
    }
    fs::write(&ruta, &datos).expect("escribir el GGUF");
    ruta
}

fn clave(nombre: &str) -> Vec<u8> {
    let mut v = (nombre.len() as u64).to_le_bytes().to_vec();
    v.extend_from_slice(nombre.as_bytes());
    v
}

/// El `general.file_type` (un uint32, tipo 6) se lee de la metadata.
#[test]
fn el_file_type_se_lee_de_la_metadata() {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&3u32.to_le_bytes()); // versión
    datos.extend_from_slice(&0u64.to_le_bytes()); // n_tensors
    datos.extend_from_slice(&1u64.to_le_bytes()); // n_metadata
    datos.extend_from_slice(&clave("general.file_type"));
    datos.extend_from_slice(&6u32.to_le_bytes()); // tipo: uint32
    datos.extend_from_slice(&15u32.to_le_bytes()); // Q4_K_M
    datos.extend_from_slice(&vec![0u8; 2048]);
    let ruta = gguf(&[&datos]);
    assert_eq!(verificar::file_type(&ruta), Some(15));
    let _ = fs::remove_dir_all(ruta.parent().unwrap());
}

/// Un GGUF **sin metadata** no puede dar su clase: hay que fiarse del nombre.
#[test]
fn un_gguf_sin_metadata_no_da_file_type() {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&3u32.to_le_bytes());
    datos.extend_from_slice(&0u64.to_le_bytes());
    datos.extend_from_slice(&0u64.to_le_bytes());
    datos.extend_from_slice(&vec![0u8; 2048]);
    let ruta = gguf(&[&datos]);
    assert_eq!(verificar::file_type(&ruta), None, "inventa una metadata");
    let _ = fs::remove_dir_all(ruta.parent().unwrap());
}

/// Un fichero que no es GGUF, o que no está, no da clase (no panica).
#[test]
fn lo_que_no_es_gguf_no_da_file_type() {
    let ruta = gguf(&[b"NOTGGUF", &vec![0u8; 2048]]);
    assert_eq!(verificar::file_type(&ruta), None);
    assert_eq!(
        verificar::file_type(Path::new("/no/existe/modelo.gguf")),
        None
    );
    let _ = fs::remove_dir_all(ruta.parent().unwrap());
}

/// Una **array** antes del `general.file_type` no rompe la lectura, y una
/// array larga no se lee entera: la lectura de la metadata está acotada.
#[test]
fn las_arrays_no_rompen_la_lectura() {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&3u32.to_le_bytes());
    datos.extend_from_slice(&0u64.to_le_bytes());
    datos.extend_from_slice(&2u64.to_le_bytes()); // dos KVs
    // El primero: una array de uint32 con tres valores.
    datos.extend_from_slice(&clave("general.arr"));
    datos.extend_from_slice(&13u32.to_le_bytes()); // tipo: array
    datos.extend_from_slice(&6u32.to_le_bytes()); // tipo de los elementos
    datos.extend_from_slice(&3u64.to_le_bytes()); // número de elementos
    for i in 0..3u32 {
        datos.extend_from_slice(&i.to_le_bytes());
    }
    datos.extend_from_slice(&clave("general.file_type"));
    datos.extend_from_slice(&6u32.to_le_bytes());
    datos.extend_from_slice(&39u32.to_le_bytes()); // NVFP4
    datos.extend_from_slice(&vec![0u8; 2048]);
    let ruta = gguf(&[&datos]);
    assert_eq!(verificar::file_type(&ruta), Some(39), "se perdió el KV");
    let _ = fs::remove_dir_all(ruta.parent().unwrap());

    // Una array con un tamaño desmesurado: no se lee el fichero entero, y no
    // se inventa un valor (la metadata se acaba antes de llegar al KV).
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&3u32.to_le_bytes());
    datos.extend_from_slice(&0u64.to_le_bytes());
    datos.extend_from_slice(&1u64.to_le_bytes());
    datos.extend_from_slice(&clave("general.arr"));
    datos.extend_from_slice(&13u32.to_le_bytes());
    datos.extend_from_slice(&6u32.to_le_bytes());
    datos.extend_from_slice(&60_000_000u64.to_le_bytes());
    datos.extend_from_slice(&vec![0u8; 2048]);
    let ruta = gguf(&[&datos]);
    assert_eq!(verificar::file_type(&ruta), None, "se inventa un file_type");
    let _ = fs::remove_dir_all(ruta.parent().unwrap());
}

/// La versión 1 de la cabecera (contadores de 32 bits) también se lee.
#[test]
fn la_versión_una_también_se_lee() {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&1u32.to_le_bytes()); // versión
    datos.extend_from_slice(&0u32.to_le_bytes()); // n_tensors
    datos.extend_from_slice(&1u32.to_le_bytes()); // n_metadata
    datos.extend_from_slice(&(17u32.to_le_bytes())); // longitud de la clave
    datos.extend_from_slice(b"general.file_type");
    datos.extend_from_slice(&6u32.to_le_bytes());
    datos.extend_from_slice(&1u32.to_le_bytes()); // F16
    datos.extend_from_slice(&vec![0u8; 2048]);
    let ruta = gguf(&[&datos]);
    assert_eq!(verificar::file_type(&ruta), Some(1));
    let _ = fs::remove_dir_all(ruta.parent().unwrap());
}

/// Lo que la verificación mínima sigue comprando: que el GGUF existe y su
/// cabecera es la magic (la fase 4, R-GGUF-15.4).
#[test]
fn la_verificación_mínima_sigue_verificando() {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&vec![0u8; 2048]);
    let ruta = gguf(&[&datos]);
    assert!(verificar::verificar(&ruta).is_ok(), "un GGUF de 2 KiB pasa");
    let _ = fs::remove_dir_all(ruta.parent().unwrap());
}
