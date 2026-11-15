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

/// El checkpoint de una tirada: la **última fase completada**, **de qué
/// petición** es y **qué cuantizó** la fase 3.
///
/// Al reanudar, la tirada empieza por la fase siguiente a la guardada.
/// Un checkpoint que dice `Conversion` significa: la fuente y la
/// conversión ya están, sigue por la cuantización.
///
/// La **huella** es lo que hace que el checkpoint sea de *esta* tirada: sin
/// ella, una petición distinta sobre la misma fuente reanuda con el
/// checkpoint de la anterior y anuncia un GGUF que no existe (T-30). Es el
/// hash de la forma canónica de la petición
/// ([`crate::peticion::Peticion::huella`]), no su texto: el JSON no se
/// rompe con las barras o comillas de una ruta.
///
/// Los **quants cuantizados** son lo que la fase 3 hizo de verdad: sin ellos,
/// una tirada reanudada diría «no se cuantizó» de algo que sí se cuantizó, y
/// una tirada cuyas salidas ya estaban diría «se cuantizó» sin que el
/// cuantizador llegara a correr (R-GGUF-20: no mentir sobre el resultado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checkpoint {
    /// La última fase completada.
    pub fase: Fase,
    /// La huella de la petición que completó esa fase (vacía si no se sabe).
    pub huella: String,
    /// Los quants que la fase 3 **cuantizó** de verdad.
    pub cuantizados: Vec<String>,
}

impl Checkpoint {
    /// El checkpoint de una fase completada, **sin huella** (el formato viejo:
    /// no se sabe de qué petición es, y por eso no reanuda).
    pub fn de(fase: Fase) -> Self {
        Checkpoint {
            fase,
            huella: String::new(),
            cuantizados: Vec::new(),
        }
    }

    /// El checkpoint de una fase completada **por esta petición**.
    pub fn con(fase: Fase, huella: &str) -> Self {
        Checkpoint {
            fase,
            huella: huella.to_string(),
            cuantizados: Vec::new(),
        }
    }

    /// El checkpoint de una fase completada **por esta petición**, con lo que
    /// la fase 3 cuantizó de verdad.
    pub fn con_cuantizados(fase: Fase, huella: &str, cuantizados: &[String]) -> Self {
        Checkpoint {
            fase,
            huella: huella.to_string(),
            cuantizados: cuantizados.to_vec(),
        }
    }

    /// Si el checkpoint es de la petición con esa huella.
    pub fn es_de(&self, huella: &str) -> bool {
        !self.huella.is_empty() && self.huella == huella
    }

    /// El **estado** del checkpoint, tal y como va al fichero (JSON
    /// mínimo, escrito a mano como el sidecar de procedencia).
    pub fn a_json(self) -> String {
        format!(
            "{{\n  \"fase\": \"{}\",\n  \"huella\": \"{}\",\n  \"cuantizados\": \"{}\"\n}}\n",
            self.fase.nombre(),
            escapa(&self.huella),
            escapa(&self.cuantizados.join(", "))
        )
    }

    /// El checkpoint que hay en un fichero de estado.
    ///
    /// Un fichero que no se puede leer o que no dice una fase de las seis
    /// **no es un checkpoint**: la tirada empieza de cero (y eso es lo
    /// seguro: reanudar con un estado medio entendido borra o salta cosas
    /// que no toca). La huella es opcional: un checkpoint del formato viejo
    /// se lee con huella vacía, y `es_de` dice que no es de nadie. Los quants
    /// cuantizados también: un checkpoint viejo no dice cuáles, y no se
    /// inventan.
    pub fn de_json(texto: &str) -> Option<Self> {
        let fase = valor(texto, "fase")?;
        let fase = Fase::TODAS.iter().find(|f| f.nombre() == fase)?;
        let cuantizados = valor(texto, "cuantizados")
            .unwrap_or_default()
            .split(',')
            .map(|q| q.trim().to_string())
            .filter(|q| !q.is_empty())
            .collect();
        Some(Checkpoint {
            fase: *fase,
            huella: valor(texto, "huella").unwrap_or_default().to_string(),
            cuantizados,
        })
    }
}

/// El valor de una clave de un JSON plano de un nivel (el checkpoint se
/// escribe y se lee a mano, como el resto de la forja).
fn valor<'a>(texto: &'a str, clave: &str) -> Option<&'a str> {
    let trozos: Vec<&str> = texto.split('"').collect();
    for (i, trozo) in trozos.iter().enumerate() {
        if *trozo == clave {
            let valor = *trozos.get(i + 2)?;
            return Some(valor.trim().trim_matches(|c| c == ',' || c == ' '));
        }
    }
    None
}

/// Escapa lo que rompería un valor JSON (comillas y barras).
fn escapa(texto: &str) -> String {
    texto.replace('\\', "\\\\").replace('"', "\\\"")
}
