//! El **sha256** del sidecar (R-GGUF-19).
//!
//! Es el mismo que saca `sha256sum` (y que `hashlib.sha256` de Python): si
//! no, el hash del sidecar no sirve para nada.

use ropero_forge::sha256;

/// Los vectores de prueba del NIST (FIPS 180-4).
#[test]
fn el_sha256_es_el_de_sempre() {
    assert_eq!(
        sha256::sha256(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256::sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256::sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

/// Un fichero de más de un bloque (el relleno importa).
#[test]
fn el_sha256_de_un_fichero_largo() {
    let datos = vec![b'a'; 1000];
    assert_eq!(
        sha256::sha256(&datos),
        "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
    );
}

/// El hash de un fichero que no está: `None` (no se inventa).
#[test]
fn el_sha256_de_un_fichero_que_no_esta() {
    assert_eq!(
        sha256::sha256_de(std::path::Path::new("/no/existe/imatrix.dat")),
        None
    );
}

/// El hash de un fichero es el de sus bytes, se lea como se lea: el
/// fichero se lee **por bloques** (no se copia entero en memoria) y el
/// hash sale igual.
#[test]
fn el_sha256_de_un_fichero_es_el_de_sus_bytes() {
    let datos: Vec<u8> = (0..100_000u32).map(|n| (n % 251) as u8).collect();
    let base = std::env::temp_dir().join(format!("ropero-forge-sha256-{}", std::process::id()));
    std::fs::create_dir_all(&base).expect("crear el directorio");
    let ruta = base.join("imatrix.dat");
    std::fs::write(&ruta, &datos).expect("escribir el fichero");

    assert_eq!(
        sha256::sha256_de(&ruta),
        Some(sha256::sha256(&datos)),
        "el hash del fichero no es el de sus bytes"
    );
    let _ = std::fs::remove_dir_all(&base);
}

/// El hash por partes: alimentar el hash a trozos da el mismo hash que
/// darle el fichero entero de una vez.
#[test]
fn el_sha256_por_partes_es_el_mismo() {
    let datos: Vec<u8> = (0..5_000u32).map(|n| (n % 97) as u8).collect();
    let mut hash = sha256::Sha256::nuevo();
    for trozo in datos.chunks(37) {
        hash.actualiza(trozo);
    }
    assert_eq!(hash.finalize(), sha256::sha256(&datos));
}
