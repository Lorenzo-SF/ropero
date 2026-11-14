//! Las validaciones duras de la petición (R-GGUF-3/5/6/7).
//!
//! La puerta del doc: cada error es un error de **uso** (código 2), en
//! español, diciendo **qué hacer**. Y se comprueban **antes de nada**:
//! ninguna validación lee el disco, así que una petición mal escrita no
//! puede costar una descarga.

use ropero_forge::Error;
use ropero_forge::peticion::{Flags, Peticion};

/// El error de uso, para afinar las aserciones.
fn uso(peticion: Result<Peticion, Error>) -> String {
    match peticion {
        Err(Error::Uso(mensaje)) => mensaje,
        otro => panic!("esperaba un error de uso, y salió: {otro:?}"),
    }
}

/// La fuente es **una de tres**, y sin ninguna es un error de uso que
/// dice las tres.
#[test]
fn sin_fuente_es_error_de_uso() {
    let error = uso(Peticion::de_flags(Flags::default()));
    assert!(error.contains("--path"), "dice {error}");
    assert!(error.contains("--repo"), "dice {error}");
    assert!(error.contains("--gguf"), "dice {error}");
}

/// Dos fuentes a la vez son un error de uso (R-GGUF-3: excluyentes).
#[test]
fn dos_fuentes_son_error_de_uso() {
    for flags in [
        Flags::default()
            .con_repo("acme/modelo")
            .con_path("/tmp/pesos"),
        Flags::default().con_repo("acme/modelo").con_gguf("x.gguf"),
        Flags::default().con_path("/tmp/pesos").con_gguf("x.gguf"),
    ] {
        let error = uso(Peticion::de_flags(flags));
        assert!(error.contains("una sola"), "dice {error}");
    }
}

/// Una fuente sola es válida.
#[test]
fn cada_fuente_sola_es_valida() {
    for flags in [
        Flags::default().con_repo("acme/modelo"),
        Flags::default().con_path("/tmp/pesos"),
        Flags::default().con_gguf("modelo.gguf"),
    ] {
        let peticion = Peticion::de_flags(flags).expect("la fuente sola es válida");
        assert!(!peticion.fuente.describe().is_empty());
    }
}

/// `--repo` tiene que ser OWNER/NAME.
#[test]
fn un_repo_sin_owner_es_error_de_uso() {
    let error = uso(Peticion::de_flags(Flags::default().con_repo("modelo")));
    assert!(error.contains("OWNER/NAME"), "dice {error}");
}

/// `--imatrix` es excluyente con el par `--imatrix-repo`/`--imatrix-file`
/// (R-GGUF-6).
#[test]
fn la_imatrix_local_y_la_remota_son_excluyentes() {
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_imatrix("imx.dat")
        .con_imatrix_remota("acme/imx", "imx.dat");
    let error = uso(Peticion::de_flags(flags));
    assert!(error.contains("excluyente"), "dice {error}");
}

/// El par `--imatrix-repo`/`--imatrix-file` va **siempre juntos**.
#[test]
fn el_par_del_imatrix_va_junto() {
    for mut flags in [
        Flags::default()
            .con_repo("acme/modelo")
            .con_quants(&["IQ2_M"]),
        Flags::default()
            .con_repo("acme/modelo")
            .con_quants(&["IQ2_M"]),
    ] {
        flags.imatrix_repo = Some("acme/imx".to_string());
        let error = uso(Peticion::de_flags(flags.clone()));
        assert!(error.contains("pareja"), "dice {error}");
        flags.imatrix_repo = None;
        flags.imatrix_file = Some("imx.dat".to_string());
        let error = uso(Peticion::de_flags(flags));
        assert!(error.contains("pareja"), "dice {error}");
    }
}

/// Un imatrix **sin `--quant`** es un error (no hay nada que cuantizar).
#[test]
fn un_imatrix_sin_quant_es_error() {
    let error = uso(Peticion::de_flags(
        Flags::default()
            .con_repo("acme/modelo")
            .con_imatrix("imx.dat"),
    ));
    assert!(error.contains("--quant"), "dice {error}");
}

/// La excepción: `--diagnostico` está exenta de la regla del imatrix.
#[test]
fn el_diagnostico_esta_exento_de_la_regla_del_imatrix() {
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_imatrix("imx.dat")
        .con_diagnostico();
    assert!(
        Peticion::de_flags(flags).is_ok(),
        "el diagnóstico está exento"
    );
}

/// `--quant` es **repetible** y admite **coma** (R-GGUF-7).
#[test]
fn los_quants_son_repetibles_y_con_coma() {
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["iq2_m,IQ3_XS", "Q4_K_M"]);
    let peticion = Peticion::de_flags(flags).expect("los quants se parten");
    assert_eq!(peticion.quants, vec!["IQ2_M", "IQ3_XS", "Q4_K_M"]);
}

/// Un quant que no existe en la tabla de `llama_ftype` es error, y la
/// pista manda a `--list-quants`.
#[test]
fn un_quant_desconocido_es_error_con_pista() {
    let error = Peticion::de_flags(
        Flags::default()
            .con_repo("acme/modelo")
            .con_quants(&["Q9_K"]),
    )
    .expect_err("el quant no existe");
    match &error {
        Error::QuantInvalido { quant } => assert_eq!(quant, "Q9_K"),
        otro => panic!("esperaba un quant inválido, y salió: {otro:?}"),
    }
    assert!(error.pista().unwrap().contains("--list-quants"));
}

/// Con **varios** quants, `--out` no puede ser la ruta exacta de un
/// fichero (R-GGUF-5).
#[test]
fn con_varios_quants_la_out_no_es_un_fichero() {
    let error = uso(Peticion::de_flags(
        Flags::default()
            .con_repo("acme/modelo")
            .con_quants(&["IQ2_M", "IQ3_XS"])
            .con_out("/tmp/salida.gguf"),
    ));
    assert!(error.contains("nombre o un directorio"), "dice {error}");
}

/// Con **un solo** quant, `--out` sí puede ser la ruta exacta (R-GGUF-4).
#[test]
fn con_un_quant_la_out_puede_ser_un_fichero() {
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"])
        .con_out("/tmp/salida.gguf");
    assert!(Peticion::de_flags(flags).is_ok());
}

/// `--out-principal` exige una ruta de fichero `.gguf` (R-GGUF-4).
#[test]
fn la_out_principal_exige_un_fichero_gguf() {
    let error = uso(Peticion::de_flags(
        Flags::default()
            .con_repo("acme/modelo")
            .con_quants(&["IQ2_M"])
            .con_out_principal("/tmp/modelo"),
    ));
    assert!(error.contains(".gguf"), "dice {error}");
}

/// `--intermedio` sólo admite `f16` o `bf16`.
#[test]
fn el_intermedio_solo_admite_f16_y_bf16() {
    let error = uso(Peticion::de_flags(
        Flags::default()
            .con_repo("acme/modelo")
            .con_quants(&["IQ2_M"])
            .con_intermedio("q8"),
    ));
    assert!(error.contains("f16 o bf16"), "dice {error}");
}
