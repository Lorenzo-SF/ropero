//! Las validaciones duras de la petición (R-GGUF-3/5/6/7).
//!
//! La puerta del doc: cada error es un error de **uso** (código 2), en
//! español, diciendo **qué hacer**. Y se comprueban **antes de nada**:
//! ninguna validación lee el disco, así que una petición mal escrita no
//! puede costar una descarga.

use ropero_core::Environment;
use ropero_forge::Error;
use ropero_forge::peticion::{self, Flags, Peticion};

/// El error de uso, para afinar las aserciones.
fn uso(peticion: Result<Peticion, Error>) -> String {
    match peticion {
        Err(Error::Uso(mensaje)) => mensaje,
        otro => panic!("esperaba un error de uso, y salió: {otro:?}"),
    }
}

/// Un entorno de pruebas: las variables que la forja mira (`$GGUF_DIR` y
/// `$GGUF_WORK_DIR`).
fn entorno(vars: &[(&str, &str)]) -> Environment {
    let mut mapa = std::collections::BTreeMap::new();
    for (clave, valor) in vars {
        mapa.insert(clave.to_string(), valor.to_string());
    }
    Environment::new(mapa, "/home/prueba", "/tmp/prueba")
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

// ─────────────────────────────────────────────────────────────────────────────
// La consulta (`--list-quants` y `--diagnostico`): las mismas reglas, con
// dos excepciones del doc.
// ─────────────────────────────────────────────────────────────────────────────

/// La consulta **no exige fuente**: preguntar no es fabricar.
#[test]
fn la_consulta_no_exige_fuente() {
    assert!(peticion::valida_consulta(&Flags::default()).is_ok());
}

/// La consulta **sí** dice que la fuente es una sola (R-GGUF-3).
#[test]
fn la_consulta_tambien_exige_una_fuente_sola() {
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_path("/tmp/pesos");
    let error = peticion::valida_consulta(&flags).expect_err("dos fuentes");
    match error {
        Error::Uso(mensaje) => assert!(mensaje.contains("excluyentes"), "dice {mensaje}"),
        otro => panic!("esperaba un error de uso, y salió: {otro:?}"),
    }
}

/// El imatrix **no** exige `--quant` en la consulta (R-GGUF-6): el
/// diagnóstico está exento.
#[test]
fn la_consulta_esta_exenta_de_la_regla_del_imatrix() {
    let flags = Flags::default().con_imatrix("imx.dat");
    assert!(
        peticion::valida_consulta(&flags).is_ok(),
        "el diagnóstico está exento"
    );
}

/// Pero el imatrix local y el remoto siguen siendo **excluyentes**.
#[test]
fn la_consulta_tambiene_excluye_los_dos_imatrix() {
    let flags = Flags::default()
        .con_imatrix("imx.dat")
        .con_imatrix_remota("acme/imx", "imx.dat");
    let error = peticion::valida_consulta(&flags).expect_err("los dos imatrix");
    match error {
        Error::Uso(mensaje) => assert!(mensaje.contains("excluyente"), "dice {mensaje}"),
        otro => panic!("esperaba un error de uso, y salió: {otro:?}"),
    }
}

/// Un quant que no existe es error en la consulta, también.
#[test]
fn la_consulta_tambiene_exige_quants_que_existen() {
    let flags = Flags::default().con_quants(&["Q9_K"]);
    let error = peticion::valida_consulta(&flags).expect_err("el quant no existe");
    assert!(
        matches!(error, Error::QuantInvalido { .. }),
        "sale: {error:?}"
    );
}

/// **Hallazgo 4** (doc CLI v2 §3): un `--quant` inválido es un argumento
/// inválido, o sea **uso**: sale con **2**, no con 1.
#[test]
fn un_quant_invalido_es_error_de_uso_y_sale_con_dos() {
    let error = peticion::valida_consulta(&Flags::default().con_quants(&["Q9_Z"]))
        .expect_err("el quant no existe");
    assert_eq!(error.codigo(), 2, "sale con {}: {error}", error.codigo());
    assert!(
        error.to_string().contains("Q9_Z"),
        "el mensaje no dice el valor recibido: {error}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// La pista (P-13): todos los errores dicen **qué hacer**.
// ─────────────────────────────────────────────────────────────────────────────

/// **Hallazgo 5** (P-13): los errores de ejecución —verificación, procedencia,
/// disco, fuente que no está y descarga— no tenían pista.
#[test]
fn los_errores_de_ejecucion_dicen_que_hacer() {
    let verificacion = Error::Verificacion {
        gguf: "models/gguf/modelo-Q4_K_M.gguf".to_string(),
        motivo: "la cabecera no es GGUF".to_string(),
        pesos: "NO borro los pesos: están en /cache/ropero/gguffer/acme__modelo".to_string(),
    };
    assert!(
        verificacion
            .pista()
            .is_some_and(|pista| pista.contains("rehaz") && pista.contains("--force")),
        "no dice qué hacer: {verificacion}"
    );

    let procedencia = Error::Procedencia {
        dest: "/models/gguf/modelo.gguf.fuente.json".to_string(),
        detalle: "permiso denegado".to_string(),
    };
    assert!(
        procedencia
            .pista()
            .is_some_and(|pista| pista.contains("comprueba")),
        "no dice qué hacer: {procedencia}"
    );

    let disco = Error::Disco {
        que: "crear el directorio /cache/ropero/gguffer".to_string(),
        detalle: "disco lleno".to_string(),
    };
    assert!(
        disco
            .pista()
            .is_some_and(|pista| pista.contains("comprueba")),
        "no dice qué hacer: {disco}"
    );

    let fuente = Error::FuenteNoEsta {
        fuente: "--path /models/qwen".to_string(),
        ruta: "/models/qwen".to_string(),
    };
    assert!(
        fuente
            .pista()
            .is_some_and(|pista| pista.contains("comprueba") && pista.contains("hf download")),
        "no dice qué hacer: {fuente}"
    );

    let descarga = Error::Descarga {
        repo: "acme/modelo".to_string(),
        codigo: Some(1),
    };
    assert!(
        descarga
            .pista()
            .is_some_and(|pista| pista.contains("reintenta") && pista.contains("HF_TOKEN")),
        "no dice qué hacer: {descarga}"
    );
}

// ── La huella de la petición: qué es y qué no ──────────────────────────────

/// La huella es **determinista**: la misma petición, la misma huella.
///
/// Es lo que hace que el checkpoint sea de *esta* tirada (T-30): si la
/// huella cambiara sola, ninguna tirada podría reanudarse.
#[test]
fn la_huella_es_determinista() {
    let env = entorno(&[("GGUF_DIR", "/tmp/salida")]);
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"]);
    let peticion = Peticion::de_flags(flags.clone()).expect("la petición");
    assert_eq!(
        peticion.huella(&env),
        peticion.huella(&env),
        "no es estable"
    );
    // Y dos peticiones con lo mismo dan la misma huella.
    let otra = Peticion::de_flags(flags).expect("la petición");
    assert_eq!(
        peticion.huella(&env),
        otra.huella(&env),
        "dos peticiones iguales no coinciden"
    );
}

/// La huella **cambia** cuando cambia lo que se fabrica: el quant, el
/// intermedio, el imatrix o la salida.
#[test]
fn la_huella_cambia_cuando_cambia_la_tirada() {
    let env = entorno(&[("GGUF_DIR", "/tmp/salida")]);
    let base = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"]);
    let referencia = Peticion::de_flags(base.clone())
        .expect("la petición")
        .huella(&env);

    let otro = Peticion::de_flags(base.clone().con_quants(&["Q4_K_M"])).expect("la petición");
    assert_ne!(
        otro.huella(&env),
        referencia,
        "cambiar el quant no cambia la huella"
    );

    let otro = Peticion::de_flags(base.clone().con_intermedio("bf16")).expect("la petición");
    assert_ne!(
        otro.huella(&env),
        referencia,
        "cambiar el intermedio no cambia la huella"
    );

    let otro = Peticion::de_flags(base.clone().con_imatrix("/tmp/imx.dat")).expect("la petición");
    assert_ne!(
        otro.huella(&env),
        referencia,
        "cambiar el imatrix no cambia la huella"
    );

    let otro = Peticion::de_flags(base.clone().con_out("otro")).expect("la petición");
    assert_ne!(
        otro.huella(&env),
        referencia,
        "cambiar la salida no cambia la huella"
    );
}

/// La huella **no** cambia con lo que no cambia lo que se fabrica: la
/// limpieza (`--keep-source`, `--keep-intermedio`) ni el directorio de
/// trabajo (que es temporal: si entrara, la reanudación dejaría de
/// funcionar).
#[test]
fn la_huella_no_cambia_con_la_limpieza_ni_con_el_directorio_de_trabajo() {
    let env = entorno(&[
        ("GGUF_DIR", "/tmp/salida"),
        ("GGUF_WORK_DIR", "/tmp/trabajo-1"),
    ]);
    let mut flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"]);
    let referencia = Peticion::de_flags(flags.clone())
        .expect("la petición")
        .huella(&env);

    flags.keep_source = true;
    flags.keep_intermedio = true;
    let otra = Peticion::de_flags(flags).expect("la petición");
    assert_eq!(
        otra.huella(&env),
        referencia,
        "la limpieza cambia la huella"
    );

    let otro_trabajo = entorno(&[
        ("GGUF_DIR", "/tmp/salida"),
        ("GGUF_WORK_DIR", "/tmp/trabajo-2"),
    ]);
    assert_eq!(
        otra.huella(&otro_trabajo),
        referencia,
        "el directorio de trabajo cambia la huella"
    );
}

/// El **directorio de salida resuelto** va en la huella.
///
/// El defecto: la huella no lo incluía, así que la misma petición con otro
/// `$GGUF_DIR` reutilizaba el checkpoint, no fabricaba nada y la tirada
/// moría con «salida faltante» en vez de rehacerse.
#[test]
fn la_huella_cambia_con_el_directorio_de_salida() {
    let flags = Flags::default()
        .con_repo("acme/modelo")
        .con_quants(&["IQ2_M"]);
    let peticion = Peticion::de_flags(flags).expect("la petición");
    let uno = entorno(&[("GGUF_DIR", "/tmp/salida-1")]);
    let otro = entorno(&[("GGUF_DIR", "/tmp/salida-2")]);
    assert_ne!(
        peticion.huella(&uno),
        peticion.huella(&otro),
        "el directorio de salida no va en la huella"
    );
    // Y el mismo directorio, la misma huella.
    assert_eq!(
        peticion.huella(&uno),
        peticion.huella(&entorno(&[("GGUF_DIR", "/tmp/salida-1")])),
        "el mismo directorio da huellas distintas"
    );
}
