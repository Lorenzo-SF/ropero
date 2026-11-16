//! El preflight del arranque y la elección de puerto (doc B §3.3).
//!
//! ## Por qué aquí y no en el runtime
//!
//! Porque son **decisiones**, no lecturas. `pick_port` recibe el rango y
//! los puertos ocupados y devuelve un número: no pregunta al disco quién
//! ocupa qué. `Preflight` recibe lo que la máquina tiene (engine
//! instalado, modelo instalado, VRAM, RAM) y dice si se puede arrancar.
//!
//! La E/S —leer `/proc`, buscar el binario en el `PATH`, listar los
//! puertos en uso— vive en `ropero-runtime`, que es la capa que la monta.
//!
//! ## El preflight acumula razones
//!
//! Un motor incompatible explica **todas** las razones, no la primera
//! (doc B §3.3.2). Lo mismo vale para el arranque: si faltan el engine, el
//! modelo y la VRAM, el mensaje dice las tres. Un usuario que arregla una
//! cosa y vuelve a fallar por la siguiente pierde el viaje.

use crate::manifest::PortRange;

/// Elige un puerto libre del rango de un motor.
///
/// PURA: no pregunta al disco quién ocupa qué —los puertos ocupados llegan
/// como parámetro—. Devuelve el **menor** puerto libre del rango, que es
/// lo que hace que dos motores del mismo rango no se pisen: cada uno coge
/// el primero que ve libre.
///
/// `None` si el rango no tiene ningún puerto libre, o si el rango está
/// **invertido** (`min > max`). Un rango invertido no se "arregla"
/// exchanging los extremos: es un error del manifiesto, y esconderlo
/// sería mentir.
pub fn pick_port(rango: PortRange, ocupados: &[u16]) -> Option<u16> {
    if rango.min > rango.max {
        return None;
    }
    (rango.min..=rango.max).find(|p| !ocupados.contains(p))
}

/// Los recursos que el arranque necesita saber de la máquina.
///
/// Todo llega por parámetro: el core no lee el disco (doc B §3.2.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preflight {
    /// El nombre del motor del modelo (`llama-server`, `vllm`…).
    ///
    /// Va aquí para que el mensaje pueda **nombrarlo**: un "el motor no está
    /// instalado" sin decir cuál no dice qué hacer (P-13).
    pub engine: String,

    /// El engine del modelo está instalado.
    pub engine_installed: bool,

    /// El modelo está descargado.
    pub model_installed: bool,

    /// La VRAM libre, en bytes. `None` si no se pudo leer (o si la máquina
    /// no tiene GPU).
    pub vram_available_bytes: Option<u64>,

    /// La RAM libre, en bytes. `None` si no se pudo leer.
    pub ram_available_bytes: Option<u64>,

    /// El contexto que se ha pedido servir.
    pub context_requested: u32,

    /// La VRAM que el modelo necesita, en bytes. `None` si no se sabe.
    pub vram_required_bytes: Option<u64>,

    /// La RAM que el modelo necesita, en bytes. `None` si no se sabe.
    pub ram_required_bytes: Option<u64>,

    /// Si el arranque es sólo por CPU (`--cpu`): entonces la VRAM no se
    /// mira y lo que cuenta es la RAM.
    pub cpu: bool,
}

/// El error del preflight: las razones por las que no se puede arrancar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightError {
    /// **Todas** las razones, acumuladas (doc B §3.3.2).
    Reasons(Vec<String>),
}

impl PreflightError {
    /// Las razones, una por una.
    pub fn reasons(&self) -> &[String] {
        match self {
            Self::Reasons(reasons) => reasons,
        }
    }
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self::Reasons(reasons) = self;
        if reasons.is_empty() {
            return write!(f, "no se puede arrancar el modelo");
        }
        writeln!(f, "no se puede arrancar el modelo:")?;
        for razon in reasons {
            writeln!(f, "  · {razon}")?;
        }
        Ok(())
    }
}

impl std::error::Error for PreflightError {}

impl Preflight {
    /// Comprueba si se puede arrancar, acumulando **todas** las razones.
    ///
    /// Ok si se cumple todo.
    pub fn check(&self) -> Result<(), PreflightError> {
        let mut reasons: Vec<String> = Vec::new();

        if !self.engine_installed {
            reasons.push(format!(
                "el motor '{}' no está instalado: instálalo con 'ropero install engines'",
                self.engine
            ));
        }
        if !self.model_installed {
            reasons.push(
                "el modelo no está descargado: instálalo con 'ropero install models'".to_string(),
            );
        }

        // Los recursos: sólo se comprueban los que se **saben**. Un dato
        // que no se pudo leer (`None`) no bloquea: lo que no se puede leer
        // no se inventa (doc B §3.2.3).
        // CPU-only loads declare only RAM. GPU loads may also declare a
        // minimum host-RAM requirement for weights/offload, so check both
        // dimensions when the model supplies both estimates.
        self.comprueba_recurso(
            &mut reasons,
            "VRAM",
            self.vram_available_bytes,
            self.vram_required_bytes,
        );
        self.comprueba_recurso(
            &mut reasons,
            "RAM",
            self.ram_available_bytes,
            self.ram_required_bytes,
        );

        if reasons.is_empty() {
            Ok(())
        } else {
            Err(PreflightError::Reasons(reasons))
        }
    }

    /// Comprueba un recurso: disponible contra requerido.
    fn comprueba_recurso(
        &self,
        reasons: &mut Vec<String>,
        nombre: &str,
        disponible: Option<u64>,
        requerido: Option<u64>,
    ) {
        let (Some(disponible), Some(requerido)) = (disponible, requerido) else {
            return;
        };
        if disponible >= requerido {
            return;
        }
        reasons.push(format!(
            "no hay {nombre} suficiente: hay {} y hacen falta {}; \
             usa '--cpu' para servirlo desde la RAM, o descarga otro modelo",
            gb(disponible),
            gb(requerido)
        ));
    }
}

/// Los bytes en GiB, con un decimal: los números que se enseñan son de
/// memoria, no de bytes.
fn gb(bytes: u64) -> String {
    let gib = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    format!("{gib:.1} GB")
}
