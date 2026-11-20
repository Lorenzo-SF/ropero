//! La selección de la batería: por tipo, por grupo, uno solo, o todos.
//!
//! El contrato (doc CLI v2 §13) dice que el comando ofrece lanzar **por
//! tipo, por grupo, uno solo, o todos**. El índice define grupos y
//! **dimensiones**: la segunda taxonomía del índice son las dimensiones, así
//! que `--tipo` es el **alias** de `--dimension` (el índice no declara ningún
//! "tipo"). Lo que no existe se rechaza con el valor recibido: nada se
//! interpreta y nada se inventa.

use ropero_bateria::indice::parse_indice;
use ropero_bateria::seleccion::{Opciones, Seleccion, seleccion_de, selecciona};

/// El índice real: la selección se prueba sobre la batería de verdad.
fn bateria_real() -> ropero_bateria::indice::Bateria {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/tests/indice.md");
    parse_indice(&std::fs::read_to_string(&ruta).expect("el índice del repo existe"))
        .expect("el índice real se parsea")
}

/// Unas opciones sin nada.
fn nadie() -> Opciones {
    Opciones::default()
}

#[test]
fn sin_seleccion_es_todos_y_devuelve_los_20_en_orden_del_indice() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(nadie()).expect("sin opciones es todos");
    assert_eq!(seleccion, Seleccion::Todos);
    let ids = selecciona(&bateria, &seleccion).expect("todos existe");
    assert_eq!(ids.len(), 20);
    assert_eq!(ids.first().expect("el primer test"), "A1");
    assert_eq!(ids.last().expect("el último test"), "G4");
    assert_eq!(ids[3], "A4", "el orden es el del índice, no el alfabético");
}

#[test]
fn un_grupo_devuelve_sus_tests_en_orden_del_indice() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        grupo: Some("D".to_string()),
        ..nadie()
    })
    .expect("el grupo D existe");
    assert_eq!(
        selecciona(&bateria, &seleccion).expect("grupo"),
        ["D1", "D2", "D3"]
    );
}

#[test]
fn el_grupo_se_acepta_tambien_en_minusculas() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        grupo: Some("d".to_string()),
        ..nadie()
    })
    .expect("el grupo d es el grupo D");
    assert_eq!(selecciona(&bateria, &seleccion).expect("grupo").len(), 3);
}

#[test]
fn un_test_se_elige_por_su_id() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        test: Some("F2".to_string()),
        ..nadie()
    })
    .expect("F2 existe");
    assert_eq!(selecciona(&bateria, &seleccion).expect("test"), ["F2"]);
}

#[test]
fn un_test_se_elige_por_su_fichero() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        test: Some("test-calibracion.md".to_string()),
        ..nadie()
    })
    .expect("el fichero del test D2 existe");
    assert_eq!(selecciona(&bateria, &seleccion).expect("test"), ["D2"]);
}

#[test]
fn una_dimension_se_elige_por_su_nombre_literal() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        dimension: Some("Control de instrucciones".to_string()),
        ..nadie()
    })
    .expect("la dimensión existe");
    assert_eq!(
        selecciona(&bateria, &seleccion).expect("dimensión"),
        ["F1", "F2"]
    );
}

#[test]
fn una_dimension_tolera_mayusculas_y_acentos() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        dimension: Some("abstraccion/patrones".to_string()),
        ..nadie()
    })
    .expect("la dimensión se reconoce sin el acento");
    assert_eq!(selecciona(&bateria, &seleccion).expect("dimensión"), ["A2"]);
}

#[test]
fn una_dimension_se_acepta_por_su_prefijo_unico() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        dimension: Some("honestidad".to_string()),
        ..nadie()
    })
    .expect("'honestidad' es la única dimensión que empieza así");
    assert_eq!(
        selecciona(&bateria, &seleccion).expect("dimensión"),
        ["C2", "D2"]
    );
}

#[test]
fn el_alias_tipo_es_lo_mismo_que_dimension() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        tipo: Some("memoria de trabajo".to_string()),
        ..nadie()
    })
    .expect("el alias de dimensión existe");
    assert_eq!(
        seleccion,
        Seleccion::Dimension("memoria de trabajo".to_string())
    );
    assert_eq!(selecciona(&bateria, &seleccion).expect("dimensión"), ["A4"]);
}

#[test]
fn el_alias_y_su_forma_larga_no_van_juntos() {
    let error = seleccion_de(Opciones {
        dimension: Some("Control de instrucciones".to_string()),
        tipo: Some("memoria de trabajo".to_string()),
        ..nadie()
    })
    .expect_err("--dimension y --tipo son la misma cosa");
    assert!(
        error.mensaje.contains("tipo"),
        "el mensaje dice que --tipo es el alias: {error}"
    );
}

#[test]
fn grupos_y_tests_se_combinan_sin_duplicar() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        grupo: Some("D".to_string()),
        test: Some("F2,D2".to_string()),
        ..nadie()
    })
    .expect("grupo y tests se combinan");
    assert_eq!(
        selecciona(&bateria, &seleccion).expect("selección"),
        ["D1", "D2", "D3", "F2"]
    );
}

#[test]
fn todos_no_se_combina_con_nada() {
    let error = seleccion_de(Opciones {
        todos: true,
        grupo: Some("D".to_string()),
        ..nadie()
    })
    .expect_err("todos y un grupo son dos cosas a la vez");
    assert!(
        error.mensaje.contains("todos"),
        "dice que 'todos' es una de las dos: {error}"
    );
}

#[test]
fn un_grupo_inexistente_es_error_con_el_valor_recibido() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        grupo: Some("Z".to_string()),
        ..nadie()
    })
    .expect("el valor se acepta, se rechaza al resolver");
    let error = selecciona(&bateria, &seleccion).expect_err("el grupo Z no existe");
    assert!(
        error.mensaje.contains("Z"),
        "nombra el valor recibido: {error}"
    );
    assert!(error.mensaje.contains("F"), "dice qué grupos hay: {error}");
}

#[test]
fn un_grupo_de_mas_de_una_letra_es_uso() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        grupo: Some("AB".to_string()),
        ..nadie()
    })
    .expect("el valor se acepta, se rechaza al resolver");
    let error = selecciona(&bateria, &seleccion).expect_err("'AB' no es un grupo");
    assert!(
        error.mensaje.contains("AB"),
        "nombra el valor recibido: {error}"
    );
}

#[test]
fn un_test_inexistente_es_error_con_el_valor_recibido() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        test: Some("Z9".to_string()),
        ..nadie()
    })
    .expect("el valor se acepta, se rechaza al resolver");
    let error = selecciona(&bateria, &seleccion).expect_err("Z9 no es un test");
    assert!(
        error.mensaje.contains("Z9"),
        "nombra el valor recibido: {error}"
    );
    assert!(
        error.mensaje.contains("ropero test"),
        "dice qué hacer: {error}"
    );
}

#[test]
fn una_dimension_inexistente_es_error_con_el_valor_y_las_validas() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        dimension: Some("velocidad".to_string()),
        ..nadie()
    })
    .expect("el valor se acepta, se rechaza al resolver");
    let error =
        selecciona(&bateria, &seleccion).expect_err("'velocidad' no es una dimensión del índice");
    assert!(
        error.mensaje.contains("velocidad"),
        "nombra el valor recibido: {error}"
    );
    assert!(
        error.mensaje.contains("Abstracción/patrones"),
        "dice qué dimensiones hay: {error}"
    );
}

#[test]
fn un_prefijo_de_dimension_que_ambigua_es_error() {
    let bateria = bateria_real();
    let seleccion = seleccion_de(Opciones {
        dimension: Some("ca".to_string()),
        ..nadie()
    })
    .expect("el valor se acepta, se rechaza al resolver");
    let error = selecciona(&bateria, &seleccion).expect_err("'ca' no es un prefijo único");
    assert!(
        error.mensaje.contains("ca"),
        "nombra el valor recibido: {error}"
    );
    assert!(
        error.mensaje.contains("Cálculo/probabilidad") && error.mensaje.contains("Causalidad"),
        "dice las dos dimensiones que empiezan igual: {error}"
    );
}

#[test]
fn un_valor_vacio_es_uso() {
    let error = seleccion_de(Opciones {
        grupo: Some("".to_string()),
        ..nadie()
    })
    .expect_err("un grupo vacío no es un grupo");
    assert!(
        error.mensaje.contains("grupo"),
        "dice qué flag quedó vacío: {error}"
    );
}
