//! El informe de `ropero test`: cómo respondieron y cómo rindieron.
//!
//! El informe se compone en el dominio (no en la CLI) y sale en dos formas:
//! texto para humanos y JSON para máquinas. Reglas que estos tests vigilan:
//!
//! - lo que no se puede medir se dice como **no disponible**, no se calla;
//! - lo que es aproximado se marca como **aproximado**;
//! - un test pendiente de juez se marca como pendiente: nunca se le pone nota;
//! - si ropero lanzó el motor, el informe dice que **queda en pie** y cómo
//!   pararlo;
//! - la salida cruda del modelo va escapada, y el `--json` es JSON válido.

use ropero_bateria::indice::parse_indice;
use ropero_bateria::informe::{Informe, Metrica, RespuestaTest, Tokens};
use ropero_bateria::puntuadores::puntua;

/// El índice real: el informe se compone sobre la batería de verdad.
fn bateria() -> ropero_bateria::indice::Bateria {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/tests/indice.md");
    parse_indice(&std::fs::read_to_string(&ruta).expect("el índice del repo existe"))
        .expect("el índice real se parsea")
}

/// Una respuesta de F2 con métricas de motor.
fn f2() -> RespuestaTest {
    let salida = "391\nUlán Bator\n7\n13\nGabriel García Márquez\n1024\nÁfrica\nNO SÉ";
    RespuestaTest {
        id: "F2".to_string(),
        salida: salida.to_string(),
        metrica: Metrica {
            latencia_total_ms: 1200,
            primer_token_ms: Some(300),
            tokens: Tokens::Uso(120),
        },
        puntuacion: puntua("F2", salida),
    }
}

/// Una respuesta de F1 con métricas aproximadas y sin primer token.
fn f1() -> RespuestaTest {
    let salida = "Problema: la función revienta con campo vacío. Ejemplo: 1.\nProblema: no valida. Ejemplo: 2.\nFin de la revisión.";
    RespuestaTest {
        id: "F1".to_string(),
        salida: salida.to_string(),
        metrica: Metrica {
            latencia_total_ms: 2000,
            primer_token_ms: None,
            tokens: Tokens::Aproximado(20),
        },
        puntuacion: puntua("F1", salida),
    }
}

/// Una respuesta pendiente de juez: su salida cruda se guarda.
fn a1() -> RespuestaTest {
    RespuestaTest {
        id: "A1".to_string(),
        salida: "El vecino del verde vive en la casa 3\"".to_string(),
        metrica: Metrica {
            latencia_total_ms: 900,
            primer_token_ms: Some(120),
            tokens: Tokens::NoDisponible,
        },
        puntuacion: puntua("A1", "El vecino del verde vive en la casa 3\""),
    }
}

/// Un informe con las tres respuestas.
fn informe() -> Informe {
    Informe::nuevo(
        bateria(),
        "modelo-alpha",
        "turbo-llama",
        "low",
        8100,
        true,
        vec![a1(), f1(), f2()],
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// El texto para humanos
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_informe_dice_el_modelo_el_motor_el_nivel_y_el_puerto() {
    let texto = informe().a_texto();
    assert!(texto.contains("modelo-alpha"), "dice el modelo: {texto}");
    assert!(texto.contains("turbo-llama"), "dice el motor: {texto}");
    assert!(texto.contains("low"), "dice el nivel: {texto}");
    assert!(texto.contains("8100"), "dice el puerto: {texto}");
}

#[test]
fn cada_test_sale_con_su_puntuacion_y_su_rendimiento() {
    let texto = informe().a_texto();
    let linea_f2 = texto
        .lines()
        .find(|l| l.contains("F2"))
        .expect("la línea de F2");
    assert!(linea_f2.contains("1.00"), "F2 sacó 1.00: {linea_f2}");
    assert!(linea_f2.contains("1.20 s"), "la latencia total: {linea_f2}");
    assert!(linea_f2.contains("0.30 s"), "el primer token: {linea_f2}");
    assert!(
        linea_f2.contains("120 tokens"),
        "los tokens del motor: {linea_f2}"
    );
    assert!(
        linea_f2.contains("100.0 tokens/s"),
        "la velocidad: {linea_f2}"
    );
}

#[test]
fn la_linea_de_cada_test_lleva_su_titulo_del_indice() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("F2"))
        .expect("la línea de F2");
    assert!(
        linea.contains("8 respuestas en 1 línea"),
        "el título es el del índice: {linea}"
    );
}

#[test]
fn el_primer_token_que_no_se_pudo_medir_se_dice_explicitamente() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("F1"))
        .expect("la línea de F1");
    assert!(
        linea.contains("primer token: no disponible"),
        "no se calla lo que no se midió: {linea}"
    );
}

#[test]
fn los_tokens_aproximados_se_marcan_como_aproximados() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("F1"))
        .expect("la línea de F1");
    assert!(
        linea.contains("≈20 palabras"),
        "el recuento es aproximado: {linea}"
    );
    assert!(
        linea.contains("aproximado"),
        "la palabra aproximado está: {linea}"
    );
}

#[test]
fn los_tokens_del_motor_no_se_marcan_como_aproximados() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("F2"))
        .expect("la línea de F2");
    assert!(
        !linea.contains("aproximado"),
        "los tokens del motor son reales: {linea}"
    );
}

#[test]
fn los_tokens_que_no_se_pudieron_leer_se_dicen_no_disponible() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("A1"))
        .expect("la línea de A1");
    assert!(
        linea.contains("tokens: no disponible"),
        "se dice lo que no hay: {linea}"
    );
}

#[test]
fn un_test_pendiente_se_marca_pendiente_y_no_le_pone_nota() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("A1"))
        .expect("la línea de A1");
    assert!(
        linea.contains("pendiente de juez"),
        "se dice que hace falta juez: {linea}"
    );
    assert!(
        !linea.contains("1.00"),
        "un test pendiente no tiene nota: {linea}"
    );
}

#[test]
fn si_ropero_lanzo_el_motor_lo_dice_y_dice_que_queda_en_pie() {
    let texto = informe().a_texto();
    assert!(
        texto.contains("lanzó"),
        "dice que ropero lanzó el motor: {texto}"
    );
    assert!(
        texto.contains("queda en pie"),
        "dice que queda en pie: {texto}"
    );
    assert!(
        texto.contains("ropero stop modelo-alpha"),
        "dice cómo pararlo: {texto}"
    );
}

#[test]
fn si_el_motor_ya_estaba_en_pie_lo_dice() {
    let mut informe = informe();
    informe.lanzado = false;
    let texto = informe.a_texto();
    assert!(
        texto.contains("ya estaba en pie"),
        "dice que se reutilizó el motor: {texto}"
    );
    assert!(!texto.contains("lanzó"), "no dice que lo lanzó: {texto}");
}

#[test]
fn la_nota_por_grupo_es_ponderada_y_se_marca_parcial() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("Grupo F"))
        .expect("la línea del grupo F");
    assert!(
        linea.contains("1.00"),
        "el grupo F es mecánico y saca 1.00: {linea}"
    );
    let linea_a = texto
        .lines()
        .find(|l| l.contains("Grupo A"))
        .expect("la línea del grupo A");
    assert!(
        linea_a.contains("parcial"),
        "el grupo A quedó pendiente: {linea_a}"
    );
}

#[test]
fn el_perfil_de_dimensiones_aparece_en_el_informe() {
    let texto = informe().a_texto();
    let linea = texto
        .lines()
        .find(|l| l.contains("Control de instrucciones"))
        .expect("la línea de la dimensión");
    assert!(linea.contains("100"), "F1 y F2 puntuaron al 100: {linea}");
}

#[test]
fn el_informe_texto_no_lleva_codigos_de_control() {
    let texto = informe().a_texto();
    assert!(
        !texto.contains('\u{1b}'),
        "la CLI pone el color, el informe no"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El JSON para máquinas
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_informe_json_es_json_valido() {
    let json = ropero_bateria::json::parse(&informe().a_json()).expect("el informe --json es JSON");
    assert_eq!(
        json.get("modelo").expect("modelo").as_str(),
        Some("modelo-alpha")
    );
    assert_eq!(
        json.get("motor").expect("motor").as_str(),
        Some("turbo-llama")
    );
    assert_eq!(json.get("puerto").expect("puerto").as_num(), Some(8100.0));
    assert_eq!(
        json.get("lanzado_por_ropero").expect("lanzado").as_bool(),
        Some(true)
    );
}

#[test]
fn el_informe_json_escapa_la_salida_cruda_de_la_respuesta() {
    let json = ropero_bateria::json::parse(&informe().a_json()).expect("JSON");
    let ro = json.get("tests").expect("tests").arr().expect("array");
    let a1 = ro
        .iter()
        .find(|t| t.get("id").and_then(|i| i.as_str()) == Some("A1"))
        .expect("A1");
    assert_eq!(
        a1.get("salida").expect("salida").as_str(),
        Some("El vecino del verde vive en la casa 3\""),
        "la salida cruda se lee igual que se escribió"
    );
}

#[test]
fn el_informe_json_lleva_las_metricas_de_cada_test() {
    let json = ropero_bateria::json::parse(&informe().a_json()).expect("JSON");
    let ro = json.get("tests").expect("tests").arr().expect("array");
    let f2 = ro
        .iter()
        .find(|t| t.get("id").and_then(|i| i.as_str()) == Some("F2"))
        .expect("F2");
    let metrica = f2.get("metrica").expect("metrica");
    assert_eq!(
        metrica.get("latencia_ms").expect("latencia").as_num(),
        Some(1200.0)
    );
    assert_eq!(
        metrica
            .get("primer_token_ms")
            .expect("primer token")
            .as_num(),
        Some(300.0)
    );
    assert_eq!(metrica.get("tokens").expect("tokens").as_num(), Some(120.0));
    assert_eq!(
        metrica
            .get("tokens_aproximados")
            .expect("aproximado")
            .as_bool(),
        Some(false)
    );
    let f1 = ro
        .iter()
        .find(|t| t.get("id").and_then(|i| i.as_str()) == Some("F1"))
        .expect("F1");
    let metrica = f1.get("metrica").expect("metrica");
    assert_eq!(
        metrica
            .get("primer_token_ms")
            .expect("primer token")
            .as_null(),
        Some(())
    );
    assert_eq!(
        metrica
            .get("tokens_aproximados")
            .expect("aproximado")
            .as_bool(),
        Some(true)
    );
}

#[test]
fn el_informe_json_marca_el_test_pendiente_de_juez() {
    let json = ropero_bateria::json::parse(&informe().a_json()).expect("JSON");
    let ro = json.get("tests").expect("tests").arr().expect("array");
    let a1 = ro
        .iter()
        .find(|t| t.get("id").and_then(|i| i.as_str()) == Some("A1"))
        .expect("A1");
    let puntuacion = a1.get("puntuacion").expect("puntuacion");
    assert_eq!(
        puntuacion.get("tipo").expect("tipo").as_str(),
        Some("pendiente")
    );
    assert_eq!(puntuacion.get("score"), None, "un pendiente no tiene score");
}

#[test]
fn el_informe_json_lleva_el_grupo_y_el_perfil() {
    let json = ropero_bateria::json::parse(&informe().a_json()).expect("JSON");
    let grupos = json.get("grupos").expect("grupos").arr().expect("array");
    assert_eq!(grupos.len(), 6, "los seis grupos de la batería");
    let f = grupos
        .iter()
        .find(|g| g.get("grupo").and_then(|g| g.as_str()) == Some("F"))
        .expect("grupo F");
    assert_eq!(f.get("score").expect("score").as_num(), Some(1.0));
    assert_eq!(f.get("parcial").expect("parcial").as_bool(), Some(false));
    let a = grupos
        .iter()
        .find(|g| g.get("grupo").and_then(|g| g.as_str()) == Some("A"))
        .expect("grupo A");
    assert_eq!(
        a.get("score").and_then(|v| v.as_null()),
        Some(()),
        "el grupo A no tiene nota"
    );
    let perfil = json.get("perfil").expect("perfil").arr().expect("array");
    assert_eq!(perfil.len(), 12, "las doce dimensiones");
}

#[test]
fn el_resultado_de_un_test_se_guarda_como_json_valido() {
    let texto = informe().respuestas[0].a_json();
    let json = ropero_bateria::json::parse(&texto).expect("el resultado de un test es JSON");
    assert_eq!(json.get("id").expect("id").as_str(), Some("A1"));
    assert_eq!(
        json.get("salida").expect("salida").as_str(),
        Some("El vecino del verde vive en la casa 3\"")
    );
    assert!(json.get("metrica").is_some(), "lleva las métricas");
    assert!(json.get("puntuacion").is_some(), "lleva la puntuación");
}
