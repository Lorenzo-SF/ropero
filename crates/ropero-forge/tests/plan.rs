//! La viabilidad **antes de descargar** (R-GGUF-8) y el plan de rutas.
//!
//! La puerta de R-GGUF-8: *si el quant pedido es distinto del que ya
//! traen los pesos, falla con rc≠0 sin bajar nada*. La viabilidad es
//! pura: no toca el disco, así que «sin bajar nada» es por construcción.
//! El test de la tirada entera (con el `hf` falso) afirma además que el
//! log de descargas queda **vacío**.

use std::path::PathBuf;

use ropero_core::Environment;
use ropero_forge::Error;
use ropero_forge::peticion::{Flags, Fuente, Intermedio, Peticion};
use ropero_forge::plan::{self, Clase, Plan};
use ropero_forge::quants;

fn peticion(quants: &[&str]) -> Peticion {
    let flags = Flags {
        repo: Some("acme/modelo-NVFP4".to_string()),
        quants: quants.iter().map(|q| q.to_string()).collect(),
        ..Flags::default()
    };
    Peticion::de_flags(flags).expect("la petición es válida")
}

/// (1) El GGUF ya está en el quant pedido → no se cuantiza (R-GGUF-7).
#[test]
fn el_mismo_quant_no_se_cuantiza_otra_vez() {
    let plan = plan::viabilidad(Clase::Nvfp4, Intermedio::F16, &["NVFP4".to_string()])
        .expect("el mismo quant es viable");
    assert!(!plan.cuantizar);
    assert!(plan.omitidos.contains(&"NVFP4".to_string()));
    assert!(plan.faltan.is_empty());
}

/// (2) Otro quant sobre pesos ya cuantizados → error **antes de bajar
/// nada** (R-GGUF-8), con la sugerencia del repo hermano.
#[test]
fn otro_quant_sobre_pesos_cuantizados_no_es_viable() {
    let error = plan::viabilidad(Clase::Nvfp4, Intermedio::F16, &["IQ2_M".to_string()])
        .expect_err("el motor no re-cuantiza");
    match &error {
        Error::NoViable { pedido, tienen, .. } => {
            assert_eq!(pedido, "IQ2_M");
            assert_eq!(tienen, "NVFP4");
        }
        otro => panic!("esperaba un NoViable, y salió: {otro:?}"),
    }
    assert_eq!(error.codigo(), 1);
}

/// La sugerencia del repo hermano sin cuantizar (R-GGUF-8).
#[test]
fn la_sugerencia_del_hermano_sin_cuantizar() {
    let hermanos = plan::sugerencia_hermanos("acme/modelo-NVFP4");
    assert_eq!(
        hermanos,
        vec![
            "acme/modelo-BF16".to_string(),
            "acme/modelo-F16".to_string()
        ]
    );
}

/// (3) Pesos sin cuantizar + quant de verdad → cuantizar, con imatrix.
#[test]
fn pesos_en_claro_y_quant_de_verdad_cuantiza() {
    let plan = plan::viabilidad(Clase::Ninguno, Intermedio::F16, &["IQ2_M".to_string()])
        .expect("es viable");
    assert!(plan.cuantizar);
    assert!(plan.faltan.contains(&"IQ2_M".to_string()));
}

/// (4) Un quant **plano** (F16/BF16) no es cuantización: no exige imatrix.
#[test]
fn un_quant_plano_no_exige_imatrix() {
    let plan = plan::viabilidad(Clase::Ninguno, Intermedio::Bf16, &["BF16".to_string()])
        .expect("BF16 a BF16 es una conversión de dtype");
    assert!(!plan.cuantizar);
}

/// Los pesos que el conversor **dequantiza** (FP8) salen en F16: sí se
/// pueden cuantizar desde ahí (R-GGUF-10).
#[test]
fn los_pesos_dequantizados_si_se_pueden_cuantizar() {
    let plan = plan::viabilidad(Clase::Dequant, Intermedio::F16, &["IQ2_M".to_string()])
        .expect("desde F16 se cuantiza");
    assert!(plan.cuantizar);
}

/// La clase NVFP4 fija el ftype esperado (39), el MXFP4 el 38 y el resto
/// el del intermedio (R-GGUF-9).
#[test]
fn el_ftype_esperado_de_cada_clase() {
    assert_eq!(Clase::Nvfp4.ftype_esperado(Intermedio::F16), 39);
    assert_eq!(Clase::Mxfp4.ftype_esperado(Intermedio::F16), 38);
    assert_eq!(Clase::Ninguno.ftype_esperado(Intermedio::F16), 1);
    assert_eq!(Clase::Ninguno.ftype_esperado(Intermedio::Bf16), 32);
    // El nombre es el canónico: el que va a la procedencia (R-GGUF-20).
    assert_eq!(Clase::Nvfp4.nombre_quant(Intermedio::F16), "NVFP4");
}

/// El plan: las salidas, una por quant, con el `--out-principal` primero.
#[test]
fn el_plan_pone_las_salidas_en_su_ruta() {
    let flags = Flags {
        repo: Some("acme/modelo".to_string()),
        quants: vec!["IQ2_M".to_string(), "IQ3_XS".to_string()],
        out_principal: Some("/tmp/modelo/principal.gguf".to_string()),
        ..Flags::default()
    };
    let peticion = Peticion::de_flags(flags).expect("la petición es válida");
    let env = Environment::default();
    let plan = Plan::de(&peticion, &env, Clase::Ninguno).expect("el plan");
    // El primero va a la ruta exacta del principal; los demás a su lado,
    // con el nombre del modelo y el quant.
    assert_eq!(
        plan.salidas[0].ruta,
        PathBuf::from("/tmp/modelo/principal.gguf")
    );
    assert!(plan.salidas[0].principal);
    assert_eq!(
        plan.salidas[1].ruta,
        PathBuf::from("/tmp/modelo/modelo-IQ3_XS.gguf")
    );
}

/// El intermedio se llama como lo que **lleva dentro**: pesos NVFP4 →
/// `<nombre>-NVFP4.gguf`, aunque el `--intermedio` diga f16.
#[test]
fn el_intermedio_se_llama_como_lo_que_lleva_dentro() {
    let peticion = peticion(&["NVFP4"]);
    let env = Environment::default();
    let plan = Plan::de(&peticion, &env, Clase::Nvfp4).expect("el plan");
    // El nombre ya trae el sufijo del quant ("modelo-NVFP4"), y el plan
    // no lo repite: "...-NVFP4-NVFP4" no dice nada.
    let ruta = plan.intermedio.to_string_lossy();
    assert!(ruta.ends_with(".gguf"), "dice {ruta}");
    assert!(ruta.contains("NVFP4"), "dice {ruta}");
    assert!(!ruta.contains("NVFP4-NVFP4"), "dice {ruta}");
}

/// Sin quants, la salida **es** el intermedio (la conversión es el
/// resultado, R-GGUF-18: el F16 como resultado no se toca).
#[test]
fn sin_quants_la_salida_es_el_intermedio() {
    let peticion = peticion(&[]);
    let env = Environment::default();
    let plan = Plan::de(&peticion, &env, Clase::Ninguno).expect("el plan");
    assert_eq!(plan.salidas.len(), 1);
    assert_eq!(plan.salidas[0].ruta, plan.intermedio);
    assert_eq!(plan.salidas[0].quant, "F16");
}

/// `--out <nombre>` manda sobre el nombre del repo.
#[test]
fn el_out_nombre_manda() {
    let flags = Flags {
        repo: Some("acme/modelo".to_string()),
        quants: vec!["IQ2_M".to_string()],
        out: Some("mi-quant".to_string()),
        ..Flags::default()
    };
    let peticion = Peticion::de_flags(flags).expect("la petición es válida");
    let env = Environment::default();
    let plan = Plan::de(&peticion, &env, Clase::Ninguno).expect("el plan");
    assert_eq!(plan.nombre, "mi-quant");
    assert_eq!(plan.salidas[0].numero, quants::numero_de("IQ2_M").unwrap());
}

/// El work de un `--gguf` local es un directorio **propio** dentro del
/// work root: el checkpoint y la caché del imatrix viven ahí, y el
/// directorio del usuario no se toca.
///
/// (Antes había una rama muerta que ponía el work en «.», que además de
/// noarse nunca, habría puesto el checkpoint en el directorio actual.)
#[test]
fn el_work_de_un_gguf_local_esta_en_el_work_root() {
    let flags = Flags {
        gguf: Some("/tmp/pesos/modelo.gguf".to_string()),
        ..Flags::default()
    };
    let peticion = Peticion::de_flags(flags).expect("la petición es válida");
    let mut vars = std::collections::BTreeMap::new();
    vars.insert("GGUF_WORK_DIR".to_string(), "/cache/gguffer".to_string());
    let env = Environment::new(vars, PathBuf::from("/home/usuario"), PathBuf::from("/cwd"));
    let plan = Plan::de(&peticion, &env, Clase::Ninguno).expect("el plan");

    let work = plan.work.to_string_lossy().into_owned();
    assert!(work.starts_with("/cache/gguffer"), "dice {work}");
    assert_ne!(plan.work, PathBuf::from("."));
    // Y no es el directorio del GGUF del usuario.
    assert_ne!(plan.work, PathBuf::from("/tmp/pesos"));
}

/// El nombre base de un modelo, según de dónde salga: es el nombre con el
/// que se buscan los GGUFs **publicados** de ese modelo.
#[test]
fn el_nombre_base_segun_la_fuente() {
    assert_eq!(
        plan::nombre_de_fuente(&Fuente::Repo("acme/modelo".to_string())),
        "modelo"
    );
    assert_eq!(
        plan::nombre_de_fuente(&Fuente::Path(PathBuf::from("/tmp/pesos"))),
        "pesos"
    );
    // De un GGUF: sin la extensión, y sin el `-GGUF` de los repos de
    // cuantizaciones.
    assert_eq!(
        plan::nombre_de_fuente(&Fuente::Gguf("acme/modelo-GGUF.gguf".to_string())),
        "modelo"
    );
    // Y sin el `.QUANT` final, si lo lleva.
    assert_eq!(
        plan::nombre_de_fuente(&Fuente::Gguf("acme/modelo.Q4_K_M.gguf".to_string())),
        "modelo"
    );
}
