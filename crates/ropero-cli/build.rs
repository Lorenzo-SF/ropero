//! Las constantes de compilación del binario `ropero`.
//!
//! La versión, la fecha de compilación y el autor son **datos de build**:
//! no cambian entre ejecuciones, así que no tiene sentido leerlos del
//! entorno en cada arranque. `build.rs` los fija en el momento de compilar
//! y el binario los lee de las constantes que genera.
//!
//! La fecha de compilación se pide al compilador con `SOURCE_DATE_EPOCH`
//! cuando está definido (builds reproducibles) y, si no, con la fecha del
//! sistema.

use std::time::{SystemTime, UNIX_EPOCH};

/// El autor del programa.
const AUTHOR: &str = "Lorenzo Sánchez Fraile";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-env-changed=ROPERO_BUILD_DATE");

    let version = env!("CARGO_PKG_VERSION");
    println!("cargo:rustc-env=ROPERO_VERSION={version}");
    println!("cargo:rustc-env=ROPERO_BUILD_DATE={}", build_date());
    println!("cargo:rustc-env=ROPERO_AUTHOR={AUTHOR}");
}

/// La fecha de compilación, en formato `YYYY-MM-DD`.
///
/// Se prefiere `SOURCE_DATE_EPOCH` (el estándar de los builds
/// reproducibles) y luego `ROPERO_BUILD_DATE` (para poder fijar la fecha a
/// mano). Sin ninguna de las dos, se usa la fecha del sistema.
fn build_date() -> String {
    if let Ok(epoch) = std::env::var("SOURCE_DATE_EPOCH")
        && let Ok(segundos) = epoch.parse::<u64>()
    {
        return format_date(segundos);
    }
    if let Ok(manual) = std::env::var("ROPERO_BUILD_DATE")
        && !manual.trim().is_empty()
    {
        return manual.trim().to_string();
    }
    let ahora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_date(ahora)
}

/// Formatea un instante (segundos desde epoch) como `YYYY-MM-DD`.
///
/// El cálculo es **proleptic-gregoriano** y directo: los días se cuentan
/// desde 1970-01-01 con el algoritmo de la fecha juliana, que es exacto
/// para las fechas que importan (las de compilación).
fn format_date(segundos: u64) -> String {
    let dias = (segundos / 86_400) as i64;
    let (año, mes, dia) = fecha_gregoriana(dias);
    format!("{año:04}-{mes:02}-{dia:02}")
}

/// La fecha gregoriana (año, mes, día) de un número de días desde epoch.
fn fecha_gregoriana(dias: i64) -> (i64, u32, u32) {
    // El algoritmo de Fliegel-Van Flandern: exacto para todas las fechas
    // prolepticas gregorianas.
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m as u32, d as u32)
}
