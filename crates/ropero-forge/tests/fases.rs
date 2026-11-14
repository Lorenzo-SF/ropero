//! Las seis fases y el **checkpoint** (R-GGUF-15, T-30).

use ropero_forge::fases::{Checkpoint, Fase};

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
