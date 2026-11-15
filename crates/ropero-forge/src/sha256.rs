//! El **sha256** de un fichero, sin dependencias.
//!
//! Lo necesita la procedencia (R-GGUF-19): el sidecar dice **qué**
//! importance matrix se usó, y con su `sha256` se sabe **cuál** (el mismo
//! nombre, distinto contenido, es otra matriz). La v1 lo sacaba de
//! `sha256sum`; aquí está implementado, porque la forja no arrastra
//! dependencias.
//!
//! Es el sha256 del NIST (FIPS 180-4): el mismo que `sha256sum` y que
//! `hashlib.sha256` de Python.
//!
//! Se puede hashear **por partes**: un fichero se lee por bloques de 8 KiB
//! y se va alimentando el hash, sin copiarlo entero en memoria (ni duplicar
//! el buffer).

use std::path::Path;

/// Las constantes de relleno (las 32 primeras bits fraccionarias de las
/// raíces cúbicas de los 64 primeros primos).
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// El estado inicial (las 32 primeras bits fraccionarias de las raíces
/// cuadradas de los 8 primeros primos).
const H: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// El sha256 de unos bytes, en hexadecimal (en minúsculas, como
/// `sha256sum`).
pub fn sha256(datos: &[u8]) -> String {
    let mut hash = Sha256::nuevo();
    hash.actualiza(datos);
    hash.finalize()
}

/// El sha256 de un fichero, en hexadecimal.
///
/// `None` si el fichero no se puede leer: no se inventa el hash de algo que
/// no se ha leído.
///
/// El fichero se lee **por bloques** (8 KiB) y se hashea sobre la marcha: no
/// se copia entero en memoria (ni se duplica el buffer).
pub fn sha256_de(ruta: &Path) -> Option<String> {
    use std::io::Read;
    let mut fichero = std::fs::File::open(ruta).ok()?;
    let mut hash = Sha256::nuevo();
    let mut buffer = [0u8; BLOQUE];
    loop {
        let leidos = fichero.read(&mut buffer).ok()?;
        if leidos == 0 {
            break;
        }
        hash.actualiza(&buffer[..leidos]);
    }
    Some(hash.finalize())
}

/// El tamaño del bloque con el que se lee un fichero: 8 KiB.
const BLOQUE: usize = 8 * 1024;

/// El sha256 **por partes** (el del NIST, FIPS 180-4).
///
/// Se puede ir alimentando con trozos: un fichero se lee por bloques y se
/// hashea sobre la marcha, sin copiarlo entero en memoria.
pub struct Sha256 {
    /// Los ocho registros del estado.
    estado: [u32; 8],
    /// El bloque a medias (lo que falta por completar de 64 bytes).
    bloque: [u8; 64],
    /// Lo que hay en el bloque a medias.
    usados: usize,
    /// El largo de todo lo que se ha alimentado, en bytes.
    largo: u64,
}

impl Sha256 {
    /// Un sha256 **vacío** (el del NIST: `e3b0c442…`).
    pub fn nuevo() -> Self {
        Sha256 {
            estado: H,
            bloque: [0u8; 64],
            usados: 0,
            largo: 0,
        }
    }

    /// Le da más datos al hash (en cualquier tamaño: se acumulan).
    pub fn actualiza(&mut self, datos: &[u8]) {
        self.largo = self.largo.wrapping_add(datos.len() as u64);
        let mut resto = datos;
        // Lo que completa el bloque a medias.
        if self.usados > 0 {
            let faltan = 64 - self.usados;
            let cogidos = faltan.min(resto.len());
            self.bloque[self.usados..self.usados + cogidos].copy_from_slice(&resto[..cogidos]);
            self.usados += cogidos;
            resto = &resto[cogidos..];
            if self.usados == 64 {
                let bloque = self.bloque;
                self.comprime(&bloque);
                self.usados = 0;
            }
        }
        // Los bloques que van enteros.
        while resto.len() >= 64 {
            let (bloque, mas) = resto.split_at(64);
            let mut entero = [0u8; 64];
            entero.copy_from_slice(bloque);
            self.comprime(&entero);
            resto = mas;
        }
        // Y lo que queda, para el próximo trozo.
        if !resto.is_empty() {
            self.bloque[..resto.len()].copy_from_slice(resto);
            self.usados = resto.len();
        }
    }

    /// El hash, en hexadecimal (en minúsculas, como `sha256sum`).
    pub fn finalize(mut self) -> String {
        // El relleno: un 1 bit, los ceros que falten y el largo en bits (8
        // bytes, big-endian) al final del último bloque.
        let bits = self.largo.wrapping_mul(8);
        self.bloque[self.usados] = 0x80;
        self.usados += 1;
        if self.usados > 56 {
            // El relleno no cabe en este bloque: se completa y se empieza
            // otro.
            for byte in self.bloque.iter_mut().skip(self.usados) {
                *byte = 0;
            }
            let bloque = self.bloque;
            self.comprime(&bloque);
            self.usados = 0;
        }
        for byte in self.bloque.iter_mut().take(56).skip(self.usados) {
            *byte = 0;
        }
        self.bloque[56..64].copy_from_slice(&bits.to_be_bytes());
        let bloque = self.bloque;
        self.comprime(&bloque);
        self.estado
            .iter()
            .map(|palabra| format!("{palabra:08x}"))
            .collect()
    }

    /// Comprime un bloque de 64 bytes en el estado (el núcleo del NIST).
    fn comprime(&mut self, bloque: &[u8; 64]) {
        let mut w = [0u32; 64];
        for (i, palabra) in bloque.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([palabra[0], palabra[1], palabra[2], palabra[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = self.estado;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let eleccion = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(eleccion)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let mayoria = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(mayoria);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for (i, palabra) in v.iter().enumerate() {
            self.estado[i] = self.estado[i].wrapping_add(*palabra);
        }
    }
}
