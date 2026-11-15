//! La **verificación mínima** de un GGUF (la fase 4, R-GGUF-15.4).
//!
//! Comprueba que el GGUF **existe**, que **no está vacío**, que tiene un
//! **tamaño mínimo razonable** y que la **cabecera dice GGUF**. Es
//! deliberadamente **mínima**: la lectura completa de la metadata (los KVs,
//! el `general.file_type`, el base_model…) es la tarea T-33 y queda
//! fuera de esta fase — salvo lo que la cabecera sí dice y sí se necesita:
//! el `general.file_type` (que lo lee [`file_type`], con la lectura
//! acotada) y los datos del modelo que van al sidecar (que lee
//! [`cabecera`]). Lo que esta verificación compra es la decisión de la
//! limpieza (R-GGUF-17): sólo si el GGUF pasa se borran los pesos, y para
//! eso basta saber que el fichero está íntegro por arriba (la cabecera
//! correcta y un tamaño > 0 descartan el truncado a cero bytes, que es el
//! modo de fallo corriente del disco lleno).
//!
//! ## La cabecera: el formato, tal cual
//!
//! Un GGUF es: la magic `GGUF` (4 bytes) · la **versión** (un `u32`: 1, 2 o
//! 3) · `n_tensors` (un `u64`) · `n_kv` (un `u64`) · y luego los pares
//! clave/valor de la metadata:
//!
//! ```text
//! clave   : la longitud (u32 en la v1, u64 desde la v2) + los bytes
//! tipo    : un u32 (la tabla de abajo)
//! valor   : según el tipo
//! ```
//!
//! Los tipos de valor son los de `gguf.h` (y los mismos que lee la
//! referencia del repo, `ropero-code/bin/python/ggufcheck.py`):
//!
//! | tipo | id | valor | tamaño del valor | longitud |
//! |---|--:|---|--:|---|
//! | UINT8 | 0 | u8 | 1 | — |
//! | INT8 | 1 | i8 | 1 | — |
//! | UINT16 | 2 | u16 | 2 | — |
//! | INT16 | 3 | i16 | 2 | — |
//! | UINT32 | 4 | u32 | 4 | u32 (v1) / u64 (v2+) |
//! | INT32 | 5 | i32 | 4 | u32 (v1) / u64 (v2+) |
//! | FLOAT32 | 6 | f32 | 4 | u32 (v1) / u64 (v2+) |
//! | BOOL | 7 | bool | 1 | u32 (v1) / u64 (v2+) |
//! | STRING | 8 | string | 0 | u32 (v1) / u64 (v2+) |
//! | ARRAY | 9 | array | 0 | u32 (v1) / u64 (v2+) (**los elementos**) + u32 (**el tipo del elemento**) |
//! | UINT64 | 10 | u64 | 8 | u32 (v1) / u64 (v2+) |
//! | INT64 | 11 | i64 | 8 | u32 (v1) / u64 (v2+) |
//! | FLOAT64 | 12 | f64 | 8 | u32 (v1) / u64 (v2+) |
//!
//! OJO: la **longitud** de una array es la de sus **elementos** (no la de
//! los bytes), y el **tipo del elemento** va **antes** que ella, en un
//! `u32` que **siempre** es de 32 bits.
//!
//! ## Lo que esta lectura **no** hace
//!
//! - **No inventa**: si el fichero está truncado, si la versión no es 1, 2 o
//!   3, o si el `general.file_type` no está, devuelve `None`. `None` **nunca**
//!   significa «F16»: significa «no se ha podido leer».
//! - **No lee el fichero entero**: como mucho [`LECTURA_METADATA`] bytes
//!   (la metadata está al principio; los GGUFs son de decenas de GB).

use std::io::Read;
use std::path::Path;

/// El tamaño mínimo razonable: la cabecera con sus metadatos mínimos y
/// algo de tensor. Un GGUF de menos de 1 KiB no es un GGUF de modelo.
pub const TAMAÑO_MINIMO: u64 = 1024;

/// La **magic** que lleva la cabecera de todo GGUF.
pub const MAGIC: &[u8; 4] = b"GGUF";

/// Lo máximo que se lee para buscar `general.file_type`: la metadata está al
/// principio, así que un MiB basta y no hace falta abrir el GGUF entero (los
/// GGUF son de decenas de GB).
pub const LECTURA_METADATA: u64 = 1_048_576;

// ── Los tipos de valor de la metadata (los de `gguf.h`) ────────────────────

/// `UINT8`: un entero de 8 bits sin signo.
pub const UINT8: u32 = 0;
/// `INT8`: un entero de 8 bits con signo.
pub const INT8: u32 = 1;
/// `UINT16`: un entero de 16 bits sin signo.
pub const UINT16: u32 = 2;
/// `INT16`: un entero de 16 bits con signo.
pub const INT16: u32 = 3;
/// `UINT32`: un entero de 32 bits sin signo.
pub const UINT32: u32 = 4;
/// `INT32`: un entero de 32 bits con signo.
pub const INT32: u32 = 5;
/// `FLOAT32`: un flotante de 32 bits.
pub const FLOAT32: u32 = 6;
/// `BOOL`: un booleano (un byte).
pub const BOOL: u32 = 7;
/// `STRING`: una cadena (su longitud y sus bytes).
pub const STRING: u32 = 8;
/// `ARRAY`: una lista de valores (el tipo del elemento, su número y los
/// elementos).
pub const ARRAY: u32 = 9;
/// `UINT64`: un entero de 64 bits sin signo.
pub const UINT64: u32 = 10;
/// `INT64`: un entero de 64 bits con signo.
pub const INT64: u32 = 11;
/// `FLOAT64`: un flotante de 64 bits.
pub const FLOAT64: u32 = 12;

/// La profundidad máxima de las arrays anidadas: una array de arrays
/// anidada hasta el infinito no puede colgar la lectura.
const PROFUNDIDAD_MAXIMA: usize = 4;

/// Lo que dice la **cabecera** de un GGUF.
///
/// Es lo que se puede leer de ella sin abrir el fichero entero: la
/// metadata del principio. Lo que no está, **no está**: ningún campo se
/// inventa (R-GGUF-20: no mentir sobre lo que se hizo).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cabecera {
    /// El `general.file_type`: el `llama_ftype` de los pesos (`"Q4_K_M"` =
    /// 15). `None` si el fichero no lo dice.
    pub file_type: Option<u32>,
    /// El `general.architecture`: la arquitectura del modelo (`"qwen3"`).
    pub arquitectura: Option<String>,
    /// El `<arquitectura>.block_count`: los bloques del modelo.
    pub block_count: Option<u64>,
    /// El repo del modelo base (`general.base_model.0.repo_url`): de dónde
    /// vinieron los pesos **sin** cuantizar.
    pub base_model: Option<String>,
    /// El nombre del modelo base (`general.base_model.0.name`).
    pub base_model_name: Option<String>,
    /// El `<arquitectura>.block_count` **con su prefijo**: el bueno es el de
    /// la arquitectura del GGUF, y la arquitectura se sabe al final del
    /// recorrido.
    bloque: Option<(String, u64)>,
}

/// El `general.file_type` del GGUF — el `llama_ftype` de su metadata — o
/// `None` si el fichero no lo dice.
///
/// Es lo que permite saber si un GGUF **ya está** en el quant pedido
/// (R-GGUF-7, R-GGUF-8): la clase se lee de la cabecera, no del nombre, que
/// puede decir BF16 y contener un IQ2_M. Se lee sólo el principio del
/// fichero ([`LECTURA_METADATA`]) y con la profundidad acotada: una metadata
/// corrupta o un tamaño desmesurado no hace leer el fichero entero, y no se
/// inventa un valor (devuelve `None` y quien llama cae al nombre).
pub fn file_type(gguf: &Path) -> Option<u32> {
    cabecera(gguf).and_then(|cab| cab.file_type)
}

/// La **cabecera** del GGUF: lo que dice su metadata.
///
/// Es [`file_type`] y además lo que va al sidecar de procedencia (el
/// `arch`, el `block_count` y el modelo base). `None` si el fichero no se
/// puede leer (no existe, no es un GGUF, está truncado o su versión no es
/// 1, 2 o 3).
pub fn cabecera(gguf: &Path) -> Option<Cabecera> {
    let datos = lee_hasta(gguf, LECTURA_METADATA)?;
    // Si se leyó menos del tope, el fichero entero cabe aquí: que falten
    // bytes es, entonces, que el fichero está **truncado**.
    let entero = (datos.len() as u64) < LECTURA_METADATA;
    if datos.len() < 24 || &datos[..4] != MAGIC {
        return None;
    }
    let version = u32_le(&datos, 4)?;
    if !(1..=3).contains(&version) {
        return None;
    }
    // Las **longitudes** (de las cadenas y de las arrays): de 32 bits en la
    // v1 y de 64 desde la v2. La v3 sigue con 64.
    let longitudes = if version == 1 { 4 } else { 8 };
    // Los **contadores** (`n_tensors` y `n_kv`): de 64 bits desde la v2. La
    // v1 histórica usaba 32, y la cabecera no lo dice: se prueban las dos
    // lecturas y se queda la que cuadra (la de 32 primero, que es la
    // histórica).
    let layouts: Vec<(usize, usize)> = if version == 1 {
        let de_32 = u32_le(&datos, 12).map(|n| (16usize, n as usize));
        let de_64 = u64_le(&datos, 16)
            .and_then(|n| usize::try_from(n).ok())
            .map(|n| (24usize, n));
        de_32.into_iter().chain(de_64).collect()
    } else {
        let n = u64_le(&datos, 16).and_then(|n| usize::try_from(n).ok())?;
        vec![(24usize, n)]
    };
    let mut primera: Option<Cabecera> = None;
    for (inicio, n_kv) in layouts {
        let (cab, completa) = escanea(&datos, inicio, n_kv, longitudes);
        if cab.file_type.is_some() && (completa || !entero) {
            return Some(cab);
        }
        // Faltaban bytes para terminar la metadata. Si el fichero entero
        // cabía en lo leído, está **truncado**: no se inventa. Si el
        // fichero es más largo de lo leído, lo que falta es metadata que no
        // se ha leído (el vocabulario de un modelo grande puede pesar
        // megabytes): con el `file_type` ya encontrado, se queda con él.
        if completa && primera.is_none() {
            primera = Some(cab);
        }
    }
    primera
}

/// Escanea los KVs de la metadata y devuelve lo que se busca.
///
/// El segundo valor del par dice si la metadata **entera** se leyó: si no,
/// faltaban bytes (el fichero está truncado, o su metadata es más larga de
/// lo que se ha leído).
fn escanea(datos: &[u8], inicio: usize, n_kv: usize, longitudes: usize) -> (Cabecera, bool) {
    let mut cab = Cabecera::default();
    let completa = escanea_kvs(datos, inicio, n_kv, longitudes, &mut cab).is_some();
    if let (Some(arch), Some((prefijo, n))) = (&cab.arquitectura, &cab.bloque)
        && prefijo == arch
    {
        cab.block_count = Some(*n);
    }
    (cab, completa)
}

/// El recorrido de los KVs: `None` si la metadata **no** se pudo leer entera.
fn escanea_kvs(
    datos: &[u8],
    inicio: usize,
    n_kv: usize,
    longitudes: usize,
    cab: &mut Cabecera,
) -> Option<()> {
    let mut p = inicio;
    for _ in 0..n_kv {
        let clave = lee_cadena(datos, &mut p, longitudes)?;
        let tipo = lee_u32(datos, &mut p)?;
        if clave == "general.file_type" {
            let valor = lee_valor(datos, &mut p, tipo, longitudes, 0)?;
            cab.file_type = numero_de(&valor);
            continue;
        }
        if clave == "general.architecture" {
            let valor = lee_valor(datos, &mut p, tipo, longitudes, 0)?;
            if let Valor::Texto(texto) = valor {
                cab.arquitectura = Some(texto);
            }
            continue;
        }
        if let Some(prefijo) = clave.strip_suffix(".block_count") {
            let valor = lee_valor(datos, &mut p, tipo, longitudes, 0)?;
            if let Valor::Numero(n) = valor {
                cab.bloque = Some((prefijo.to_string(), n));
            }
            continue;
        }
        if let Some(resto) = clave.strip_prefix("general.base_model.") {
            let valor = lee_valor(datos, &mut p, tipo, longitudes, 0)?;
            // El índice de la array, si lo hay (`general.base_model.0.name`).
            let resto = quita_indice(resto);
            if let Valor::Texto(texto) = valor {
                match resto {
                    "repo_url" => cab.base_model = Some(texto),
                    "name" => cab.base_model_name = Some(texto),
                    _ => {}
                }
            }
            continue;
        }
        // El resto de la metadata no se guarda: se salta (el vocabulario de
        // un modelo grande son 150 000 cadenas).
        salta_valor(datos, &mut p, tipo, longitudes, 0)?;
    }
    Some(())
}

/// El valor de un KV, leído de la cabecera.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Valor {
    /// Un número (un entero, o un flotante que es un entero exacto).
    Numero(u64),
    /// Una cadena.
    Texto(String),
    /// Un valor que no se interpreta (un flotante que no es entero, un
    /// booleano, un tipo que no se sabe leer).
    Otro,
}

/// El número de un valor, si lo es.
///
/// El `general.file_type` es un entero, pero el formato admite que se
/// guarde como un flotante o como una cadena: se lee el número sea cual
/// sea su tipo. Si no hay un número exacto, no hay valor (`None`).
fn numero_de(valor: &Valor) -> Option<u32> {
    match valor {
        Valor::Numero(n) => u32::try_from(*n).ok(),
        Valor::Texto(texto) => texto.trim().parse::<u32>().ok(),
        Valor::Otro => None,
    }
}

/// Quita el índice de una array del nombre de una clave
/// (`general.base_model.0.name` → `name`).
fn quita_indice(resto: &str) -> &str {
    let digitos = resto.len() - resto.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digitos == 0 || digitos == resto.len() {
        return resto;
    }
    match resto[digitos..].strip_prefix('.') {
        Some(sin_indice) => sin_indice,
        None => resto,
    }
}

/// Lee el valor de un KV y deja `p` detrás de él.
///
/// Las arrays se leen por su **primer elemento** (que es lo único que puede
/// interesar) y el resto se salta: la cabecera puede llevar el
/// vocabulario entero y no hace falta guardarlo.
fn lee_valor(
    datos: &[u8],
    p: &mut usize,
    tipo: u32,
    longitudes: usize,
    profundidad: usize,
) -> Option<Valor> {
    match tipo {
        UINT8 => Some(Valor::Numero(lee_u8(datos, p)? as u64)),
        INT8 => Some(entero(lee_i8(datos, p)? as i64)),
        UINT16 => Some(Valor::Numero(lee_u16(datos, p)? as u64)),
        INT16 => Some(entero(lee_i16(datos, p)? as i64)),
        UINT32 => Some(Valor::Numero(lee_u32(datos, p)? as u64)),
        INT32 => Some(entero(lee_i32(datos, p)? as i64)),
        FLOAT32 => Some(flotante(lee_f32(datos, p)? as f64)),
        BOOL => {
            // Un booleano es un byte: no es un número de quant.
            lee_u8(datos, p)?;
            Some(Valor::Otro)
        }
        UINT64 => Some(Valor::Numero(lee_u64(datos, p)?)),
        INT64 => Some(entero(lee_i64(datos, p)?)),
        FLOAT64 => Some(flotante(lee_f64(datos, p)?)),
        STRING => Some(Valor::Texto(lee_cadena(datos, p, longitudes)?)),
        ARRAY => {
            if profundidad >= PROFUNDIDAD_MAXIMA {
                return None;
            }
            // El tipo del elemento va **antes** que su número, y es un
            // u32 **siempre**.
            let tipo_elemento = lee_u32(datos, p)?;
            let n = leer(datos, p, longitudes)?;
            if n == 0 {
                return Some(Valor::Otro);
            }
            let primero = lee_valor(datos, p, tipo_elemento, longitudes, profundidad + 1)?;
            for _ in 1..n {
                salta_valor(datos, p, tipo_elemento, longitudes, profundidad + 1)?;
            }
            Some(primero)
        }
        _ => {
            salta_valor(datos, p, tipo, longitudes, profundidad)?;
            Some(Valor::Otro)
        }
    }
}

/// Salta el valor de un KV. Las arrays se recorren con la **profundidad
/// acotada** (una array de arrays anidada hasta el infinito no puede colgar
/// la lectura).
fn salta_valor(
    datos: &[u8],
    p: &mut usize,
    tipo: u32,
    longitudes: usize,
    profundidad: usize,
) -> Option<()> {
    match tipo {
        UINT8 | INT8 | BOOL => {
            lee_u8(datos, p)?;
        }
        UINT16 | INT16 => {
            lee_u16(datos, p)?;
        }
        UINT32 | INT32 | FLOAT32 => {
            lee_u32(datos, p)?;
        }
        UINT64 | INT64 | FLOAT64 => {
            lee_u64(datos, p)?;
        }
        STRING => {
            let n = leer(datos, p, longitudes)?;
            *p = (*p).checked_add(n)?;
        }
        ARRAY => {
            if profundidad >= PROFUNDIDAD_MAXIMA {
                return None;
            }
            let tipo_elemento = lee_u32(datos, p)?;
            let n = leer(datos, p, longitudes)?;
            for _ in 0..n {
                salta_valor(datos, p, tipo_elemento, longitudes, profundidad + 1)?;
            }
        }
        // Un tipo que no existe en el formato: la metadata no se puede leer.
        _ => return None,
    }
    Some(())
}

/// Un entero **con signo**, como número: sólo si no es negativo (un
/// `file_type` es un número de quant, y un negativo no es ninguno).
fn entero(valor: i64) -> Valor {
    if valor < 0 {
        return Valor::Otro;
    }
    Valor::Numero(valor as u64)
}

/// Un flotante, como número: sólo si es un entero exacto y no negativo (un
/// `file_type` es un entero, y `-1` o `29.5` no son ninguno).
fn flotante(valor: f64) -> Valor {
    if !valor.is_finite() || valor < 0.0 || valor.fract() != 0.0 || valor > u32::MAX as f64 {
        return Valor::Otro;
    }
    Valor::Numero(valor as u64)
}

/// Una cadena de la metadata: su longitud (u32 en la v1, u64 desde la v2) y
/// sus bytes.
fn lee_cadena(datos: &[u8], p: &mut usize, longitudes: usize) -> Option<String> {
    let n = leer(datos, p, longitudes)?;
    let trozo = datos.get(*p..(*p).checked_add(n)?)?;
    *p = (*p).checked_add(n)?;
    Some(String::from_utf8_lossy(trozo).into_owned())
}

/// Un entero (de `ancho` bytes, little-endian) convertido a `usize`.
fn leer(datos: &[u8], p: &mut usize, ancho: usize) -> Option<usize> {
    let valor = if ancho == 8 {
        lee_u64(datos, p)?
    } else {
        lee_u32(datos, p)? as u64
    };
    usize::try_from(valor).ok()
}

/// Los lectores de los valores fijos: cada uno deja `p` detrás del valor.
///
/// Los dos de la cabecera (`u32_le` y `u64_le`) leen **en** una posición, sin
/// moverla: son los contadores, que se leen todos de golpe.
fn u32_le(datos: &[u8], p: usize) -> Option<u32> {
    let trozo: [u8; 4] = datos.get(p..p.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(trozo))
}

fn u64_le(datos: &[u8], p: usize) -> Option<u64> {
    let trozo: [u8; 8] = datos.get(p..p.checked_add(8)?)?.try_into().ok()?;
    Some(u64::from_le_bytes(trozo))
}

fn lee_u8(datos: &[u8], p: &mut usize) -> Option<u8> {
    let valor = *datos.get(*p)?;
    *p = (*p).checked_add(1)?;
    Some(valor)
}

fn lee_i8(datos: &[u8], p: &mut usize) -> Option<i8> {
    let valor = *datos.get(*p)? as i8;
    *p = (*p).checked_add(1)?;
    Some(valor)
}

fn lee_u16(datos: &[u8], p: &mut usize) -> Option<u16> {
    let trozo: [u8; 2] = datos.get(*p..(*p).checked_add(2)?)?.try_into().ok()?;
    *p = (*p).checked_add(2)?;
    Some(u16::from_le_bytes(trozo))
}

fn lee_i16(datos: &[u8], p: &mut usize) -> Option<i16> {
    let trozo: [u8; 2] = datos.get(*p..(*p).checked_add(2)?)?.try_into().ok()?;
    *p = (*p).checked_add(2)?;
    Some(i16::from_le_bytes(trozo))
}

fn lee_u32(datos: &[u8], p: &mut usize) -> Option<u32> {
    let trozo: [u8; 4] = datos.get(*p..(*p).checked_add(4)?)?.try_into().ok()?;
    *p = (*p).checked_add(4)?;
    Some(u32::from_le_bytes(trozo))
}

fn lee_i32(datos: &[u8], p: &mut usize) -> Option<i32> {
    let trozo: [u8; 4] = datos.get(*p..(*p).checked_add(4)?)?.try_into().ok()?;
    *p = (*p).checked_add(4)?;
    Some(i32::from_le_bytes(trozo))
}

fn lee_u64(datos: &[u8], p: &mut usize) -> Option<u64> {
    let trozo: [u8; 8] = datos.get(*p..(*p).checked_add(8)?)?.try_into().ok()?;
    *p = (*p).checked_add(8)?;
    Some(u64::from_le_bytes(trozo))
}

fn lee_i64(datos: &[u8], p: &mut usize) -> Option<i64> {
    let trozo: [u8; 8] = datos.get(*p..(*p).checked_add(8)?)?.try_into().ok()?;
    *p = (*p).checked_add(8)?;
    Some(i64::from_le_bytes(trozo))
}

fn lee_f32(datos: &[u8], p: &mut usize) -> Option<f32> {
    let trozo: [u8; 4] = datos.get(*p..(*p).checked_add(4)?)?.try_into().ok()?;
    *p = (*p).checked_add(4)?;
    Some(f32::from_le_bytes(trozo))
}

fn lee_f64(datos: &[u8], p: &mut usize) -> Option<f64> {
    let trozo: [u8; 8] = datos.get(*p..(*p).checked_add(8)?)?.try_into().ok()?;
    *p = (*p).checked_add(8)?;
    Some(f64::from_le_bytes(trozo))
}

/// Los primeros `máximo` bytes del fichero.
fn lee_hasta(gguf: &Path, máximo: u64) -> Option<Vec<u8>> {
    let fichero = std::fs::File::open(gguf).ok()?;
    let mut datos = Vec::new();
    fichero.take(máximo).read_to_end(&mut datos).ok()?;
    Some(datos)
}

/// Verifica un GGUF: `Ok(())` si pasa, `Err(motivo)` si no.
///
/// El motivo es lo que el error de la fase 4 dice al usuario: por qué no
/// se verificó (y por qué, por tanto, no se borran los pesos).
pub fn verificar(gguf: &Path) -> Result<(), String> {
    let ruta = gguf.to_string_lossy();
    let datos = match std::fs::metadata(gguf) {
        Ok(datos) => datos,
        Err(detalle) => return Err(format!("no está: {detalle}")),
    };
    if !datos.is_file() {
        return Err("no es un fichero".to_string());
    }
    let tamaño = datos.len();
    if tamaño == 0 {
        return Err("está vacío (0 bytes): la conversión o la cuantización \
                    lo dejó a medias"
            .to_string());
    }
    if tamaño < TAMAÑO_MINIMO {
        return Err(format!(
            "es demasiado pequeño ({tamaño} bytes): un GGUF de modelo no \
             baja de {TAMAÑO_MINIMO}"
        ));
    }
    let cabecera = match std::fs::File::open(gguf) {
        Ok(mut fichero) => {
            let mut cabecera = [0u8; 4];
            use std::io::Read;
            match fichero.read_exact(&mut cabecera) {
                Ok(()) => cabecera,
                Err(detalle) => return Err(format!("no se pudo leer la cabecera: {detalle}")),
            }
        }
        Err(detalle) => return Err(format!("no se pudo abrir: {detalle}")),
    };
    if &cabecera != MAGIC {
        return Err(format!(
            "la cabecera no es GGUF (dice {}): el fichero no es un GGUF",
            String::from_utf8_lossy(&cabecera)
        ));
    }
    let _ = ruta;
    Ok(())
}
