//! La **política de limpieza** (R-GGUF-16/17/18, T-30).
//!
//! Las tres reglas que no se negocian, una por test:
//!
//! - al fallar la cuantización NO se borra nada (R-GGUF-16);
//! - los pesos sólo se borran si la verificación pasó (R-GGUF-17);
//! - el intermedio se borra después de todos los quants, y si es el
//!   resultado no se toca (R-GGUF-18).

use ropero_forge::limpieza::{self, EstadoTirada, Origen};

fn estado() -> EstadoTirada {
    EstadoTirada {
        origen: Origen::ClonPropio,
        verifico: true,
        cuantizo: true,
        intermedio_es_resultado: false,
        intermedio_es_entrada: false,
        keep_source: false,
        keep_intermedio: false,
    }
}

/// La tirada feliz: se borra el clon y el intermedio.
#[test]
fn la_tirada_feliz_borra_el_clon_y_el_intermedio() {
    let ordenes = limpieza::ordenes(&estado());
    assert!(ordenes.borrar_clon);
    assert!(ordenes.borrar_intermedio);
}

/// **R-GGUF-16**: al fallar la cuantización NO se borra nada. Es la
/// mutación que defiende el test: si la fase 3 falla (verifico = false),
/// ni el clon ni el intermedio se tocan.
#[test]
fn al_fallar_la_verificacion_no_se_borra_nada() {
    let mut estado = estado();
    estado.verifico = false;
    let ordenes = limpieza::ordenes(&estado);
    assert!(!ordenes.borrar_clon, "los pesos se quedan (R-GGUF-17)");
    assert!(
        !ordenes.borrar_intermedio,
        "el intermedio se queda: sirve para reintentar sólo la cuantización"
    );
}

/// **R-GGUF-17**: el GGUF de entrada jamás se borra.
#[test]
fn el_gguf_de_entrada_jamas_se_borra() {
    let mut estado = estado();
    estado.intermedio_es_entrada = true;
    let ordenes = limpieza::ordenes(&estado);
    assert!(!ordenes.borrar_intermedio);
}

/// `--keep-source` conserva los pesos aunque todo haya ido bien.
#[test]
fn keep_source_conserva_los_pesos() {
    let mut estado = estado();
    estado.keep_source = true;
    let ordenes = limpieza::ordenes(&estado);
    assert!(!ordenes.borrar_clon);
}

/// `--keep-intermedio` conserva el intermedio.
#[test]
fn keep_intermedio_conserva_el_intermedio() {
    let mut estado = estado();
    estado.keep_intermedio = true;
    let ordenes = limpieza::ordenes(&estado);
    assert!(!ordenes.borrar_intermedio);
}

/// **R-GGUF-18**: si el intermedio **es** el resultado (quant F16), no se
/// toca: es el fichero final.
#[test]
fn el_intermedio_que_es_resultado_no_se_toca() {
    let mut estado = estado();
    estado.intermedio_es_resultado = true;
    let ordenes = limpieza::ordenes(&estado);
    assert!(!ordenes.borrar_intermedio);
}

/// Sin cuantización (la conversión es el resultado), no hay intermedio
/// que borrar: no se inventa nada.
#[test]
fn sin_cuantizacion_no_hay_intermedio_que_borrar() {
    let mut estado = estado();
    estado.cuantizo = false;
    let ordenes = limpieza::ordenes(&estado);
    assert!(!ordenes.borrar_intermedio);
}

/// El `--path` del usuario **no se borra nunca**: lo trajo él. El
/// intermedio, en cambio, lo fabricó la tirada: sí se puede borrar.
#[test]
fn los_pesos_del_usuario_no_se_borran() {
    let mut estado = estado();
    estado.origen = Origen::PesosDelUsuario;
    let ordenes = limpieza::ordenes(&estado);
    assert!(!ordenes.borrar_clon);
    assert!(ordenes.borrar_intermedio);
}
