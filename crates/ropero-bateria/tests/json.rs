//! El JSON de la batería: escapar, construir y leer, sin dependencias.
//!
//! El cuerpo del `POST` al motor, el resultado guardado en disco y la salida
//! de `ropero test --json` son JSON. Como no se añaden dependencias, el
//! escape y el parser viven aquí, y se prueban: un JSON mal escapado no lo
//! entiende el motor, y un JSON mal leído puntuaría mal.

use ropero_bateria::json::{Json, cadena, cuerpo_inferencia, escape, parse};

#[test]
fn escapa_la_comilla_y_la_barra_invertida() {
    // La entrada lleva **una** barra invertida: al escaparla sale **dos**.
    assert_eq!(escape(r#"hola "mundo" \"#), r#"hola \"mundo\" \\"#);
}

#[test]
fn escapa_el_salto_de_linea_el_tab_y_el_retorno() {
    assert_eq!(escape("a\nb\tc\r\nd"), r#"a\nb\tc\r\nd"#);
}

#[test]
fn escapa_los_caracteres_de_control() {
    assert_eq!(escape("a\x00b\x1fc"), r#"a\u0000b\u001fc"#);
}

#[test]
fn los_acentos_y_la_eñe_no_se_escapan() {
    assert_eq!(
        escape("Gabriel García Márquez — áéíóú ñ"),
        "Gabriel García Márquez — áéíóú ñ"
    );
}

#[test]
fn la_barra_no_se_escapa() {
    assert_eq!(escape("a/b"), "a/b");
}

#[test]
fn cadena_pone_el_texto_entre_comillas() {
    assert_eq!(cadena(r#"decir "hola""#), r#""decir \"hola\"""#);
}

#[test]
fn leer_un_objeto_sencillo() {
    let json = parse(r#"{"a": 1, "b": "dos", "c": true, "d": null}"#).expect("es un objeto");
    let Json::Obj(objeto) = &json else {
        panic!("no es objeto: {json:?}")
    };
    assert_eq!(objeto.get("a").expect("a").as_num(), Some(1.0));
    assert_eq!(objeto.get("b").expect("b").as_str(), Some("dos"));
    assert_eq!(objeto.get("c").expect("c").as_bool(), Some(true));
    assert_eq!(objeto.get("d").expect("d").as_null(), Some(()));
    assert_eq!(
        objeto.get("e"),
        None,
        "la clave que no está no inventa nada"
    );
}

#[test]
fn leer_un_array_anidado() {
    let json = parse(r#"{"peticiones": ["devolución", "factura"], "n": [1, 2, 3]}"#)
        .expect("es un objeto");
    let Json::Obj(objeto) = &json else {
        panic!("no es objeto")
    };
    let Json::Arr(items) = objeto.get("peticiones").expect("peticiones") else {
        panic!("peticiones no es array")
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].as_str(), Some("devolución"));
    let Json::Arr(numeros) = objeto.get("n").expect("n") else {
        panic!("n no es array")
    };
    assert_eq!(numeros.len(), 3);
}

#[test]
fn leer_una_cadena_con_escapes() {
    let json = parse(r#""una \n nueva\tlínea \u00e9""#).expect("es una cadena");
    assert_eq!(json.as_str(), Some("una \n nueva\tlínea é"));
}

#[test]
fn un_objeto_con_una_comilla_sin_cerrar_es_error() {
    let error = parse(r#"{"a": 1"#).expect_err("no está cerrado");
    assert!(error.mensaje.contains("comilla"), "dice qué falta: {error}");
}

#[test]
fn texto_sueltos_despues_del_json_es_error() {
    let error = parse(r#"{"a": 1} y más"#).expect_err("hay texto sueltos");
    assert!(
        error.mensaje.contains("resto"),
        "dice que sobró texto: {error}"
    );
}

#[test]
fn texto_vacio_es_error() {
    let error = parse("").expect_err("nada no es JSON");
    assert!(
        error.mensaje.contains("vacío"),
        "dice que está vacío: {error}"
    );
}

#[test]
fn texto_no_json_es_error() {
    let error = parse("Marta quiere devolver el #4412").expect_err("no es JSON");
    assert!(
        error.mensaje.contains("JSON"),
        "dice que no es JSON: {error}"
    );
}

#[test]
fn una_cadena_escalada_se_lee_como_la_cadena_original() {
    let original = "línea 1\nlínea 2 \"con comillas\" \\ barra";
    let texto = cadena(original);
    let json = parse(&texto).expect("el JSON escapado se lee");
    assert_eq!(
        json.as_str(),
        Some(original),
        "la ida y la vuelta son la misma cadena"
    );
}

#[test]
fn el_cuerpo_del_post_es_json_valido() {
    let prompt = "Responde cada pregunta en una sola línea, sin explicar.\nSi no sabes: \"NO SÉ\".";
    let cuerpo = cuerpo_inferencia("modelo-alpha", prompt, true);
    let json = parse(&cuerpo).expect("el cuerpo del POST es JSON");
    let Json::Obj(objeto) = &json else {
        panic!("el cuerpo tiene que ser un objeto")
    };
    assert_eq!(
        objeto.get("model").expect("model").as_str(),
        Some("modelo-alpha")
    );
    let Json::Arr(mensajes) = objeto.get("messages").expect("messages") else {
        panic!("messages tiene que ser un array")
    };
    assert_eq!(
        mensajes[0].get("content").expect("content").as_str(),
        Some(prompt)
    );
    assert_eq!(objeto.get("stream").expect("stream").as_bool(), Some(true));
}

#[test]
fn el_cuerpo_del_post_con_salida_no_streaming_lo_pide_explicito() {
    let cuerpo = cuerpo_inferencia("m", "hola", false);
    let json = parse(&cuerpo).expect("JSON");
    assert_eq!(json.get("stream").expect("stream").as_bool(), Some(false));
}

#[test]
fn el_cuerpo_del_post_escapa_el_prompto_con_acentos_y_comillas() {
    let prompt = "di «hola» y \"NO SÉ\"\n2^10";
    let cuerpo = cuerpo_inferencia("m", prompt, false);
    let json = parse(&cuerpo).expect("JSON escapado");
    let Json::Arr(mensajes) = json.get("messages").expect("messages") else {
        panic!("array")
    };
    assert_eq!(
        mensajes[0].get("content").expect("content").as_str(),
        Some(prompt)
    );
}

#[test]
fn escribir_un_objeto_produce_json_valido() {
    let objeto = Json::objeto(vec![
        ("test".to_string(), Json::Str("F2".to_string())),
        ("salida".to_string(), Json::Str("391\nNO SÉ".to_string())),
        ("score".to_string(), Json::Num(1.0)),
        ("pendiente".to_string(), Json::Bool(true)),
    ]);
    let texto = objeto.a_texto();
    let leido = parse(&texto).expect("el JSON escrito se lee");
    assert_eq!(leido.get("test").expect("test").as_str(), Some("F2"));
    assert_eq!(
        leido.get("salida").expect("salida").as_str(),
        Some("391\nNO SÉ")
    );
    assert_eq!(leido.get("score").expect("score").as_num(), Some(1.0));
    assert_eq!(
        leido.get("pendiente").expect("pendiente").as_bool(),
        Some(true)
    );
}
