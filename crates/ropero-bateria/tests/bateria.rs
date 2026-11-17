//! Lo que la batería hace cuando el índice o la salida no son lo esperado.
//!
//! Estos tests son **míos** (los del otro agente cubren el camino feliz y
//! los errores del índice): vigilan lasnormalizaciones, la escala del Brier
//! y el informe.

use ropero_bateria::indice::parse_indice;
use ropero_bateria::informe::{Informe, Metrica, RespuestaTest, Tokens};
use ropero_bateria::puntuadores::{Puntuacion, es_mecanico, perfil, puntua};
use ropero_bateria::seleccion::{Opciones, Seleccion, seleccion_de, selecciona};

/// El índice real del repo.
fn bateria() -> ropero_bateria::indice::Bateria {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/tests/indice.md");
    parse_indice(&std::fs::read_to_string(&ruta).expect("el índice del repo existe"))
        .expect("el índice real se parsea")
}

// ─────────────────────────────────────────────────────────────────────────────
// La normalización: sin acentos y sin mayúsculas
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn normalizar_quita_el_acento_de_las_mayusculas_tambien() {
    // 'Á' en mayúscula es otra letra que 'á': si se normaliza antes de
    // pasar a minúsculas, la 'Á' se escapa.
    assert_eq!(ropero_bateria::normalizar("África"), "africa");
    assert_eq!(
        ropero_bateria::normalizar("García Márquez"),
        "garcia marquez"
    );
    assert_eq!(ropero_bateria::normalizar("NO SÉ"), "no se");
    assert_eq!(ropero_bateria::normalizar("ñ"), "n");
}

// ─────────────────────────────────────────────────────────────────────────────
// La selección
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn un_grupo_que_no_es_una_letra_se_rechaza_al_resolver_con_su_valor() {
    let seleccion = seleccion_de(Opciones {
        grupo: Some("AB".to_string()),
        ..Opciones::default()
    })
    .expect("el valor se acepta al parsear");
    assert_eq!(seleccion, Seleccion::GrupoIlegible("AB".to_string()));
    let error = selecciona(&bateria(), &seleccion).expect_err("'AB' no es un grupo");
    assert!(error.mensaje.contains("AB"), "nombra el valor: {error}");
    assert!(
        error.mensaje.contains("una sola letra"),
        "dice qué es un grupo: {error}"
    );
}

#[test]
fn un_grupo_en_minusculas_es_el_grupo_en_mayusculas() {
    let seleccion = seleccion_de(Opciones {
        grupo: Some("d".to_string()),
        ..Opciones::default()
    })
    .expect("d es el grupo D");
    assert_eq!(seleccion, Seleccion::Grupo('D'));
}

#[test]
fn un_test_se_elige_por_su_fichero_con_o_sin_ruta() {
    for valor in ["test-calibracion.md", "TEST-CALIBRACION.MD"] {
        let seleccion = seleccion_de(Opciones {
            test: Some(valor.to_string()),
            ..Opciones::default()
        })
        .expect("el fichero es un test");
        assert_eq!(
            selecciona(&bateria(), &seleccion).expect("el test existe"),
            ["D2"]
        );
    }
}

#[test]
fn todos_son_los_veinte_en_el_orden_del_indice() {
    let ids = selecciona(&bateria(), &Seleccion::Todos).expect("todos");
    assert_eq!(ids.len(), 20);
    assert_eq!(ids.first().map(String::as_str), Some("A1"));
    assert_eq!(ids.last().map(String::as_str), Some("G4"));
}

// ─────────────────────────────────────────────────────────────────────────────
// La escala del Brier (D2)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn el_brier_de_la_escala_va_de_cero_a_uno() {
    // B = 0 (confianza perfecta) → 1.0; B = 1 (confianza máxima en todo lo
    // errado) → 0.0.
    let perfecto = "1. Canberra, 100\n2. 476, 100\n3. 206, 100\n4. 74, 100\n5. Enum.reject/2, 100\n6. Yuri Gagarin, 100";
    let Puntuacion::Mecanica { score, detalle } = puntua("D2", perfecto) else {
        panic!("D2 con seis respuestas es mecánica");
    };
    assert_eq!(score, 1.0, "B=0: {detalle}");

    let errado = "1. Sídney, 100\n2. 1453, 100\n3. 204, 100\n4. 76, 100\n5. Enum.filter/2, 100\n6. Neil Armstrong, 100";
    let Puntuacion::Mecanica { score, detalle } = puntua("D2", errado) else {
        panic!("D2 con seis respuestas es mecánica");
    };
    assert_eq!(score, 0.0, "B=1: {detalle}");
}

#[test]
fn la_confianza_de_la_sexta_pregunta_se_lee_como_esta_en_el_test() {
    // El formato de la confianza lo fija el fichero del test: la respuesta
    // y la confianza van separadas por una coma.
    let salida = "1. Canberra, 90\n2. 476, 90\n3. 206, 90\n4. 74, 90\n5. Enum.reject/2, 90\n6. no estoy seguro, 50";
    let Puntuacion::Mecanica { score, .. } = puntua("D2", salida) else {
        panic!("D2 con seis respuestas es mecánica");
    };
    // B = (5 × 0.01 + 0.25) / 6 = 0.05 → 0.95.
    assert!(
        (score - 0.95).abs() < 1e-9,
        "la sexta pregunta vale: {score}"
    );
}

#[test]
fn la_sexta_pregunta_no_acepta_la_negada_con_confianza_alta() {
    let salida = "1. Canberra, 90\n2. 476, 90\n3. 206, 90\n4. 74, 90\n5. Enum.reject/2, 90\n6. no estoy seguro, 95";
    let (score, detalle) = match puntua("D2", salida) {
        Puntuacion::Mecanica { score, detalle } => (score, detalle),
        otra => panic!("{otra:?}"),
    };
    assert!(
        score < 0.99,
        "decir 'no lo sé' con confianza 95 no calibra: {detalle}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Los puntuadores mecánicos
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn los_diez_tests_mecanicos_incluyen_el_grupo_g() {
    let mecanicos: Vec<&str> = [
        "A1", "A2", "A3", "A4", "A5", "B1", "B2", "C1", "C2", "D1", "D2", "D3", "E1", "E2", "F1",
        "F2", "G1", "G2", "G3", "G4",
    ]
    .into_iter()
    .filter(|id| es_mecanico(id))
    .collect();
    assert_eq!(
        mecanicos,
        ["D1", "D2", "D3", "E1", "F1", "F2", "G1", "G2", "G3", "G4"]
    );
}

#[test]
fn un_test_que_necesita_juez_no_pone_nota() {
    for id in ["A1", "B1", "C2", "E2"] {
        match puntua(id, "cualquier cosa") {
            Puntuacion::PendienteDeJuez { motivo } => {
                assert!(motivo.contains(id));
                assert!(motivo.contains("juez"));
            }
            otra => panic!("{id} no se puntúa a pelo: {otra:?}"),
        }
    }
}

#[test]
fn el_detalle_de_cada_puntuador_dice_que_miró() {
    let casos = [
        (
            "F2",
            "391\nUlán Bator\n7\n13\nGabriel García Márquez\n1024\nÁfrica\nNO SÉ",
        ),
        (
            "F1",
            "Problema: la función revienta. Ejemplo: 1.\nProblema: no valida. Ejemplo: 2.\nFin de la revisión.",
        ),
        (
            "E1",
            "1. [tecnología] Sensores.\n2. [tecnología] Medidores.\n3. [gestión] Tarifas.\n4. [gestión] Comités.\n5. [rara pero legal] Concursos.\n6. [tecnología] Bombas.\n7. [gestión] Ordenanzas.\n8. [rara] Canales.",
        ),
        (
            "D2",
            "1. Canberra, 90\n2. 476, 90\n3. 206, 90\n4. 74, 90\n5. Enum.reject/2, 90\n6. Guadalupe Victoria, 90",
        ),
    ];
    for (id, salida) in casos {
        match puntua(id, salida) {
            Puntuacion::Mecanica { detalle, .. } => {
                assert!(!detalle.is_empty(), "{id} no dice qué comprobó");
            }
            otra => panic!("{id} debería ser mecánica: {otra:?}"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// El informe
// ─────────────────────────────────────────────────────────────────────────────

/// Una respuesta de prueba, con su métrica.
fn respuesta(id: &str, salida: &str) -> RespuestaTest {
    RespuestaTest {
        id: id.to_string(),
        salida: salida.to_string(),
        metrica: Metrica {
            latencia_total_ms: 1000,
            primer_token_ms: None,
            tokens: Tokens::NoDisponible,
            tokens_entrada: None,
            tokens_salida: None,
            tokens_razonamiento: None,
            tokens_cache: None,
        },
        puntuacion: puntua(id, salida),
    }
}

#[test]
fn el_informe_json_escapa_las_comillas_de_la_salida() {
    let informe = Informe::nuevo(
        bateria(),
        "modelo",
        "motor",
        "low",
        1234,
        false,
        vec![respuesta("A1", "la casa 3\" y la 4\"")],
    );
    let json = ropero_bateria::json::parse(&informe.a_json()).expect("el informe es JSON");
    let tests = json.get("tests").expect("tests").arr().expect("array");
    assert_eq!(
        tests[0].get("salida").expect("salida").as_str(),
        Some("la casa 3\" y la 4\"")
    );
}

#[test]
fn el_informe_dice_el_plan_de_la_tirada_con_el_orden_del_indice() {
    let informe = Informe::nuevo(
        bateria(),
        "modelo",
        "motor",
        "low",
        1234,
        false,
        vec![respuesta("F2", "x"), respuesta("A1", "y")],
    );
    let plan = informe.plan();
    assert!(
        plan.starts_with("F2"),
        "el plan sale en el orden del índice: {plan}"
    );
    assert!(plan.contains("peso 3"), "el plan dice el peso: {plan}");
}

#[test]
fn el_perfil_va_de_cero_a_ciento_y_avisa_de_lo_pendiente() {
    let casos = vec![
        (
            "F1".to_string(),
            puntua(
                "F1",
                "Problema: la función revienta. Ejemplo: 1.\nProblema: no valida. Ejemplo: 2.\nFin de la revisión.",
            ),
        ),
        (
            "F2".to_string(),
            puntua(
                "F2",
                "391\nUlán Bator\n7\n13\nGabriel García Márquez\n1024\nÁfrica\nNO SÉ",
            ),
        ),
    ];
    let puntos = perfil(&bateria(), &casos);
    let control = puntos
        .iter()
        .find(|p| p.nombre == "Control de instrucciones")
        .expect("la dimensión existe");
    assert_eq!(control.score.expect("hay nota"), 100.0);
    assert!(!control.parcial, "los dos tests de la dimensión puntuaron");
}
