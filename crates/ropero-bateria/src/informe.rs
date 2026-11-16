//! El informe de `ropero test`: cómo respondieron y cómo rindieron.
//!
//! El informe se compone en el dominio (no en la CLI) y sale en dos formas:
//! texto para humanos y JSON para máquinas. Las reglas que vigila este
//! módulo son tres, y las tres son del contrato:
//!
//! - lo que no se puede medir se dice como **no disponible**, no se calla;
//! - lo que es aproximado se marca como **aproximado**;
//! - un test pendiente de juez se marca como pendiente: nunca se le pone
//!   nota.
//!
//! ## El plan de la tirada
//!
//! El informe dice **qué** se lanza, en **qué orden** y con **qué peso**:
//! el orden es el del índice (no el alfabético) y el peso es el que declara
//! la rúbrica. Sin el plan, el resultado no se puede repetir.

use crate::indice::Bateria;
use crate::json::Json;
use crate::puntuadores::{Puntuacion, perfil, puntuacion_de_grupo};

/// Los tokens de una respuesta: de dónde sale el número.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tokens {
    /// El motor los dice (`usage`): el número es real.
    Uso(u64),

    /// No hay `usage`: se cuentan **palabras** y el número es aproximado.
    Aproximado(u64),

    /// No se pudo saber de ninguna manera.
    NoDisponible,
}

/// Las métricas de una respuesta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metrica {
    /// La latencia total, en milisegundos.
    pub latencia_total_ms: u64,

    /// El tiempo hasta el primer token, en milisegundos.
    ///
    /// `None` cuando el motor no dio streaming: no se mide, y no se inventa.
    pub primer_token_ms: Option<u64>,

    /// Los tokens.
    pub tokens: Tokens,

    /// Tokens de entrada que informa el motor.
    pub tokens_entrada: Option<u64>,

    /// Tokens de salida que informa el motor.
    pub tokens_salida: Option<u64>,

    /// Tokens de razonamiento que informa el motor, si los desglosa.
    pub tokens_razonamiento: Option<u64>,

    /// Tokens de entrada reutilizados desde caché, si los desglosa.
    pub tokens_cache: Option<u64>,
}

/// La respuesta de un test: lo que dijo el modelo y lo que se midió.
#[derive(Debug, Clone, PartialEq)]
pub struct RespuestaTest {
    /// El id del test (`F2`).
    pub id: String,

    /// La respuesta cruda del modelo.
    pub salida: String,

    /// Lo que se midió.
    pub metrica: Metrica,

    /// Lo que se pudo puntuar.
    pub puntuacion: Puntuacion,
}

impl RespuestaTest {
    /// La respuesta como JSON: la salida cruda, las métricas y la
    /// puntuación.
    pub fn a_json(&self) -> String {
        self.a_json_valor().a_texto()
    }

    /// La respuesta como valor JSON.
    pub fn a_json_valor(&self) -> Json {
        Json::objeto(vec![
            ("id".to_string(), Json::Str(self.id.clone())),
            ("salida".to_string(), Json::Str(self.salida.clone())),
            ("metrica".to_string(), self.metrica.a_json()),
            ("puntuacion".to_string(), self.puntuacion.a_json()),
        ])
    }
}

impl Metrica {
    /// Las métricas como JSON.
    pub fn a_json(&self) -> Json {
        let (tokens, aproximados) = match self.tokens {
            Tokens::Uso(n) => (Json::Num(n as f64), false),
            Tokens::Aproximado(n) => (Json::Num(n as f64), true),
            Tokens::NoDisponible => (Json::Null, false),
        };
        Json::objeto(vec![
            (
                "latencia_ms".to_string(),
                Json::Num(self.latencia_total_ms as f64),
            ),
            (
                "primer_token_ms".to_string(),
                match self.primer_token_ms {
                    Some(ms) => Json::Num(ms as f64),
                    None => Json::Null,
                },
            ),
            ("tokens".to_string(), tokens),
            ("tokens_aproximados".to_string(), Json::Bool(aproximados)),
            (
                "tokens_entrada".to_string(),
                numero_opcional(self.tokens_entrada),
            ),
            (
                "tokens_salida".to_string(),
                numero_opcional(self.tokens_salida),
            ),
            (
                "tokens_razonamiento".to_string(),
                numero_opcional(self.tokens_razonamiento),
            ),
            (
                "tokens_cache".to_string(),
                numero_opcional(self.tokens_cache),
            ),
        ])
    }

    /// La velocidad, en tokens por segundo.
    ///
    /// `None` cuando no hay tokens o no hay tiempo: sin una de las dos
    /// cosas **no hay velocidad**, y no se inventa.
    pub fn tokens_por_segundo(&self) -> Option<f64> {
        let n = match (self.tokens_salida, self.tokens) {
            (Some(n), _) => n,
            (None, Tokens::Aproximado(n)) => n,
            (None, Tokens::Uso(_) | Tokens::NoDisponible) => return None,
        };
        let decode_ms = self.latencia_total_ms.checked_sub(self.primer_token_ms?)?;
        if decode_ms == 0 {
            return None;
        }
        Some(n as f64 / (decode_ms as f64 / 1000.0))
    }

    /// La velocidad de prefill, si el motor da tokens de entrada y TTFT.
    pub fn entrada_por_segundo(&self) -> Option<f64> {
        let tokens = self.tokens_entrada?;
        let ms = self.primer_token_ms?;
        (ms > 0).then_some(tokens as f64 / (ms as f64 / 1000.0))
    }
}

fn numero_opcional(value: Option<u64>) -> Json {
    value.map_or(Json::Null, |n| Json::Num(n as f64))
}

impl Puntuacion {
    /// La puntuación como JSON.
    ///
    /// Un pendiente **no** tiene `score`: la clave no existe, para que nadie
    /// la lea como un cero.
    pub fn a_json(&self) -> Json {
        match self {
            Puntuacion::Mecanica { score, detalle } => Json::objeto(vec![
                ("tipo".to_string(), Json::Str("mecanica".to_string())),
                ("score".to_string(), Json::Num(*score)),
                ("detalle".to_string(), Json::Str(detalle.clone())),
            ]),
            Puntuacion::Contada { total, detalle } => Json::objeto(vec![
                ("tipo".to_string(), Json::Str("contada".to_string())),
                ("total".to_string(), Json::Num(*total as f64)),
                ("detalle".to_string(), Json::Str(detalle.clone())),
            ]),
            Puntuacion::PendienteDeJuez { motivo } => Json::objeto(vec![
                ("tipo".to_string(), Json::Str("pendiente".to_string())),
                ("motivo".to_string(), Json::Str(motivo.clone())),
            ]),
            Puntuacion::ErrorEjecucion { motivo } => Json::objeto(vec![
                ("tipo".to_string(), Json::Str("error_ejecucion".to_string())),
                ("motivo".to_string(), Json::Str(motivo.clone())),
            ]),
        }
    }
}

/// El informe entero: la tirada y sus resultados.
#[derive(Debug, Clone, PartialEq)]
pub struct Informe {
    /// La batería sobre la que se ha tirado.
    pub bateria: Bateria,

    /// El modelo que se ha probado.
    pub modelo: String,

    /// El motor que lo sirvió.
    pub motor: String,

    /// El nivel de razonamiento con el que se sirvió.
    pub nivel: String,

    /// El puerto en el que se sirvió.
    pub puerto: u16,

    /// Si **ropero** lanzó el motor (o ya estaba en pie).
    pub lanzado: bool,

    /// Las respuestas, en el orden en que se pidieron.
    pub respuestas: Vec<RespuestaTest>,
}

impl Informe {
    /// Un informe nuevo.
    pub fn nuevo(
        bateria: Bateria,
        modelo: &str,
        motor: &str,
        nivel: &str,
        puerto: u16,
        lanzado: bool,
        respuestas: Vec<RespuestaTest>,
    ) -> Informe {
        Informe {
            bateria,
            modelo: modelo.to_string(),
            motor: motor.to_string(),
            nivel: nivel.to_string(),
            puerto,
            lanzado,
            respuestas,
        }
    }

    /// Las puntuaciones, en el orden de las respuestas.
    fn puntuaciones(&self) -> Vec<(String, Puntuacion)> {
        self.respuestas
            .iter()
            .map(|r| (r.id.clone(), r.puntuacion.clone()))
            .collect()
    }

    /// El informe como texto, para humanos.
    pub fn a_texto(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "batería de pruebas · {} · {} · nivel {} · puerto {}\n",
            self.modelo, self.motor, self.nivel, self.puerto
        ));
        if self.lanzado {
            out.push_str(&format!(
                "ropero lanzó el motor y **queda en pie**: páralo con \
                 'ropero stop {}'\n",
                self.modelo
            ));
        } else {
            out.push_str(&format!(
                "el motor ya estaba en pie (no hizo falta ponerlo en marcha): se \
                 puede parar con 'ropero stop {}'\n",
                self.modelo
            ));
        }
        out.push('\n');

        // Los tests, uno por línea.
        for respuesta in &self.respuestas {
            out.push_str(&linea_test(self, respuesta));
            out.push('\n');
        }
        out.push('\n');

        // El perfil por dimensiones (antes que los grupos: el nombre de un
        // grupo puede contener el de una dimensión).
        out.push_str("perfil por dimensiones\n");
        for punto in perfil(&self.bateria, &self.puntuaciones()) {
            let nota = match punto.score {
                Some(score) => format!("{score:.1}"),
                None => "sin nota".to_string(),
            };
            let cola = if punto.parcial {
                " (parcial)".to_string()
            } else {
                String::new()
            };
            out.push_str(&format!("{} · {nota}{cola}\n", punto.nombre));
        }
        out.push('\n');

        // La nota por grupo.
        for grupo in &self.bateria.grupos {
            let p = puntuacion_de_grupo(&self.bateria, grupo.letra, &self.puntuaciones());
            let nota = match p.score {
                Some(score) => format!("{score:.2}"),
                None => "sin nota".to_string(),
            };
            let cola = if p.parcial {
                format!(" (parcial: {} de {})", p.peso_puntuado, p.peso_total)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "Grupo {} · {} · {nota}{cola}\n",
                grupo.letra, grupo.nombre
            ));
        }
        out.push('\n');

        // El plan de la tirada: qué se lanza, en qué orden y con qué peso.
        out.push_str(&format!("plan de la tirada: {}\n", self.plan()));
        out
    }

    /// El plan de la tirada: los tests, en orden, con su peso.
    pub fn plan(&self) -> String {
        self.respuestas
            .iter()
            .map(|r| {
                let peso = self.bateria.test(&r.id).map(|t| t.peso).unwrap_or(0);
                format!("{} (peso {peso})", r.id)
            })
            .collect::<Vec<String>>()
            .join(" → ")
    }

    /// El informe como JSON, para máquinas.
    pub fn a_json(&self) -> String {
        let puntuaciones = self.puntuaciones();
        let grupos: Vec<Json> = self
            .bateria
            .grupos
            .iter()
            .map(|grupo| {
                let p = puntuacion_de_grupo(&self.bateria, grupo.letra, &puntuaciones);
                Json::objeto(vec![
                    ("grupo".to_string(), Json::Str(p.grupo.clone())),
                    ("nombre".to_string(), Json::Str(p.nombre.clone())),
                    (
                        "score".to_string(),
                        match p.score {
                            Some(score) => Json::Num(score),
                            None => Json::Null,
                        },
                    ),
                    ("parcial".to_string(), Json::Bool(p.parcial)),
                    (
                        "peso_puntuado".to_string(),
                        Json::Num(p.peso_puntuado as f64),
                    ),
                    ("peso_total".to_string(), Json::Num(p.peso_total as f64)),
                    (
                        "pendientes".to_string(),
                        Json::Arr(
                            p.pendientes
                                .iter()
                                .map(|id| Json::Str(id.clone()))
                                .collect(),
                        ),
                    ),
                ])
            })
            .collect();
        let dimensiones: Vec<Json> = perfil(&self.bateria, &puntuaciones)
            .into_iter()
            .map(|punto| {
                Json::objeto(vec![
                    ("dimension".to_string(), Json::Str(punto.nombre.clone())),
                    (
                        "score".to_string(),
                        match punto.score {
                            Some(score) => Json::Num(score),
                            None => Json::Null,
                        },
                    ),
                    ("parcial".to_string(), Json::Bool(punto.parcial)),
                    (
                        "peso_puntuado".to_string(),
                        Json::Num(punto.peso_puntuado as f64),
                    ),
                    ("peso_total".to_string(), Json::Num(punto.peso_total as f64)),
                ])
            })
            .collect();
        Json::objeto(vec![
            ("modelo".to_string(), Json::Str(self.modelo.clone())),
            ("motor".to_string(), Json::Str(self.motor.clone())),
            ("nivel".to_string(), Json::Str(self.nivel.clone())),
            ("puerto".to_string(), Json::Num(self.puerto as f64)),
            ("lanzado_por_ropero".to_string(), Json::Bool(self.lanzado)),
            ("plan".to_string(), Json::Str(self.plan())),
            (
                "tests".to_string(),
                Json::Arr(self.respuestas.iter().map(|r| r.a_json_valor()).collect()),
            ),
            ("grupos".to_string(), Json::Arr(grupos)),
            ("perfil".to_string(), Json::Arr(dimensiones)),
        ])
        .a_texto()
    }
}

/// La línea de un test: la nota, el detalle y el rendimiento.
fn linea_test(informe: &Informe, respuesta: &RespuestaTest) -> String {
    let titulo = informe
        .bateria
        .test(&respuesta.id)
        .map(|t| t.titulo.clone())
        .unwrap_or_default();
    let nota = match &respuesta.puntuacion {
        Puntuacion::Mecanica { score, .. } => format!("{score:.2}"),
        Puntuacion::Contada { total, .. } => format!("recuento: {total}"),
        Puntuacion::PendienteDeJuez { .. } => "pendiente de juez".to_string(),
        Puntuacion::ErrorEjecucion { motivo } => format!("error de ejecución: {motivo}"),
    };
    let mut partes = vec![nota];
    partes.push(format!(
        "{:.2} s",
        respuesta.metrica.latencia_total_ms as f64 / 1000.0
    ));
    partes.push(match respuesta.metrica.primer_token_ms {
        Some(ms) => format!("primer token: {:.2} s", ms as f64 / 1000.0),
        None => "primer token: no disponible".to_string(),
    });
    match respuesta.metrica.tokens {
        Tokens::Uso(n) => partes.push(format!("{n} tokens")),
        Tokens::Aproximado(n) => partes.push(format!("≈{n} palabras (aproximado)")),
        Tokens::NoDisponible => partes.push("tokens: no disponible".to_string()),
    }
    for (label, value) in [
        ("entrada", respuesta.metrica.tokens_entrada),
        ("salida", respuesta.metrica.tokens_salida),
        ("razonamiento", respuesta.metrica.tokens_razonamiento),
        ("caché", respuesta.metrica.tokens_cache),
    ] {
        if let Some(value) = value {
            partes.push(format!("tokens {label}: {value}"));
        }
    }
    if let Some(velocidad) = respuesta.metrica.tokens_por_segundo() {
        let unidad = if matches!(respuesta.metrica.tokens, Tokens::Aproximado(_)) {
            "palabras/s"
        } else {
            "tokens salida/s"
        };
        partes.push(format!("{velocidad:.1} {unidad}"));
    } else {
        // Sin tokens o sin tiempo no hay velocidad: se dice, no se calla.
        partes.push("tokens/s: no disponible".to_string());
    }
    if let Some(velocidad) = respuesta.metrica.entrada_por_segundo() {
        partes.push(format!("entrada/TTFT: ≈{velocidad:.1} tokens/s"));
    }
    format!("{} · {} · {}", respuesta.id, titulo, partes.join(" · "))
}
