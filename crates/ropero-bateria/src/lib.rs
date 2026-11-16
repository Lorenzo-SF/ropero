//! El dominio de la batería de pruebas de ropero.
//!
//! `ropero-bateria` es **dominio puro**: recibe el índice como texto, recibe
//! la salida del modelo como texto, y devuelve puntuaciones, agregados e
//! informe. No lee disco, no consulta el entorno, no lanza procesos y no
//! habla con la red: la E/S vive en `ropero-runtime`.
//!
//! Lo que aquí se hace, y lo que **no**:
//!
//! - `indice` lee la rúbrica del índice y **valida**: un índice que no cuadra
//!   dice la línea y el valor recibido; ningún peso se inventa.
//! - `seleccion` resuelve `--todos / --grupo / --test / --dimension` contra
//!   la batería; lo que no existe se rechaza con el valor recibido.
//! - `puntuadores` comprueban sólo lo que un programa puede comprobar solo
//!   (F2, F1, E1, D1, D2, D3, G1–G4). Los diez tests que necesitan un juez quedan
//!   **pendientes**: su salida se guarda y no se le pone nota.
//! - `json` escapa, construye y lee JSON a mano, sin dependencias: el cuerpo
//!   del `POST`, el resultado guardado y la salida `--json`.
//! - `informe` dice cómo respondieron y cómo rindieron, y dice en voz alta lo
//!   que no se pudo medir.
//!
//! Cero dependencias: el crate no necesita `serde`, `serde_json` ni `regex`.

pub mod indice;
pub mod informe;
pub mod json;
pub mod puntuadores;
pub mod seleccion;

/// Pone en minúsculas y quita los acentos: así se comparan los nombres del
/// índice sin depender de que el usuario escriba los acentos.
///
/// Es una normalización **de comparación**, no de escritura: los nombres se
/// devuelven siempre tal como los declaró el índice.
pub fn normalizar(texto: &str) -> String {
    let mut out = String::with_capacity(texto.len());
    // Primero se pasa a minúsculas y **después** se quitan los acentos: si
    // se quitaran antes, una 'Á' mayúscula se escaparía (no es la misma
    // letra que la 'á' acentuada).
    for c in texto.chars().flat_map(char::to_lowercase) {
        match c {
            'á' | 'à' | 'ä' | 'â' => out.push('a'),
            'é' | 'è' | 'ë' | 'ê' => out.push('e'),
            'í' | 'ì' | 'ï' | 'î' => out.push('i'),
            'ó' | 'ò' | 'ö' | 'ô' => out.push('o'),
            'ú' | 'ù' | 'ü' | 'û' => out.push('u'),
            'ñ' => out.push('n'),
            'ç' => out.push('c'),
            c => out.push(c),
        }
    }
    out
}
