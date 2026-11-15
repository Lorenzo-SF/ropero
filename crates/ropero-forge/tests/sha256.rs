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
