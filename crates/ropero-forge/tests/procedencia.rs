//! La **procedencia** (R-GGUF-19, T-34).
//!
//! La puerta de T-34: *un GGUF sin sidecar se rechaza*. La parte pura es
//! el contenido; que el fichero se escriba (y que la tirada falle si no
//! se puede) lo prueba la tirada con dobles (`tests/ejecutar.rs`).

use ropero_forge::procedencia::{ImatrixInfo, MtpInfo, Procedencia};

/// El sidecar dice la herramienta **que se usó de verdad** (hallazgo 6): si no
/// hubo cuantización, no puede afirmar que se llamó al cuantizador.
const CUANTIZA: &str = "ropero gguffer (llama.cpp llama-quantize)";
const SIN_CUANTIZAR: &str = "ropero gguffer (sin cuantizar)";

fn ejemplo() -> Procedencia {
    Procedencia {
        gguf: "modelo-IQ2_M.gguf".to_string(),
        quant: "IQ2_M".to_string(),
        fuente_repo: "acme/modelo".to_string(),
        intermedio: Some("F16".to_string()),
        herramienta: CUANTIZA.to_string(),
        imatrix: Some(ImatrixInfo {
            fichero: "imatrix.dat".to_string(),
            repo: Some("alguien/imatrix".to_string()),
        }),
        mtp: Some(MtpInfo {
            bloque: 64,
            quant: "Q4_0".to_string(),
        }),
        fecha: "2026-11-15T10:00:00Z".to_string(),
    }
}

/// El sidecar declara el quant **por nombre** (R-GGUF-19), la fuente y la
/// imatrix.
/// El sidecar no afirma lo que no pasó (hallazgo 6): `intermedio` es `null` cuando
/// no hubo conversión, `mtp` cuando no se creó, y la herramienta dice la verdad.
#[test]
fn el_sidecar_no_afirma_lo_que_no_pasó() {
    let honesto = Procedencia {
        intermedio: None,
        herramienta: SIN_CUANTIZAR.to_string(),
        mtp: None,
        ..ejemplo()
    };
    let json = honesto.a_json();
    assert!(json.contains("\"intermedio\": null"), "dice {json}");
    assert!(json.contains("\"mtp\": null"), "dice {json}");
    assert!(
        json.contains("\"herramienta\": \"ropero gguffer (sin cuantizar)\""),
        "dice {json}"
    );
}

#[test]
fn el_sidecar_declara_el_quant_por_nombre() {
    let json = ejemplo().a_json();
    assert!(json.contains("\"quant\": \"IQ2_M\""), "dice {json}");
    assert!(
        json.contains("\"fuente_repo\": \"acme/modelo\""),
        "dice {json}"
    );
    assert!(json.contains("\"fichero\": \"imatrix.dat\""), "dice {json}");
    assert!(
        json.contains("\"repo\": \"alguien/imatrix\""),
        "dice {json}"
    );
}

/// La cabeza MTP, si quedó en un quant aparte, queda declarada.
#[test]
fn el_sidecar_declara_la_mtp() {
    let json = ejemplo().a_json();
    assert!(json.contains("\"bloque\": 64"), "dice {json}");
    assert!(json.contains("\"quant\": \"Q4_0\""), "dice {json}");
}

/// Sin imatrix y sin MTP, los campos son `null` (no se miente por
/// omisión: el campo está y dice que no hubo).
#[test]
fn sin_imatrix_y_sin_mtp_los_campos_son_null() {
    let sin = Procedencia {
        imatrix: None,
        mtp: None,
        ..ejemplo()
    };
    let json = sin.a_json();
    assert!(json.contains("\"imatrix\": null"), "dice {json}");
    assert!(json.contains("\"mtp\": null"), "dice {json}");
}

/// La ruta del sidecar es `<out>.fuente.json`, al lado del GGUF.
#[test]
fn la_ruta_del_sidecar_es_al_lado() {
    let ruta = Procedencia::ruta(std::path::Path::new("/tmp/x/modelo-IQ2_M.gguf"));
    assert_eq!(
        ruta,
        std::path::PathBuf::from("/tmp/x/modelo-IQ2_M.gguf.fuente.json")
    );
}

/// Un valor con comillas no rompe el JSON.
#[test]
fn los_valores_con_comillas_se_escapan() {
    let escapada = Procedencia {
        fuente_repo: "acme/\"raro\"".to_string(),
        ..ejemplo()
    };
    let json = escapada.a_json();
    assert!(
        json.contains("\"fuente_repo\": \"acme/\\\"raro\\\"\""),
        "dice {json}"
    );
}
