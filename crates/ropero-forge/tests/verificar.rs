//! La lectura de la **cabecera** de un GGUF (R-GGUF-7).
//!
//! `general.file_type` es el `llama_ftype` de los pesos: es lo que dice si un
//! GGUF ya está cuantizado, y por eso la clase se lee **de la cabecera** y no
//! del nombre (`modelo-BF16.gguf` con cabecera IQ2_M es un IQ2_M).
//!
//! Las fixtures se construyen **según el formato**, no según el código: los
//! números de tipo van **a mano** (la tabla de `gguf.h`), así que si el
//! código vuelve a equivocarlos, estos tests **fallan**.

use std::fs;
use std::path::{Path, PathBuf};

use ropero_forge::verificar;

// ── La tabla de tipos del formato GGUF (la de `gguf.h`) ────────────────────
//
// A mano, a propósito: el test tiene que **fallar** si el código cambia sus
// constantes (que es lo que pasó: el código usaba el 6 para el UINT32 y el 13
// para la ARRAY, y no encontraba el `general.file_type` en ningún GGUF real).
const UINT8: u32 = 0;
const INT8: u32 = 1;
const UINT16: u32 = 2;
const INT16: u32 = 3;
const UINT32: u32 = 4;
const INT32: u32 = 5;
const FLOAT32: u32 = 6;
const BOOL: u32 = 7;
const STRING: u32 = 8;
const ARRAY: u32 = 9;
const UINT64: u32 = 10;
const INT64: u32 = 11;
const FLOAT64: u32 = 12;

/// Un valor de la metadata, con el tipo que lo guarda.
#[derive(Debug, Clone)]
enum Valor {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    F32(f32),
    Bool(bool),
    Str(String),
    U64(u64),
    I64(i64),
    F64(f64),
    /// Una array: el tipo de sus elementos y los elementos.
    Array(u32, Vec<Valor>),
}

impl Valor {
    /// El tipo con el que se guarda este valor.
    fn tipo(&self) -> u32 {
        match self {
            Valor::U8(_) => UINT8,
            Valor::I8(_) => INT8,
            Valor::U16(_) => UINT16,
            Valor::I16(_) => INT16,
            Valor::U32(_) => UINT32,
            Valor::I32(_) => INT32,
            Valor::F32(_) => FLOAT32,
            Valor::Bool(_) => BOOL,
            Valor::Str(_) => STRING,
            Valor::U64(_) => UINT64,
            Valor::I64(_) => INT64,
            Valor::F64(_) => FLOAT64,
            Valor::Array(_, _) => ARRAY,
        }
    }
}

/// La **longitud** de las cadenas y de las arrays: de 32 bits en la v1 y de 64
/// desde la v2.
fn ancho_de(version: u32) -> usize {
    if version == 1 { 4 } else { 8 }
}

/// Escribe una cadena: su longitud y sus bytes.
fn cadena(datos: &mut Vec<u8>, texto: &str, ancho: usize) {
    if ancho == 8 {
        datos.extend_from_slice(&(texto.len() as u64).to_le_bytes());
    } else {
        datos.extend_from_slice(&(texto.len() as u32).to_le_bytes());
    }
    datos.extend_from_slice(texto.as_bytes());
}

/// Escribe el valor de un KV, con su tipo.
fn escribe_valor(datos: &mut Vec<u8>, valor: &Valor, ancho: usize) {
    match valor {
        Valor::U8(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::I8(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::U16(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::I16(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::U32(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::I32(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::F32(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::Bool(n) => datos.extend_from_slice(&u8::from(*n).to_le_bytes()),
        Valor::Str(s) => {
            if ancho == 8 {
                datos.extend_from_slice(&(s.len() as u64).to_le_bytes());
            } else {
                datos.extend_from_slice(&(s.len() as u32).to_le_bytes());
            }
            datos.extend_from_slice(s.as_bytes());
        }
        Valor::U64(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::I64(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::F64(n) => datos.extend_from_slice(&n.to_le_bytes()),
        Valor::Array(tipo_elemento, elementos) => {
            // El tipo del elemento va **antes** que su número, y es un u32
            // **siempre**.
            datos.extend_from_slice(&tipo_elemento.to_le_bytes());
            if ancho == 8 {
                datos.extend_from_slice(&(elementos.len() as u64).to_le_bytes());
            } else {
                datos.extend_from_slice(&(elementos.len() as u32).to_le_bytes());
            }
            for elemento in elementos {
                escribe_valor(datos, elemento, ancho);
            }
        }
    }
}

/// Un GGUF de fixture: la cabecera y sus KVs.
///
/// Los contadores (`n_tensors` y `n_kv`) son u64, como dice el formato; las
/// **longitudes** son u32 en la v1 y u64 desde la v2.
fn gguf(version: u32, kvs: &[(&str, u32, Valor)]) -> Vec<u8> {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&version.to_le_bytes());
    datos.extend_from_slice(&0u64.to_le_bytes()); // n_tensors
    datos.extend_from_slice(&(kvs.len() as u64).to_le_bytes()); // n_kv
    let ancho = ancho_de(version);
    for (clave, tipo, valor) in kvs {
        assert_eq!(*tipo, valor.tipo(), "el tipo no casa con el valor");
        cadena(&mut datos, clave, ancho);
        datos.extend_from_slice(&tipo.to_le_bytes());
        escribe_valor(&mut datos, valor, ancho);
    }
    datos
}

/// Un GGUF de fixture de la **v1 histórica**: los contadores de 32 bits.
///
/// La v1 usaba u32 para los contadores y la v2 los pasó a u64, y la cabecera
/// no lo dice: el lector tiene que valer para las dos.
fn gguf_v1_contadores_32(kvs: &[(&str, u32, Valor)]) -> Vec<u8> {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&1u32.to_le_bytes()); // versión
    datos.extend_from_slice(&0u32.to_le_bytes()); // n_tensors
    datos.extend_from_slice(&(kvs.len() as u32).to_le_bytes()); // n_kv
    for (clave, tipo, valor) in kvs {
        assert_eq!(*tipo, valor.tipo(), "el tipo no casa con el valor");
        cadena(&mut datos, clave, 4);
        datos.extend_from_slice(&tipo.to_le_bytes());
        escribe_valor(&mut datos, valor, 4);
    }
    datos
}

/// Un GGUF de fixture en un directorio limpio.
fn fichero(datos: &[u8]) -> PathBuf {
    let ruta = ruta_de_fixture();
    fs::write(&ruta, datos).expect("escribir el GGUF");
    ruta
}

/// Un GGUF de fixture **disperso**: se escriben los bytes que importan y
/// el fichero se estira hasta `tamaño` sin ocupar disco de verdad.
///
/// Es lo que permite probar el tope de la lectura (un MiB) sin reservar
/// 240 MB en RAM: el hueco del fichero no lo ocupa nada.
fn fichero_disperso(datos: &[u8], tamaño: u64) -> PathBuf {
    let ruta = ruta_de_fixture();
    let mut fichero = fs::File::create(&ruta).expect("crear el GGUF");
    use std::io::Write;
    fichero.write_all(datos).expect("escribir el GGUF");
    fichero.set_len(tamaño).expect("estirar el GGUF");
    ruta
}

/// La ruta de un fixture: un directorio limpio **por fixture** (los tests
/// corren en paralelo, y cada uno necesita el suyo).
fn ruta_de_fixture() -> PathBuf {
    static CONTADOR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = CONTADOR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base =
        std::env::temp_dir().join(format!("ropero-forge-verificar-{}-{n}", std::process::id()));
    fs::create_dir_all(&base).expect("crear el fixture");
    base.join("modelo.gguf")
}

/// Un KV con una **cadena enorme declarada** (sus bytes no están: es lo que
/// pasa con el vocabulario de un modelo grande).
///
/// La clave es una de las que el lector **quiere** (`general.architecture`),
/// así que la lectura se topa con ella y no puede seguir: la metadata queda
/// incompleta.
fn kv_cadena_enorme(datos: &mut Vec<u8>, clave: &str, longitud: u64) {
    cadena(datos, clave, 8);
    datos.extend_from_slice(&STRING.to_le_bytes());
    datos.extend_from_slice(&longitud.to_le_bytes());
}

/// Un KV con el `general.file_type` (un uint32, **tipo 4**).
fn kv_file_type(datos: &mut Vec<u8>, file_type: u32) {
    cadena(datos, "general.file_type", 8);
    datos.extend_from_slice(&UINT32.to_le_bytes());
    datos.extend_from_slice(&file_type.to_le_bytes());
}

/// La cabecera de un GGUF de la v3, con `n_kv` KVs.
fn cabecera_v3(n_kv: u64) -> Vec<u8> {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&3u32.to_le_bytes()); // versión
    datos.extend_from_slice(&0u64.to_le_bytes()); // n_tensors
    datos.extend_from_slice(&n_kv.to_le_bytes()); // n_kv
    datos
}

/// Borra el fixture.
fn limpia(ruta: &Path) {
    if let Some(base) = ruta.parent() {
        let _ = fs::remove_dir_all(base);
    }
}

/// El `general.file_type` (un uint32, **tipo 4**) se lee de la metadata.
#[test]
fn el_file_type_se_lee_de_la_metadata() {
    let datos = gguf(
        3,
        &[(
            "general.file_type",
            UINT32,
            Valor::U32(15), // Q4_K_M
        )],
    );
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), Some(15));
    limpia(&ruta);
}

/// La **v1**: las longitudes son de 32 bits.
#[test]
fn la_version_uno_usa_longitudes_de_32_bits() {
    let datos = gguf(
        1,
        &[("general.file_type", UINT32, Valor::U32(1))], // F16
    );
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), Some(1));
    limpia(&ruta);
}

/// La **v1 histórica**: los contadores también de 32 bits.
#[test]
fn la_version_uno_histórica_tiene_contadores_de_32_bits() {
    let datos = gguf_v1_contadores_32(&[("general.file_type", UINT32, Valor::U32(1))]);
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), Some(1));
    limpia(&ruta);
}

/// La **v2**: las longitudes ya son de 64 bits (la v1 usaba 32).
#[test]
fn la_version_dos_usa_longitudes_de_64_bits() {
    let datos = gguf(
        2,
        &[("general.file_type", UINT32, Valor::U32(29))], // IQ2_M
    );
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), Some(29));
    limpia(&ruta);
}

/// El `general.file_type` es un número, pero el formato admite que se guarde
/// con **otro tipo**: se lee igual.
#[test]
fn el_file_type_se_lee_de_cualquier_tipo() {
    for (tipo, valor) in [
        (UINT32, Valor::U32(29)),
        (FLOAT32, Valor::F32(29.0)),
        (STRING, Valor::Str("29".to_string())),
        (ARRAY, Valor::Array(UINT32, vec![Valor::U32(29)])),
        (UINT64, Valor::U64(29)),
        (FLOAT64, Valor::F64(29.0)),
    ] {
        let datos = gguf(3, &[("general.file_type", tipo, valor)]);
        let ruta = fichero(&datos);
        assert_eq!(
            verificar::file_type(&ruta),
            Some(29),
            "el tipo {tipo} no se lee"
        );
        limpia(&ruta);
    }
}

/// Todos los **tipos de valor** del formato se saben saltar: un KV de cada
/// tipo antes del `general.file_type` no lo estropea (si un tipo está mal
/// dimensionado, el lector se desvía y no encuentra nada).
#[test]
fn todos_los_tipos_de_valor_se_saltan_bien() {
    let datos = gguf(
        3,
        &[
            ("general.u8", UINT8, Valor::U8(1)),
            ("general.i8", INT8, Valor::I8(-1)),
            ("general.u16", UINT16, Valor::U16(2)),
            ("general.i16", INT16, Valor::I16(-2)),
            ("general.u32", UINT32, Valor::U32(3)),
            ("general.i32", INT32, Valor::I32(-3)),
            ("general.f32", FLOAT32, Valor::F32(3.5)),
            ("general.bool", BOOL, Valor::Bool(true)),
            ("general.str", STRING, Valor::Str("hola".to_string())),
            ("general.u64", UINT64, Valor::U64(4)),
            ("general.i64", INT64, Valor::I64(-4)),
            ("general.f64", FLOAT64, Valor::F64(4.5)),
            (
                "general.array",
                ARRAY,
                Valor::Array(UINT16, vec![Valor::U16(7), Valor::U16(8)]),
            ),
            ("general.file_type", UINT32, Valor::U32(15)),
        ],
    );
    let ruta = fichero(&datos);
    assert_eq!(
        verificar::file_type(&ruta),
        Some(15),
        "un tipo mal dimensionado desvió la lectura"
    );
    limpia(&ruta);
}

/// Los metadatos **antes** del `general.file_type` no lo estropean.
#[test]
fn los_metadatos_de_antes_del_file_type_no_lo_rompen() {
    let datos = gguf(
        3,
        &[
            (
                "general.architecture",
                STRING,
                Valor::Str("qwen3".to_string()),
            ),
            ("general.name", STRING, Valor::Str("modelo".to_string())),
            ("qwen3.block_count", UINT32, Valor::U32(48)),
            (
                "tokenizer.ggml.tokens",
                ARRAY,
                Valor::Array(STRING, vec![Valor::Str("a".to_string())]),
            ),
            ("general.file_type", UINT32, Valor::U32(15)),
        ],
    );
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), Some(15), "se perdió el KV");
    limpia(&ruta);
}

/// Las **arrays anidadas** no rompen la lectura (y su profundidad está
/// acotada: no pueden colgar la lectura).
#[test]
fn las_arrays_anidadas_no_rompen_la_lectura() {
    let datos = gguf(
        3,
        &[
            (
                "general.anidada",
                ARRAY,
                Valor::Array(
                    ARRAY,
                    vec![
                        Valor::Array(UINT32, vec![Valor::U32(1), Valor::U32(2)]),
                        Valor::Array(UINT32, vec![Valor::U32(3)]),
                    ],
                ),
            ),
            ("general.file_type", UINT32, Valor::U32(15)),
        ],
    );
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), Some(15), "se perdió el KV");
    limpia(&ruta);
}

/// Un GGUF **sin** `general.file_type` no puede dar su clase: hay que fiarse
/// del nombre.
#[test]
fn un_gguf_sin_file_type_no_da_file_type() {
    let datos = gguf(
        3,
        &[(
            "general.architecture",
            STRING,
            Valor::Str("qwen3".to_string()),
        )],
    );
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), None, "inventa un file_type");
    limpia(&ruta);
}

/// Un fichero **truncado** no da clase: la metadata promete más KVs de los
/// que hay, y no se inventa ninguno.
#[test]
fn un_fichero_truncado_no_da_file_type() {
    let mut datos = gguf(
        3,
        &[
            ("general.file_type", UINT32, Valor::U32(15)),
            ("general.otro", UINT32, Valor::U32(1)),
        ],
    );
    // Se corta por el medio del segundo KV.
    datos.truncate(datos.len() - 6);
    let ruta = fichero(&datos);
    assert_eq!(
        verificar::file_type(&ruta),
        None,
        "lee un fichero truncado como si estuviera entero"
    );
    limpia(&ruta);
}

/// Una **versión** que no es 1, 2 o 3 no se lee (el formato no la tiene).
#[test]
fn una_version_invalida_no_da_file_type() {
    for version in [0u32, 4, 99] {
        let datos = gguf(version, &[("general.file_type", UINT32, Valor::U32(15))]);
        let ruta = fichero(&datos);
        assert_eq!(
            verificar::file_type(&ruta),
            None,
            "lee la versión {version}"
        );
        limpia(&ruta);
    }
}

/// Un fichero que no es GGUF, o que no está, no da clase (no panica).
#[test]
fn lo_que_no_es_gguf_no_da_file_type() {
    let mut datos = b"NOTGGUF".to_vec();
    datos.extend_from_slice(&[0u8; 2048]);
    let ruta = fichero(&datos);
    assert_eq!(verificar::file_type(&ruta), None);
    assert_eq!(
        verificar::file_type(Path::new("/no/existe/modelo.gguf")),
        None
    );
    limpia(&ruta);
}

/// Una **array** desmesurada no se lee entera: la lectura de la metadata
/// está acotada y no se inventa un valor.
///
/// La fixture es **barata**: declara 60 millones de elementos y **no los
/// escribe** (la anterior los escribía: 240 MB en RAM y 7 s de test). Con
/// eso basta para probar el tope: la lectura se topa con él y no inventa
/// nada.
#[test]
fn una_array_desmesurada_no_se_lee_entera() {
    let mut datos = cabecera_v3(1);
    cadena(&mut datos, "general.arr", 8);
    datos.extend_from_slice(&ARRAY.to_le_bytes());
    datos.extend_from_slice(&UINT32.to_le_bytes()); // el tipo del elemento
    datos.extend_from_slice(&60_000_000u64.to_le_bytes()); // su número
    // El fichero pasa del tope de la lectura (un MiB), pero es **disperso**:
    // no reserva nada en RAM.
    let ruta = fichero_disperso(&datos, 1_100_000);
    assert_eq!(verificar::file_type(&ruta), None, "se inventa un file_type");
    limpia(&ruta);
}

/// La lectura de la metadata está **acotada** (un MiB): lo que está más allá
/// no se lee, y no se inventa.
///
/// El `general.file_type` va **después** de una cadena enorme: queda fuera
/// de lo leído, así que no se sabe (y `None` no significa «F16»).
#[test]
fn la_lectura_de_la_metadata_esta_acotada() {
    let mut datos = cabecera_v3(2);
    // Una cadena enorme (el vocabulario de un modelo grande) **antes** del
    // file_type.
    kv_cadena_enorme(&mut datos, "general.architecture", 1_500_000);
    kv_file_type(&mut datos, 15);
    let ruta = fichero_disperso(&datos, 1_600_000);
    assert_eq!(
        verificar::file_type(&ruta),
        None,
        "se leyó más allá del tope de la lectura"
    );
    limpia(&ruta);
}

/// El `general.file_type` se lee **aunque la metadata no quepa** en la
/// lectura: lo que se encontró, se queda.
///
/// El fichero es más largo que lo leído, así que lo que falta es metadata
/// que no se ha leído (no un fichero truncado): el `file_type` que se
/// encontró vale.
#[test]
fn el_file_type_se_lee_aunque_la_metadata_no_cabe() {
    let mut datos = cabecera_v3(2);
    kv_file_type(&mut datos, 15);
    // Y después, una cadena que no cabe: la metadata queda incompleta.
    kv_cadena_enorme(&mut datos, "general.architecture", 1_500_000);
    let ruta = fichero_disperso(&datos, 1_600_000);
    assert_eq!(
        verificar::file_type(&ruta),
        Some(15),
        "se perdió el file_type por lo que no se leyó"
    );
    limpia(&ruta);
}

/// La cabecera **sin** `general.file_type` no es `None`: es la cabecera que
/// se pudo leer (lo que no está, no está).
#[test]
fn la_cabecera_sin_file_type_no_es_none() {
    let datos = gguf(
        3,
        &[(
            "general.architecture",
            STRING,
            Valor::Str("qwen3".to_string()),
        )],
    );
    let ruta = fichero(&datos);
    let cab = verificar::cabecera(&ruta).expect("la cabecera se lee");
    assert_eq!(cab.file_type, None, "se inventó un file_type");
    assert_eq!(cab.arquitectura.as_deref(), Some("qwen3"));
    assert_eq!(verificar::file_type(&ruta), None, "file_type no es None");
    limpia(&ruta);
}

/// La cabecera trae también los datos del modelo que van al sidecar (el
/// `arch`, el `block_count` y el modelo base).
#[test]
fn la_cabecera_trae_los_datos_del_modelo() {
    let datos = gguf(
        3,
        &[
            (
                "general.architecture",
                STRING,
                Valor::Str("qwen3".to_string()),
            ),
            ("qwen3.block_count", UINT32, Valor::U32(48)),
            (
                "general.base_model.0.repo_url",
                STRING,
                Valor::Str("acme/modelo".to_string()),
            ),
            (
                "general.base_model.0.name",
                STRING,
                Valor::Str("Modelo".to_string()),
            ),
            ("general.file_type", UINT32, Valor::U32(15)),
        ],
    );
    let ruta = fichero(&datos);
    let cab = verificar::cabecera(&ruta).expect("la cabecera se lee");
    assert_eq!(cab.file_type, Some(15));
    assert_eq!(cab.arquitectura.as_deref(), Some("qwen3"));
    assert_eq!(cab.block_count, Some(48));
    assert_eq!(cab.base_model.as_deref(), Some("acme/modelo"));
    assert_eq!(cab.base_model_name.as_deref(), Some("Modelo"));
    limpia(&ruta);
}

/// Lo que la verificación mínima sigue comprando: que el GGUF existe y su
/// cabecera es la magic (la fase 4, R-GGUF-15.4).
#[test]
fn la_verificación_mínima_sigue_verificando() {
    let mut datos = Vec::new();
    datos.extend_from_slice(b"GGUF");
    datos.extend_from_slice(&vec![0u8; 2048]);
    let ruta = fichero(&datos);
    assert!(verificar::verificar(&ruta).is_ok(), "un GGUF de 2 KiB pasa");
    limpia(&ruta);
}
