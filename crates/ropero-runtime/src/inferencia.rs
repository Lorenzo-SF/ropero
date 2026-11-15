//! La inferencia contra el motor: el `POST`, las métricas y el guardado.
//!
//! ## Qué hace este módulo
//!
//! 1. **Reutiliza el motor** si ya está en pie (su puerto sale del estado
//!    en disco) y lo **arranca** si no lo está. Cuando lo arranca, lo deja
//!    en pie: el informe dice cómo pararlo.
//! 2. Manda cada test como una **pregunta independiente**: el contenido
//!    **íntegro** del fichero del test, en una sesión limpia.
//! 3. **Mide** lo que se puede medir: la latencia total, el tiempo hasta el
//!    primer token (si el motor da streaming) y los tokens (del campo
//!    `usage` si lo da; si no, se cuentan palabras y se dice que es
//!    aproximado). Lo que no se puede medir se marca como **no disponible**:
//!    nunca se inventa una cifra.
//! 4. **Guarda** la salida de cada test bajo el estado de ropero
//!    (`<estado>/tests/<marca>/<id>.json`) para que un juez la revise.
//!
//! ## La clave de API
//!
//! La clave va en la cabecera `Authorization: Bearer`, **nunca** en el
//! `argv` de otro proceso ni en la salida. Si el motor la pide (responde
//! 401) y hay una en el entorno (`ROPERO_API_KEY`), se reintenta con ella;
//! si no hay, el error dice dónde ponerla.

use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use ropero_bateria::indice::{Bateria, parse_indice};
use ropero_bateria::informe::{Informe, Metrica, RespuestaTest, Tokens};
use ropero_bateria::json::{self, Json};
use ropero_bateria::puntuadores::puntua;
use ropero_bateria::seleccion::{Seleccion, selecciona};
use ropero_core::{Catalog, EngineManifest, Environment};

use crate::launch::now_epoch;
use crate::probe::abre_peticion;
use crate::run::{RunOutcome, RunRequest};
use crate::state::read_state;

/// Cuánto se espera a cada lectura del motor.
const TIMEOUT_DE_LECTURA: Duration = Duration::from_secs(600);

/// Un error de la tirada: qué pasó y qué código de salida lleva.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorInferencia {
    /// El mensaje: dice qué pasó y qué hacer.
    pub mensaje: String,

    /// El código de salida: 1 (error de ejecución) o 2 (uso).
    pub codigo: i32,
}

impl ErrorInferencia {
    /// Un error de ejecución (código 1).
    pub fn ejecucion(mensaje: impl Into<String>) -> Self {
        Self {
            mensaje: mensaje.into(),
            codigo: 1,
        }
    }

    /// Un error de uso (código 2).
    pub fn uso(mensaje: impl Into<String>) -> Self {
        Self {
            mensaje: mensaje.into(),
            codigo: 2,
        }
    }

    /// Informa del error: lo escribe en stderr (todo lo humano va allí) y
    /// devuelve su código de salida.
    pub fn informa(&self) -> i32 {
        ropero_core::err_line(&format!("error: {}", self.mensaje));
        self.codigo
    }
}

impl std::fmt::Display for ErrorInferencia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.mensaje)
    }
}

impl std::error::Error for ErrorInferencia {}

/// La respuesta del motor a una pregunta, con sus métricas.
#[derive(Debug, Clone, PartialEq)]
pub struct Respuesta {
    /// Lo que dijo el modelo.
    pub texto: String,

    /// Lo que se midió.
    pub metrica: Metrica,
}

/// El motor que sirve el modelo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotorEnPie {
    /// El puerto en el que sirve.
    pub puerto: u16,

    /// El motor que lo sirve.
    pub motor: String,

    /// El nivel de razonamiento con el que se arrancó.
    pub nivel: String,

    /// Si **ropero** lo arrancó (o ya estaba en pie).
    pub lanzado: bool,
}

/// El motor del modelo, si ya está en pie.
///
/// El puerto sale del **estado en disco**: es la fuente de verdad de lo que
/// ropero arrancó. Un motor cuyo proceso ya no existe no está en pie.
pub fn motor_de(modelo: &str, state_dir: &Path) -> Option<MotorEnPie> {
    for puerto in crate::stop::SysStopper::puertos_de_modelo(state_dir, modelo) {
        let Some(estado) = read_state(state_dir, puerto) else {
            continue;
        };
        if !sigue_vivo(estado.pid) {
            continue;
        }
        return Some(MotorEnPie {
            puerto,
            motor: estado.engine.clone(),
            nivel: estado.level.clone(),
            lanzado: false,
        });
    }
    None
}

/// Si el proceso sigue vivo (y no es un zombi).
fn sigue_vivo(pid: Option<u32>) -> bool {
    match pid {
        Some(pid) => crate::process::estado_del_proceso(pid).is_some_and(|estado| estado != 'Z'),
        None => false,
    }
}

/// Manda una pregunta al motor y devuelve lo que dijo, con sus métricas.
///
/// El `prompt` se manda **íntegro**: es el fichero del test, sin recortes.
pub fn pregunta(
    puerto: u16,
    modelo: &str,
    prompt: &str,
    api_key: Option<&str>,
) -> Result<Respuesta, ErrorInferencia> {
    let cuerpo = json::cuerpo_inferencia(modelo, prompt, true);
    let (estado, texto, primer_token_ms, latencia_ms) = manda(puerto, &cuerpo, api_key)?;
    if estado == 401 || estado == 403 {
        return Err(ErrorInferencia::ejecucion(
            "el motor pide una clave de API: ponla en ROPERO_API_KEY (no va en \
             la línea de órdenes) y vuelve a tirar",
        ));
    }
    if !(200..300).contains(&estado) {
        return Err(ErrorInferencia::ejecucion(format!(
            "el motor respondió {estado} a la pregunta: {}",
            resumen(&texto)
        )));
    }
    let (contenido, tokens) = parsea_respuesta(&texto);
    if contenido.trim().is_empty() {
        return Err(ErrorInferencia::ejecucion(format!(
            "el motor no devolvió texto: {}",
            resumen(&texto)
        )));
    }
    // Los tokens: los del motor si los da; si no, se cuentan palabras y se
    // dice que el recuento es aproximado.
    let tokens = match tokens {
        Some(n) => Tokens::Uso(n),
        None => Tokens::Aproximado(contenido.split_whitespace().count() as u64),
    };
    Ok(Respuesta {
        metrica: Metrica {
            latencia_total_ms: latencia_ms,
            primer_token_ms,
            tokens,
        },
        texto: contenido,
    })
}

/// Manda el `POST` y devuelve el código de estado, el cuerpo y las medidas.
///
/// El `primer_token_ms` se mide **mientras** se lee: es el instante en que
/// llega el primer trozo con contenido, no el final de la respuesta.
fn manda(
    puerto: u16,
    cuerpo: &str,
    api_key: Option<&str>,
) -> Result<(u16, String, Option<u64>, u64), ErrorInferencia> {
    // El reloj arranca antes de conectar: la latencia total es la que ve
    // el usuario, conexión incluida.
    let inicio = Instant::now();
    let mut stream = abre_peticion(
        puerto,
        "POST",
        "/v1/chat/completions",
        Some(cuerpo),
        TIMEOUT_DE_LECTURA,
        api_key,
    )
    .ok_or_else(|| {
        ErrorInferencia::ejecucion(format!(
            "no se pudo conectar con el motor en el puerto {puerto}: ¿está en pie?"
        ))
    })?;

    let mut acumulado = Vec::new();
    let mut primer_token_ms: Option<u64> = None;
    let mut buffer = [0u8; 8192];
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                acumulado.extend_from_slice(&buffer[..n]);
                if primer_token_ms.is_none() {
                    let texto = String::from_utf8_lossy(&acumulado);
                    if trae_contenido(&texto) {
                        primer_token_ms = Some(inicio.elapsed().as_millis() as u64);
                    }
                }
                if acumulado.len() > 8_000_000 {
                    break;
                }
            }
            Err(e) => {
                // Si ya llegó contenido, el corte es del motor: se sigue con
                // lo que hay. Si no, no hay respuesta que puntuar.
                let texto = String::from_utf8_lossy(&acumulado);
                if !trae_contenido(&texto) {
                    return Err(ErrorInferencia::ejecucion(format!(
                        "la lectura del motor se cortó ({e}) sin que llegara nada"
                    )));
                }
                break;
            }
        }
    }
    let latencia = inicio.elapsed().as_millis() as u64;
    let texto = String::from_utf8_lossy(&acumulado).into_owned();
    let (estado, cuerpo) = match texto.split_once("\r\n\r\n") {
        Some((cabecera, cuerpo)) => (estado_de(cabecera), cuerpo.to_string()),
        None => (0, texto.clone()),
    };
    Ok((estado, cuerpo, primer_token_ms, latencia))
}

/// El código de estado de la línea de estado de la respuesta.
fn estado_de(cabecera: &str) -> u16 {
    cabecera
        .lines()
        .next()
        .and_then(|linea| linea.split_whitespace().nth(1))
        .and_then(|codigo| codigo.parse::<u16>().ok())
        .unwrap_or(0)
}

/// Si el texto ya trae contenido del modelo (y no sólo el andamiaje).
///
/// Sólo cuenta el *streaming*: una respuesta entera llega de golpe y no
/// tiene «primer token» (eso se dice como **no disponible**).
fn trae_contenido(texto: &str) -> bool {
    if !texto.contains("\"delta\"") {
        return false;
    }
    let mut resto = texto;
    while let Some(pos) = resto.find("\"content\"") {
        let despues = &resto[pos + 9..];
        let Some(colon) = despues.find(':') else {
            break;
        };
        let valor = despues[colon + 1..].trim_start();
        if let Some(inicio) = valor.strip_prefix('"')
            && !inicio.is_empty()
            && !inicio.starts_with('"')
        {
            return true;
        }
        resto = &despues[colon + 1..];
    }
    false
}

/// El texto del modelo y los tokens, de una respuesta del motor.
///
/// Se aceptan las dos formas: el *streaming* (líneas `data: …`) y la
/// respuesta entera. Los tokens sólo se dan si el motor manda `usage`: si no,
/// `None` (y quien llama cuenta palabras y lo dice).
fn parsea_respuesta(cuerpo: &str) -> (String, Option<u64>) {
    if cuerpo.contains("data:") {
        let mut texto = String::new();
        let mut tokens = None;
        for linea in cuerpo.lines() {
            let Some(datos) = linea.strip_prefix("data:") else {
                continue;
            };
            let datos = datos.trim();
            if datos.is_empty() || datos == "[DONE]" {
                continue;
            }
            let Ok(json) = json::parse(datos) else {
                continue;
            };
            if let Some(total) = tokens_de(&json) {
                tokens = Some(total);
            }
            if let Some(contenido) = contenido_de(&json) {
                texto.push_str(&contenido);
            }
        }
        return (texto, tokens);
    }
    let Ok(json) = json::parse(cuerpo) else {
        return (String::new(), None);
    };
    (contenido_de(&json).unwrap_or_default(), tokens_de(&json))
}

/// El contenido de una respuesta del motor.
fn contenido_de(json: &Json) -> Option<String> {
    let choices = json.get("choices").and_then(Json::arr)?;
    for choice in choices {
        if let Some(delta) = choice
            .get("delta")
            .and_then(|d| d.get("content"))
            .and_then(Json::as_str)
        {
            return Some(delta.to_string());
        }
        if let Some(mensaje) = choice
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Json::as_str)
        {
            return Some(mensaje.to_string());
        }
    }
    None
}

/// Los tokens que dice el motor (`usage`).
fn tokens_de(json: &Json) -> Option<u64> {
    let usage = json.get("usage")?;
    usage
        .get("total_tokens")
        .and_then(Json::as_num)
        .map(|n| n as u64)
}

/// Un resumen corto de un cuerpo de respuesta, para los errores.
fn resumen(texto: &str) -> String {
    let limpio = texto.trim();
    if limpio.is_empty() {
        return "cuerpo vacío".to_string();
    }
    let corto: String = limpio.chars().take(200).collect();
    format!("'{corto}'")
}

// ─────────────────────────────────────────────────────────────────────────────
// La tirada completa
// ─────────────────────────────────────────────────────────────────────────────

/// El modelo que se prueba: el pedido, o el que se elige del menú.
///
/// Sin `--modelo` se muestran los **disponibles** (motor instalado y
/// modelo descargado) y se espera selección; sin terminal eso es un **error
/// de uso** (código 2) que dice qué hacer.
pub fn modelo_de(
    catalogo: &Catalog,
    motores: &[EngineManifest],
    env: &Environment,
    pedido: Option<String>,
) -> Result<String, ErrorInferencia> {
    let Some(modelo) = pedido else {
        return elegir_modelo(catalogo, motores, env);
    };
    if ropero_core::find_spec(catalogo, &modelo).is_none() {
        return Err(ErrorInferencia::uso(format!(
            "modelo desconocido: '{modelo}' (ropero list → los del catálogo)"
        )));
    }
    Ok(modelo)
}

/// Elige el modelo de los **disponibles**: motor instalado y modelo
/// descargado (doc CLI v2 §13).
///
/// Sin terminal es un **error de uso** (código 2) que dice qué hacer: un
/// menú que se traga un EOF acabaría eligiendo por su cuenta.
pub fn elegir_modelo(
    catalogo: &Catalog,
    motores: &[EngineManifest],
    env: &Environment,
) -> Result<String, ErrorInferencia> {
    if !crate::install::hay_tty() {
        return Err(ErrorInferencia::uso(
            "sin terminal no se puede elegir el modelo: usa \
             'ropero test --modelo <id>' (ropero list → los modelos disponibles)",
        ));
    }
    let detector = crate::Detector { env };
    let modelos: Vec<ropero_core::ModelSpec> = catalogo
        .models
        .iter()
        .chain(catalogo.fired.iter())
        .cloned()
        .collect();
    let foto = ropero_core::snapshot::build_snapshot(motores, &modelos, &detector);
    let disponibles = ropero_core::snapshot::available_models(&foto);
    if disponibles.is_empty() {
        return Err(ErrorInferencia::uso(
            "no hay modelos disponibles (motor instalado y modelo descargado): \
             'ropero list' → los que hay, 'ropero install' → los que faltan",
        ));
    }
    let nombres: Vec<String> = disponibles
        .iter()
        .map(|m| m.spec.model.name.clone())
        .collect();
    for (i, nombre) in nombres.iter().enumerate() {
        ropero_core::err_line(&format!("  {}. {nombre}", i + 1));
    }
    let respuesta = crate::install::pregunta("elige el modelo que se prueba: ")
        .ok_or_else(|| ErrorInferencia::uso("no hay respuesta: se cancela"))?;
    let elegidos = ropero_core::install::resuelve_seleccion(&respuesta, &nombres)
        .map_err(|e| ErrorInferencia::uso(e.to_string()))?;
    elegidos
        .into_iter()
        .next()
        .ok_or_else(|| ErrorInferencia::uso("no se eligió ningún modelo"))
}

/// El comando `ropero test`, de principio a fin.
///
/// El orden importa: primero se resuelve **qué** se lanza (si el grupo o el
/// test no existen, es un error de uso y no se toca el motor), después se
/// elige el modelo y sólo entonces se tira.
///
/// Devuelve el código de salida.
pub fn tira(
    catalogo: &Catalog,
    motores: &[EngineManifest],
    env: &Environment,
    root: &Path,
    modelo: Option<String>,
    seleccion: &Seleccion,
    json: bool,
) -> i32 {
    let bateria = match lee_indice(root) {
        Ok(bateria) => bateria,
        Err(e) => return e.informa(),
    };
    let ids = match selecciona(&bateria, seleccion) {
        Ok(ids) => ids,
        Err(e) => return ErrorInferencia::uso(e.mensaje).informa(),
    };
    let modelo = match modelo_de(catalogo, motores, env, modelo) {
        Ok(modelo) => modelo,
        Err(e) => return e.informa(),
    };
    match tira_la_bateria(catalogo, motores, env, root, &bateria, &ids, &modelo) {
        Ok(informe) => {
            imprime(&informe, json);
            0
        }
        Err(e) => e.informa(),
    }
}

/// Lanza la batería contra un modelo y devuelve el informe.
///
/// `ids` son los tests que se lanzan, **en el orden del índice**. Cada test
/// va en una pregunta independiente: el contenido íntegro del fichero del
/// test, sin contexto previo.
pub fn tira_la_bateria(
    catalogo: &Catalog,
    motores: &[EngineManifest],
    env: &Environment,
    root: &Path,
    bateria: &Bateria,
    ids: &[String],
    modelo: &str,
) -> Result<Informe, ErrorInferencia> {
    let state_dir = crate::state::state_dir(&env.home, env.get("ROPERO_STATE_DIR"));
    let motor = match motor_de(modelo, &state_dir) {
        Some(motor) => motor,
        None => arranca_motor(catalogo, motores, env, &state_dir, modelo)?,
    };

    let api_key = env.get("ROPERO_API_KEY");
    let marca = now_epoch().to_string();
    let mut respuestas = Vec::with_capacity(ids.len());
    for id in ids {
        let test = bateria
            .test(id.as_str())
            .ok_or_else(|| ErrorInferencia::uso(format!("el test '{id}' no está en la batería")))?;
        let prompt = lee_test(root, &test.fichero)?;
        let respuesta = match pregunta(motor.puerto, modelo, &prompt, api_key) {
            Ok(respuesta) => respuesta,
            Err(e) => {
                // Un test que falla no puede dejar la tirada a medias: se
                // guarda igual, con la salida vacía y el motivo.
                let vacia = RespuestaTest {
                    id: id.clone(),
                    salida: String::new(),
                    metrica: Metrica {
                        latencia_total_ms: 0,
                        primer_token_ms: None,
                        tokens: Tokens::NoDisponible,
                    },
                    puntuacion: ropero_bateria::puntuadores::Puntuacion::PendienteDeJuez {
                        motivo: format!("el test '{id}' no se pudo lanzar: {}", e.mensaje),
                    },
                };
                respuestas.push(vacia);
                continue;
            }
        };
        let puntuacion = puntua(id.as_str(), &respuesta.texto);
        let metrica = respuesta.metrica;
        let respuesta_test = RespuestaTest {
            id: id.clone(),
            salida: respuesta.texto,
            metrica,
            puntuacion,
        };
        guarda(&state_dir, &marca, &respuesta_test);
        respuestas.push(respuesta_test);
    }

    Ok(Informe::nuevo(
        bateria.clone(),
        modelo,
        &motor.motor,
        &motor.nivel,
        motor.puerto,
        motor.lanzado,
        respuestas,
    ))
}

/// Lee el índice de la batería.
fn lee_indice(root: &Path) -> Result<Bateria, ErrorInferencia> {
    let ruta = root.join("docs").join("tests").join("indice.md");
    let texto = std::fs::read_to_string(&ruta).map_err(|e| {
        ErrorInferencia::ejecucion(format!(
            "no se pudo leer el índice de la batería ({}): {e}",
            ruta.display()
        ))
    })?;
    parse_indice(&texto)
        .map_err(|e| ErrorInferencia::ejecucion(format!("el índice de la batería no cuadra: {e}")))
}

/// Lee el fichero de un test: es lo que se manda al modelo.
fn lee_test(root: &Path, fichero: &str) -> Result<String, ErrorInferencia> {
    let ruta = root.join("docs").join("tests").join(fichero);
    std::fs::read_to_string(&ruta).map_err(|e| {
        ErrorInferencia::ejecucion(format!("no se pudo leer el test ({}): {e}", ruta.display()))
    })
}

/// Guarda el resultado de un test bajo el estado de ropero.
fn guarda(state_dir: &Path, marca: &str, respuesta: &RespuestaTest) {
    let dir = state_dir.join("tests").join(marca);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let ruta = dir.join(format!("{}.json", respuesta.id));
    let _ = std::fs::write(ruta, respuesta.a_json());
}

/// Imprime el informe: el JSON son los **datos** (stdout) y el texto es lo
/// humano (stderr).
pub fn imprime(informe: &Informe, json: bool) {
    if json {
        ropero_core::out_line(&informe.a_json());
        ropero_core::err_line(informe.a_texto().trim_end());
    } else {
        ropero_core::out_line(informe.a_texto().trim_end());
    }
}

/// Arranca el motor del modelo y lo deja en pie.
///
/// El motor se lanza en su propio grupo de procesos: cuando ropero sale, el
/// motor sigue sirviendo. Por eso el informe dice cómo pararlo.
fn arranca_motor(
    catalogo: &Catalog,
    motores: &[EngineManifest],
    env: &Environment,
    state_dir: &Path,
    modelo: &str,
) -> Result<MotorEnPie, ErrorInferencia> {
    let peticion = RunRequest {
        model: modelo.to_string(),
        ..RunRequest::default()
    };
    let plan = crate::run::preparar(catalogo, motores, env, &peticion)
        .map_err(|o: RunOutcome| ErrorInferencia::ejecucion(mensaje_de(&o)))?;
    let outcome = crate::run::arranca(&plan, &peticion, env, state_dir, false, |_puerto, _pid| {});
    if outcome.code != 0 {
        return Err(ErrorInferencia::ejecucion(mensaje_de(&outcome)));
    }
    let Some(puerto) = outcome.port else {
        return Err(ErrorInferencia::ejecucion(format!(
            "el motor de '{modelo}' arrancó pero no se sabe en qué puerto"
        )));
    };
    Ok(MotorEnPie {
        puerto,
        motor: plan.engine.id.as_str().to_string(),
        nivel: plan.level.clone(),
        lanzado: true,
    })
}

/// El mensaje de un arranque fallido.
fn mensaje_de(outcome: &RunOutcome) -> String {
    outcome
        .error
        .clone()
        .unwrap_or_else(|| format!("no se pudo arrancar el motor (código {})", outcome.code))
}
