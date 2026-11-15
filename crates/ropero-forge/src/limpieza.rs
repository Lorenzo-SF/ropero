//! La **política de limpieza**: qué se borra y qué no, en cada caso.
//!
//! Las reglas son duras y están en el doc (R-GGUF-16/17/18):
//!
//! - **Al fallar la cuantización NO se borra nada**: el clon y el GGUF
//!   intermedio se quedan (R-GGUF-16). Un fallo de cuantización no debe
//!   costar 60 GB de descarga.
//! - **Los pesos sólo se borran si la verificación pasó** (R-GGUF-17): si
//!   el GGUF está corrupto, tener los safetensors vale más que 60 GB.
//! - **El GGUF de entrada jamás se borra** (R-GGUF-17): lo trajo el
//!   usuario.
//! - **El intermedio se borra después de todos los quants** (R-GGUF-18),
//!   y si el F16 **es** el resultado, no se toca.
//!
//! Todo va en **una función pura** a propósito: entra el estado de la
//! tirada, salen los borrados. Así la política se afirma en un test y la
//! fase 5 es sólo ejecutar lo que la política dijo.

use std::path::PathBuf;

/// De qué tipo es la fuente de la tirada: eso decide si el work se puede
/// borrar (un clon propio sí; el directorio de pesos del usuario, no).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    /// Un repo que la tirada descargó: el work es suyo.
    ClonPropio,
    /// Un `--path` del usuario: **no se borra nunca**.
    PesosDelUsuario,
    /// Un `--gguf` de un fichero local: es del usuario.
    GgufDelUsuario,
}

/// El estado de la tirada cuando llega la fase 5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstadoTirada {
    /// Qué clase de fuente fue.
    pub origen: Origen,
    /// Si la verificación de los GGUFs pasó.
    pub verifico: bool,
    /// Si se hizo cuantización de verdad (con el intermedio aparte).
    pub cuantizo: bool,
    /// Si el intermedio es también una salida (`--quant F16`): es el
    /// fichero final, no un intermedio.
    pub intermedio_es_resultado: bool,
    /// Si el intermedio es el GGUF que trajo el usuario.
    pub intermedio_es_entrada: bool,
    /// `--keep-source`.
    pub keep_source: bool,
    /// `--keep-intermedio`.
    pub keep_intermedio: bool,
}

/// Lo que la fase 5 tiene que borrar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ordenes {
    /// Borrar el clon de los pesos (la descarga entera).
    pub borrar_clon: bool,
    /// Borrar el GGUF intermedio (después de todos los quants).
    pub borrar_intermedio: bool,
    /// Las rutas, para los mensajes («conservo los pesos en: …»).
    pub rutas: Vec<PathBuf>,
}

/// La política, en una función pura.
///
/// El mensaje con las rutas («conservo los pesos en: …» / «borro el clon…»)
/// lo compone [`crate::ejecutar`] con lo que aquí se decidió.
pub fn ordenes(estado: &EstadoTirada) -> Ordenes {
    let mut ordenes = Ordenes {
        borrar_clon: false,
        borrar_intermedio: false,
        rutas: Vec::new(),
    };

    // R-GGUF-17: los pesos SÓLO se borran si la verificación pasó. Si el
    // GGUF está corrupto, tener los safetensors vale más que 60 GB.
    if !estado.verifico {
        ordenes.rutas.push(PathBuf::from("conservo los pesos"));
        return ordenes;
    }

    // El clon propio se borra (R-GGUF-17), salvo `--keep-source`. Los
    // pesos del usuario y el GGUF de entrada no se tocan nunca.
    ordenes.borrar_clon = estado.origen == Origen::ClonPropio && !estado.keep_source;

    // R-GGUF-18: el intermedio se borra DESPUÉS de todos los quants (la
    // fase 5 es después, por definición), y no si:
    //   · es el resultado (quant F16);
    //   · es el GGUF que trajo el usuario (R-GGUF-17);
    //   · no hubo cuantización (el intermedio ES el GGUF final);
    //   · `--keep-intermedio`.
    ordenes.borrar_intermedio = estado.cuantizo
        && !estado.intermedio_es_resultado
        && !estado.intermedio_es_entrada
        && !estado.keep_intermedio;

    ordenes
}
