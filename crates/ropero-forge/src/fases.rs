//! Las **seis fases** de la forja (R-GGUF-15) y su **checkpoint**.
//!
//! La tirada es una máquina de estados, no un script: cada fase deja su
//! checkpoint en el directorio de trabajo, y al reanudar se continúa por
//! la fase siguiente a la última completada (T-30). Lo que hace que una
//! tirada cara no se repita de gratis: si la cuantización falla a las tres
//! de la madrugada, la descarga (una hora, 60 GB) y la conversión **no se
//! repiten**.
//!
//! El checkpoint vive en el módulo puro y su fichero en el directorio de
//! trabajo (lo escribe [`crate::ejecutar`]): es un JSON mínimo, a mano,
//! como el resto de la forja.

/// Una fase de la tirada, **en orden fijo**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    /// 1. Descargar/clonar el repo (o usar el GGUF traído).
    Fuente,
    /// 2. Convertir a GGUF (o cuantizar el GGUF existente).
    Conversion,
    /// 3. Cuantizar (imatrix → cuantizador), con un log aparte.
    Cuantizacion,
    /// 4. Verificar los GGUFs.
    Verificacion,
    /// 5. Limpiar.
    Limpieza,
    /// 6. Escribir la procedencia.
    Procedencia,
}

impl Fase {
    /// Las seis, **en su orden**.
    pub const TODAS: [Fase; 6] = [
        Fase::Fuente,
        Fase::Conversion,
        Fase::Cuantizacion,
        Fase::Verificacion,
        Fase::Limpieza,
        Fase::Procedencia,
    ];

    /// Su posición en la tirada, desde 0.
    pub fn orden(self) -> u8 {
        match self {
            Fase::Fuente => 0,
            Fase::Conversion => 1,
            Fase::Cuantizacion => 2,
            Fase::Verificacion => 3,
            Fase::Limpieza => 4,
            Fase::Procedencia => 5,
        }
    }

    /// La fase, por su posición (el valor que guarda el checkpoint).
    pub fn de_orden(orden: u8) -> Option<Fase> {
        Fase::TODAS.get(orden as usize).copied()
    }

    /// La fase siguiente, si la hay.
    pub fn siguiente(self) -> Option<Fase> {
        Fase::de_orden(self.orden() + 1)
    }

    /// Su nombre, para los mensajes y el checkpoint.
    pub fn nombre(self) -> &'static str {
        match self {
            Fase::Fuente => "fuente",
            Fase::Conversion => "conversion",
            Fase::Cuantizacion => "cuantizacion",
            Fase::Verificacion => "verificacion",
            Fase::Limpieza => "limpieza",
            Fase::Procedencia => "procedencia",
        }
    }
}

/// El checkpoint de una tirada: la **última fase completada**.
///
/// Al reanudar, la tirada empieza por la fase siguiente a la guardada.
/// Un checkpoint que dice `Conversion` significa: la fuente y la
/// conversión ya están, sigue por la cuantización.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    /// La última fase completada.
    pub fase: Fase,
}

impl Checkpoint {
    /// El checkpoint de una fase completada.
    pub fn de(fase: Fase) -> Self {
        Checkpoint { fase }
    }

    /// El **estado** del checkpoint, tal y como va al fichero (JSON
    /// mínimo, escrito a mano como el sidecar de procedencia).
    pub fn a_json(self) -> String {
        format!("{{\n  \"fase\": \"{}\"\n}}\n", self.fase.nombre())
    }

    /// El checkpoint que hay en un fichero de estado.
    ///
    /// Un fichero que no se puede leer o que no dice una fase de las seis
    /// **no es un checkpoint**: la tirada empieza de cero (y eso es lo
    /// seguro: reanudar con un estado medio entendido borra o salta cosas
    /// que no toca).
    pub fn de_json(texto: &str) -> Option<Self> {
        let fase = texto
            .split('"')
            .skip_while(|trozo| *trozo != "fase")
            .nth(2)?;
        let fase = fase.trim().trim_matches(|c| c == ',' || c == ' ');
        let fase = Fase::TODAS.iter().find(|f| f.nombre() == fase)?;
        Some(Checkpoint { fase: *fase })
    }
}
