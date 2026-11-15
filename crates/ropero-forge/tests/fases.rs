//! Las seis fases y el **checkpoint** (R-GGUF-15, T-30).

use ropero_forge::fases::{Checkpoint, Estado, Fase};

/// Las seis fases, **en orden fijo** (R-GGUF-15).
#[test]
fn las_seis_fases_en_orden_fijo() {
    assert_eq!(
        Fase::TODAS,
        [
            Fase::Fuente,
            Fase::Conversion,
            Fase::Cuantizacion,
            Fase::Verificacion,
            Fase::Limpieza,
            Fase::Procedencia,
        ]
    );
}

/// Cada fase tiene su siguiente, y la última no tiene más.
#[test]
fn el_orden_de_las_fases_encadena() {
    for fase in Fase::TODAS {
        match (fase.orden(), fase.siguiente()) {
            (5, None) => {}
            (orden, Some(siguiente)) => assert_eq!(siguiente.orden(), orden + 1),
            otro => panic!("la fase {fase:?} quedó mal encadenada: {otro:?}"),
        }
    }
}

/// El checkpoint se escribe y se lee, y **no** se inventa: un fichero que
/// no dice una fase de las seis no es un checkpoint.
#[test]
fn el_checkpoint_se_escribe_y_se_lee() {
    let escrito = Checkpoint::de(Fase::Conversion).a_json();
    let leido = Checkpoint::de_json(&escrito).expect("se lee");
    assert_eq!(leido, Checkpoint::de(Fase::Conversion));
}

/// Un checkpoint corrupto (o de otra herramienta) **no** reanuda: la
/// tirada empieza de cero, que es lo seguro.
#[test]
fn un_checkpoint_corrupto_no_reanuda() {
    for texto in ["", "no es json", "{\"fase\": \"otra\"}", "{\"fase\": 3}"] {
        assert_eq!(Checkpoint::de_json(texto), None, "con {texto:?}");
    }
}

/// **Hallazgo 1 (T-30)**: el checkpoint no guarda sólo la fase, guarda **qué
/// petición** la completó (su huella). Sin eso, una tirada con otros quants
/// sobre la misma fuente reanuda con el checkpoint de otra petición y anuncia
/// un GGUF que no existe.
#[test]
fn el_checkpoint_guarda_la_huella_de_la_petición() {
    let escrito = Checkpoint::con(Fase::Cuantizacion, "0123456789abcdef");
    let json = escrito.clone().a_json();
    assert!(
        json.contains("\"huella\": \"0123456789abcdef\""),
        "dice {json}"
    );
    let leido = Checkpoint::de_json(&json).expect("se lee");
    assert_eq!(leido, escrito, "la huella se perdió en el round-trip");
    assert!(
        leido.es_de("0123456789abcdef"),
        "no sabe de qué petición es"
    );
    assert!(!leido.es_de("fedcba9876543210"), "confunde dos peticiones");
}

/// Un checkpoint **de otra petición** no reanuda, y uno sin huella (el formato
/// viejo) tampoco es de ninguna: la tirada empieza de cero.
#[test]
fn un_checkpoint_de_otra_petición_no_reanuda() {
    let ajeno = Checkpoint::de_json(&Checkpoint::con(Fase::Procedencia, "huella-1").a_json())
        .expect("se lee");
    assert!(
        !ajeno.es_de("huella-2"),
        "reanuda con la petición equivocada"
    );

    let viejo = Checkpoint::de_json("{\n  \"fase\": \"procedencia\"\n}\n").expect("se lee");
    assert!(
        !viejo.es_de("huella-2"),
        "un checkpoint sin huella no es de nadie"
    );
}

/// La reanudación: un checkpoint de `Conversion` hace empezar por la
/// **cuantización** (T-30: no se repiten la descarga ni la conversión).
#[test]
fn el_checkpoint_de_la_conversion_reanuda_en_la_cuantizacion() {
    let checkpoint = Checkpoint::de_json(&Checkpoint::de(Fase::Conversion).a_json())
        .expect("se lee")
        .fase
        .siguiente()
        .expect("la cuantización");
    assert_eq!(checkpoint, Fase::Cuantizacion);
}

/// El checkpoint guarda el **estado** de cada salida y el **hash** del
/// imatrix (R-GGUF-20, R-GGUF-19).
///
/// El estado es lo que permite que una tirada reanudada diga la verdad en
/// el sidecar (si el cuantizador corrió en una tirada anterior, el sidecar
/// lo dice), y el hash es el del imatrix **remoto**: su caché la borra la
/// limpieza, y sin el hash el sidecar no dice cuál matriz se usó.
#[test]
fn el_checkpoint_guarda_los_estados_y_el_hash_del_imatrix() {
    let escrito = Checkpoint::con_estado(
        Fase::Cuantizacion,
        "0123456789abcdef",
        &[
            ("Q4_K_M".to_string(), Estado::Cuantizado),
            ("F16".to_string(), Estado::Convertido),
        ],
        Some("abc123"),
    );
    let json = escrito.clone().a_json();
    assert!(json.contains("\"estados\""), "dice {json}");
    assert!(
        json.contains("\"imatrix_sha256\": \"abc123\""),
        "dice {json}"
    );

    let leido = Checkpoint::de_json(&json).expect("se lee");
    assert_eq!(leido, escrito, "el estado se perdió en el round-trip");
    assert_eq!(leido.estados.len(), 2, "los estados no se leen");
    assert_eq!(leido.imatrix_sha256.as_deref(), Some("abc123"));
}

/// Un checkpoint del formato **viejo** (que sólo dice los quants
/// cuantizados) se lee: lo que no se sabe, no se inventa.
#[test]
fn un_checkpoint_del_formato_viejo_no_inventa_estados() {
    let viejo = Checkpoint::de_json(
        "{\n  \"fase\": \"cuantizacion\",\n  \"cuantizados\": \"Q4_K_M, IQ2_M\"\n}\n",
    )
    .expect("se lee");
    assert_eq!(
        viejo.estados,
        vec![
            ("Q4_K_M".to_string(), Estado::Cuantizado),
            ("IQ2_M".to_string(), Estado::Cuantizado),
        ],
        "el checkpoint viejo dice sólo lo que cuantizó"
    );
    assert_eq!(viejo.imatrix_sha256, None, "se inventó un hash");
}
