//! El **sistema de formato** de la salida del CLI (doc CLI v2 §2).
//!
//! ## Por qué existe
//!
//! Cada comando pintaba sus líneas como lejana: unos con `{:ancho$}`, otros
//! a ojo, otros con un `format!` suelto. El resultado eran salidas que se
//! veían **cada una a su manera**: dos columnas que no compartían el mismo
//! ancho, un tamaño que empujaba al artefacto, un color que descuadraba la
//! fila. El dueño lo pidió de otra manera: *una sola línea estética*.
//!
//! ## Qué hay aquí
//!
//! | Tipo | Para qué |
//! |---|---|
//! | [`Tabla`] | columnas con alineación, cabecera, regla y notas |
//! | [`Seccion`] | un título y su cuerpo |
//! | [`pares`] | listados de `clave: valor` con las claves alineadas |
//!
//! ## Las reglas (normativas)
//!
//! 1. El **ancho visible** es el que ocupa el texto en la pantalla: los
//!    códigos ANSI no cuentan, los caracteres anchos (CJK, emojis) valen dos
//!    y los acentos combinantes y los modificadores de tono de piel, ninguno.
//! 2. Los datos **numéricos** se alinean a la **derecha**; los de texto, a la
//!    **izquierda**.
//! 3. El texto que no cabe se **recorta** con `…`: nunca se parte en dos
//!    líneas. El título de una columna se recorta como sus datos y un
//!    tabulador o un salto de línea se sustituyen por un espacio.
//! 4. Las líneas **no** llevan espacios al final.
//! 5. El **color** es el de la leyenda de motores (doc CLI v2 §4) y sólo se
//!    usa en el **nombre del motor**. Se cierra con un único reset.
//! 6. La sangría es de **dos espacios** por nivel; un título de sección va en
//!    el margen y su cuerpo, un nivel más dentro.
//!
//! ## Sin E/S
//!
//! Todo aquí es texto: no se lee el disco, no se consulta el entorno. Por eso
//! vive en el core y se puede probar sin máquina.

use crate::EngineColor;
use crate::palette::{CIERRE, Palette};

/// La sangría de un nivel: dos espacios.
const SANGRIA: &str = "  ";

/// El hueco entre dos columnas de una tabla.
const HUECO: &str = "  ";

/// La regla que separa la cabecera de las filas.
const REGLA: &str = "─";

/// Lo que se pone donde el texto se recorta.
const TRUNCADO: &str = "…";

/// Lo que se pone donde el dato **no se sabe**.
///
/// Es el signo de la salida entera (doc CLI v2 §2.1): `—`. Lo que no se
/// sabe no se inventa ni se pregunta con otro signo.
pub const SIN_DATO: &str = "—";

/// El ancho máximo de una columna, en columnas visibles.
///
/// Es el **uno** que decide: una columna sin máximo (`None`) crece hasta lo
/// que ocupe el dato más largo; con máximo, el dato se recorta.
pub const ANCHO_MAXIMO: usize = 36;

/// La alineación de una columna.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineacion {
    /// El dato es texto: se alinea a la izquierda.
    Izquierda,
    /// El dato es un número: se alinea a la derecha.
    Derecha,
}

/// El ancho visible de un texto.
///
/// Los **códigos ANSI no ocupan**: el color no puede desalinear una tabla.
/// Los caracteres **anchos** (CJK, emojis) ocupan dos columnas y los
/// acentos combinantes, ninguna.
pub fn ancho_visible(texto: &str) -> usize {
    let mut ancho = 0usize;
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            salta_ansi(&mut chars);
            continue;
        }
        ancho += ancho_de_caracter(c);
    }
    ancho
}

/// Salta una secuencia ANSI: la que empieza por `ESC [`.
///
/// No devuelve nada: lo que se salta **no ocupa** ancho.
fn salta_ansi(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    if chars.peek() != Some(&'[') {
        return;
    }
    chars.next();
    for c in chars.by_ref() {
        if c.is_ascii_alphabetic() {
            break;
        }
    }
}

/// El ancho de un carácter en la pantalla.
fn ancho_de_caracter(c: char) -> usize {
    let cp = c as u32;
    if cp < 0x20 || cp == 0x7f {
        // Un control no se ve.
        return 0;
    }
    if es_de_ancho_cero(cp) {
        return 0;
    }
    if es_de_ancho_doble(cp) {
        return 2;
    }
    1
}

/// Sustituye los controles que **parten** la celda.
///
/// El tabulador y el salto de línea rompen la rejilla y dividen el dato en
/// dos líneas (doc CLI v2 §2.1: el texto nunca se parte). Se sustituyen
/// por un espacio: la rejilla sigue cuadrando y no se pierde el dato.
///
/// El escape (el de los colores) **no** se toca: no parte nada y es del
/// sistema.
fn sanea(texto: &str) -> String {
    if !texto.contains(['\t', '\n', '\r']) {
        return texto.to_string();
    }
    texto
        .chars()
        .map(|c| match c {
            '\t' | '\n' | '\r' => ' ',
            otro => otro,
        })
        .collect()
}

/// Los rangos que **no** ocupan columna.
///
/// Son los acentos combinantes (los de las tildes y los de las vocales de
/// otros idiomas), los selectores de variación (los de los emojis), los
/// modificadores de tono de piel (los que van pegados a un emoji) y los
/// espacios de ancho cero.
fn es_de_ancho_cero(cp: u32) -> bool {
    matches!(
        cp,
        0x0300..=0x036F // acentos combinantes
            | 0x0483..=0x0489 // marcas cirílicas
            | 0x0591..=0x05BD // marcas hebreas
            | 0x05BF
            | 0x05C1..=0x05C2
            | 0x05C4..=0x05C5
            | 0x05C7
            | 0x0610..=0x061A // marcas árabes
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06DC // marcas coránicas
            | 0x06DF..=0x06E8
            | 0x06EA..=0x06ED
            | 0x0711 // marca siria
            | 0x0730..=0x074A // marcas tora
            | 0x07A6..=0x07B0 // marcas nko
            | 0x0816..=0x082D // marcas de la India
            | 0x0900..=0x0903
            | 0x093A..=0x094F
            | 0x0951..=0x0957
            | 0x0E31
            | 0x0E34..=0x0E3A
            | 0x0E47..=0x0E4E
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x1F3FB..=0x1F3FF // modificadores de tono de piel
            | 0x200B..=0x200F // ancho cero, ZWNJ, ZWJ
            | 0x2060..=0x2064
            | 0x20D0..=0x20F0 // marcas de símbolos
            | 0xFE00..=0xFE0F // selectores de variación
            | 0xFE20..=0xFE2F
            | 0xFEFF // el BOM
            | 0xE0100..=0xE01EF
    )
}

/// Los rangos que ocupan **dos** columnas.
///
/// Son el CJK, el hangul y los emojis: los que el terminal pinta dobles.
fn es_de_ancho_doble(cp: u32) -> bool {
    matches!(
        cp,
        0x1100..=0x115F // jamo hangul
            | 0x231A..=0x231B
            | 0x2329..=0x232A
            | 0x23E9..=0x23EC
            | 0x23F0
            | 0x23F3
            | 0x25FD..=0x25FE
            | 0x2614..=0x2615
            | 0x2648..=0x2653
            | 0x267F
            | 0x2693
            | 0x26A0..=0x26A1
            | 0x26AA..=0x26AB
            | 0x26BD..=0x26BE
            | 0x26C4..=0x26C5
            | 0x26CE
            | 0x26D4
            | 0x26EA
            | 0x26F2..=0x26F3
            | 0x26F5
            | 0x26FA
            | 0x26FD
            | 0x2705
            | 0x270A..=0x270B
            | 0x2728
            | 0x274C
            | 0x274E
            | 0x2753..=0x2755
            | 0x2757
            | 0x2795..=0x2797
            | 0x27B0
            | 0x27BF
            | 0x2B1B..=0x2B1C
            | 0x2B50
            | 0x2B55
            | 0x2E80..=0x303E // radicales CJK y símbolos
            | 0x3041..=0x33FF // kana, hangul y CJK
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF // yi
            | 0xA960..=0xA97F
            | 0xAC00..=0xD7A3 // sílabas hangul
            | 0xF900..=0xFAFF
            | 0xFE10..=0xFE19
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60 // formas de ancho completo
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F // emojis
            | 0x1F680..=0x1F6FF
            | 0x1F900..=0x1F9FF
            | 0x1FA70..=0x1FAFF
            | 0x20000..=0x2FFFD
            | 0x30000..=0x3FFFD
    )
}

/// Recorta un texto al ancho visible dado.
///
/// Lo que no cabe se corta por el final y se marca con `…`: una celda
/// recortada se **ve** recortada, no se parte en dos líneas.
pub fn truncar(texto: &str, ancho: usize) -> String {
    if ancho_visible(texto) <= ancho {
        return texto.to_string();
    }
    if ancho == 0 {
        return String::new();
    }
    let limite = ancho - 1;
    let mut out = String::new();
    let mut usado = 0usize;
    let mut con_color = false;
    let mut chars = texto.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            con_color = true;
            out.push(c);
            if chars.peek() == Some(&'[') {
                out.push(chars.next().unwrap_or('['));
                for c in chars.by_ref() {
                    out.push(c);
                    if c.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        let w = ancho_de_caracter(c);
        if usado + w > limite {
            break;
        }
        out.push(c);
        usado += w;
    }
    out.push_str(TRUNCADO);
    if con_color {
        // El color se cierra con el **único** reset del sistema (el de la
        // paleta): si no, se derrama sobre lo que venga después.
        out.push_str(CIERRE);
    }
    out
}

/// Rellena un texto hasta el ancho visible dado, según la alineación.
///
/// El relleno va **fuera** del color: los espacios de relleno no se pintan.
fn rellena(texto: &str, ancho: usize, alineacion: Alineacion) -> String {
    let visible = ancho_visible(texto);
    if visible >= ancho {
        return texto.to_string();
    }
    let relleno = " ".repeat(ancho - visible);
    match alineacion {
        Alineacion::Izquierda => format!("{texto}{relleno}"),
        Alineacion::Derecha => format!("{relleno}{texto}"),
    }
}

/// Quita los espacios del final de una línea.
///
/// Ninguna línea del CLI lleva espacios al final: rompen el diff y
/// descolocan al copiar.
fn sin_espacios_finales(linea: &str) -> String {
    linea.trim_end().to_string()
}

/// Una celda: su texto y, si la leyenda lo dice, su color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Celda {
    /// El texto de la celda.
    texto: String,
    /// El color de la leyenda, si lo tiene.
    color: Option<EngineColor>,
    /// El sufijo de la celda: texto **fuera** del color.
    ///
    /// Sólo lo usan las filas de grupo: el nombre del motor va en su color
    /// y lo que va detrás ("instalado") va en el color de siempre.
    sufijo: String,
}

impl Celda {
    /// Una celda sin color.
    ///
    /// El texto se **sanea**: un tabulador o un salto de línea partirían la
    /// celda en dos líneas (doc CLI v2 §2.1).
    pub fn plana(texto: impl Into<String>) -> Self {
        let texto = texto.into();
        Celda {
            texto: sanea(&texto),
            color: None,
            sufijo: String::new(),
        }
    }

    /// Una celda con el color de un motor (doc CLI v2 §4).
    ///
    /// Es el **único** sitio donde se pinta: el color es de la leyenda de
    /// motores y no se usa para nada más.
    pub fn motor(texto: impl Into<String>, color: EngineColor) -> Self {
        let texto = texto.into();
        Celda {
            texto: sanea(&texto),
            color: Some(color),
            sufijo: String::new(),
        }
    }

    /// Una celda de motor con un **sufijo** fuera del color.
    ///
    /// Es la fila de grupo del doctor: el nombre del motor va en el color de
    /// la leyenda y su estado ("instalado"), detrás, en el color de siempre.
    pub fn motor_con_sufijo(
        nombre: impl Into<String>,
        sufijo: impl Into<String>,
        color: EngineColor,
    ) -> Self {
        let nombre = nombre.into();
        let sufijo = sufijo.into();
        Celda {
            texto: sanea(&nombre),
            color: Some(color),
            sufijo: sanea(&sufijo),
        }
    }

    /// El texto de la celda.
    pub fn texto(&self) -> &str {
        &self.texto
    }

    /// El color de la celda, si lo tiene.
    pub fn color(&self) -> Option<EngineColor> {
        self.color
    }

    /// El sufijo de la celda: el texto que va **fuera** del color.
    pub fn sufijo(&self) -> &str {
        &self.sufijo
    }
}

/// Una columna de una tabla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Columna {
    /// El título de la columna.
    titulo: String,
    /// Cómo se alinean sus datos.
    alineacion: Alineacion,
    /// El ancho máximo, si lo tiene.
    ancho_maximo: Option<usize>,
}

impl Columna {
    /// Una columna de **texto**: se alinea a la izquierda.
    pub fn texto(titulo: impl Into<String>) -> Self {
        Columna {
            titulo: titulo.into(),
            alineacion: Alineacion::Izquierda,
            ancho_maximo: None,
        }
    }

    /// Una columna de **números**: se alinea a la derecha.
    pub fn numero(titulo: impl Into<String>) -> Self {
        Columna {
            titulo: titulo.into(),
            alineacion: Alineacion::Derecha,
            ancho_maximo: None,
        }
    }

    /// El ancho máximo de la columna: lo que no cabe se recorta.
    pub fn maximo(mut self, ancho: usize) -> Self {
        self.ancho_maximo = Some(ancho);
        self
    }

    /// El título de la columna.
    pub fn titulo(&self) -> &str {
        &self.titulo
    }

    /// Cómo se alinean los datos de la columna.
    pub fn alineacion(&self) -> Alineacion {
        self.alineacion
    }

    /// El ancho máximo de la columna, si lo tiene.
    pub fn ancho_maximo(&self) -> Option<usize> {
        self.ancho_maximo
    }
}

/// Una fila de una tabla.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fila {
    /// Una celda por columna.
    Datos(Vec<Celda>),
    /// Una fila de **grupo**: una celda que ocupa toda la tabla.
    ///
    /// Es la fila del motor en un listado: su nombre va en el color de la
    /// leyenda (§4) y las filas de sus modelos van debajo. La precede una
    /// regla (salvo si es la primera fila): la regla separa un grupo del
    /// siguiente, y así el listado **no** repite la cabecera por motor.
    Grupo(Celda),
    /// Una nota: una celda que ocupa **toda** la tabla.
    ///
    /// Es lo que permite decir por qué una fila no vale (sin romper la
    /// rejilla) sin inventar una columna que sólo usa una fila.
    Nota(Celda),
}

/// Los anchos de las columnas de una tabla.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Anchos {
    /// El ancho de cada columna, en columnas visibles.
    columnas: Vec<usize>,
}

impl Anchos {
    /// Los anchos de varias tablas: **el mayor** de cada columna.
    ///
    /// Así las tablas de una misma salida (los grupos de un listado) salen
    /// con las **mismas** columnas y el listado se lee como una sola tabla.
    pub fn combinado(tablas: &[&Tabla]) -> Self {
        let mut columnas: Vec<usize> = Vec::new();
        for tabla in tablas {
            for (i, ancho) in tabla.anchos().columnas.iter().enumerate() {
                if columnas.len() <= i {
                    columnas.push(*ancho);
                } else if *ancho > columnas[i] {
                    columnas[i] = *ancho;
                }
            }
        }
        Anchos { columnas }
    }

    /// El ancho de una columna.
    pub fn columna(&self, indice: usize) -> usize {
        self.columnas.get(indice).copied().unwrap_or(0)
    }
}

/// Una tabla: columnas con alineación, cabecera, regla y filas.
///
/// Las filas pueden ser de datos o **notas**: una nota ocupa toda la tabla
/// y sirve para lo que va debajo de una fila sin descuadrar la rejilla.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tabla {
    /// El nivel de sangría de las líneas de la tabla.
    nivel: usize,
    /// Las columnas, en su orden.
    columnas: Vec<Columna>,
    /// Las filas, en su orden.
    filas: Vec<Fila>,
}

impl Tabla {
    /// Una tabla vacía, con sus líneas indentadas `nivel` niveles.
    pub fn nivel(nivel: usize) -> Self {
        Tabla {
            nivel,
            columnas: Vec::new(),
            filas: Vec::new(),
        }
    }

    /// Añade una columna (y devuelve la tabla, para encadenar).
    pub fn columna(mut self, columna: Columna) -> Self {
        self.columnas.push(columna);
        self
    }

    /// Añade una fila de datos: una celda por columna.
    pub fn fila(&mut self, fila: Vec<Celda>) {
        self.filas.push(Fila::Datos(fila));
    }

    /// Añade una fila de **grupo**: el nombre del motor, en su color.
    ///
    /// La fila ocupa toda la tabla y la precede una regla (salvo si es la
    /// primera): los grupos se separan sin repetir la cabecera.
    pub fn grupo(&mut self, celda: Celda) {
        self.filas.push(Fila::Grupo(celda));
    }

    /// Añade una nota: una línea que ocupa toda la tabla.
    pub fn nota(&mut self, celda: Celda) {
        self.filas.push(Fila::Nota(celda));
    }

    /// Si la tabla no tiene filas.
    pub fn esta_vacia(&self) -> bool {
        self.filas.is_empty()
    }

    /// Los anchos de las columnas: el mayor entre el título y las celdas.
    ///
    /// Una columna con **máximo** no crece de ese ancho: lo que no cabe se
    /// recorta al pintar.
    pub fn anchos(&self) -> Anchos {
        let mut columnas = Vec::with_capacity(self.columnas.len());
        for (i, columna) in self.columnas.iter().enumerate() {
            let mut ancho = ancho_visible(columna.titulo());
            for fila in &self.filas {
                let Fila::Datos(celdas) = fila else {
                    continue;
                };
                if let Some(celda) = celdas.get(i) {
                    ancho = ancho.max(ancho_visible(celda.texto()));
                }
            }
            if let Some(maximo) = columna.ancho_maximo() {
                ancho = ancho.min(maximo);
            }
            columnas.push(ancho);
        }
        Anchos { columnas }
    }

    /// Las líneas de la tabla, con sus propios anchos.
    pub fn lineas(&self, paleta: &Palette) -> Vec<String> {
        self.lineas_con(&self.anchos(), paleta)
    }

    /// Las líneas de la tabla, con los anchos dados.
    ///
    /// Una tabla sin filas no imprime nada: ni cabecera ni regla.
    ///
    /// Una tabla **sin columnas** tampoco lleva cabecera: no hay nada que
    /// etiquetar. Es lo que necesitan las tablas de sólo grupos (los
    /// motores plegados del listado).
    pub fn lineas_con(&self, anchos: &Anchos, paleta: &Palette) -> Vec<String> {
        if self.filas.is_empty() {
            return Vec::new();
        }
        let sangria = SANGRIA.repeat(self.nivel);
        let ancho_total = self.ancho_total(anchos);
        let mut out: Vec<String> = Vec::with_capacity(self.filas.len() + 2);
        // La cabecera y su regla.
        if !self.columnas.is_empty() {
            let cabecera: Vec<String> = self
                .columnas
                .iter()
                .enumerate()
                .map(|(i, columna)| {
                    let ancho = anchos.columna(i);
                    // El título se recorta como los datos: si no, desborda
                    // la regla de su columna.
                    let titulo = truncar(columna.titulo(), ancho);
                    rellena(&titulo, ancho, columna.alineacion())
                })
                .collect();
            out.push(sin_espacios_finales(&format!(
                "{sangria}{}",
                cabecera.join(HUECO)
            )));
            // La regla.
            out.push(format!("{sangria}{}", REGLA.repeat(ancho_total)));
        }
        // Las filas.
        for (indice, fila) in self.filas.iter().enumerate() {
            match fila {
                Fila::Datos(celdas) => {
                    let celdas: Vec<String> = self
                        .columnas
                        .iter()
                        .enumerate()
                        .map(|(i, columna)| {
                            let ancho = anchos.columna(i);
                            match celdas.get(i) {
                                Some(celda) => {
                                    let texto = truncar(celda.texto(), ancho);
                                    let pintado = match celda.color() {
                                        Some(color) => paleta.paint(color, &texto),
                                        None => texto,
                                    };
                                    rellena(&pintado, ancho, columna.alineacion())
                                }
                                None => String::new(),
                            }
                        })
                        .collect();
                    out.push(sin_espacios_finales(&format!(
                        "{sangria}{}",
                        celdas.join(HUECO)
                    )));
                }
                Fila::Grupo(celda) => {
                    // La regla separa un grupo del siguiente: el primer
                    // grupo ya lo separa la regla de la cabecera.
                    if indice > 0 && ancho_total > 0 {
                        out.push(format!("{sangria}{}", REGLA.repeat(ancho_total)));
                    }
                    let pintado = match celda.color() {
                        Some(color) => paleta.paint(color, celda.texto()),
                        None => celda.texto().to_string(),
                    };
                    // El sufijo va **fuera** del color: el color es del
                    // nombre del motor, no de lo que va detrás.
                    out.push(format!("{sangria}{pintado}{}", celda.sufijo()));
                }
                Fila::Nota(celda) => {
                    // Una nota **no** se recorta: es texto libre, no una
                    // celda, y su ancho no tiene que encajar en la tabla.
                    let pintado = match celda.color() {
                        Some(color) => paleta.paint(color, celda.texto()),
                        None => celda.texto().to_string(),
                    };
                    out.push(format!("{sangria}{SANGRIA}{pintado}"));
                }
            }
        }
        out
    }

    /// El ancho total de la tabla: las columnas y sus huecos.
    fn ancho_total(&self, anchos: &Anchos) -> usize {
        let suma: usize = (0..self.columnas.len()).map(|i| anchos.columna(i)).sum();
        suma + HUECO.len() * self.columnas.len().saturating_sub(1)
    }
}

/// Una sección: un título y su cuerpo.
///
/// El título va en el margen (o indentado, si la sección está anidada) y el
/// cuerpo lo compone quien llama: las tablas ya saben su nivel de sangría.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Seccion {
    /// El título de la sección.
    titulo: String,
    /// El nivel de sangría del título.
    nivel: usize,
    /// Las líneas del cuerpo.
    cuerpo: Vec<String>,
}

impl Seccion {
    /// Una sección de primer nivel: el título va en el margen.
    pub fn nueva(titulo: impl Into<String>) -> Self {
        Seccion {
            titulo: titulo.into(),
            nivel: 0,
            cuerpo: Vec::new(),
        }
    }

    /// Añade una línea al cuerpo.
    pub fn linea(&mut self, linea: impl Into<String>) {
        self.cuerpo.push(linea.into());
    }

    /// Añade un cuerpo entero (las líneas de una tabla, por ejemplo).
    pub fn cuerpo(&mut self, lineas: Vec<String>) {
        self.cuerpo.extend(lineas);
    }

    /// Las líneas de la sección: el título y su cuerpo.
    pub fn lineas(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.cuerpo.len() + 1);
        out.push(format!("{}{}", SANGRIA.repeat(self.nivel), self.titulo));
        out.extend(self.cuerpo.iter().cloned());
        out
    }
}

/// Las líneas de un listado de pares `clave: valor`.
///
/// Las claves se alinean: el valor de cada par empieza **siempre** en la
/// misma columna.
pub fn pares(nivel: usize, pares: &[(&str, &str)]) -> Vec<String> {
    let ancho = pares
        .iter()
        .map(|(clave, _)| ancho_visible(clave))
        .max()
        .unwrap_or(0);
    let sangria = SANGRIA.repeat(nivel);
    pares
        .iter()
        .map(|(clave, valor)| {
            let etiqueta = format!("{clave}:");
            let relleno = " ".repeat((ancho + 1).saturating_sub(ancho_visible(&etiqueta)));
            format!("{sangria}{etiqueta}{relleno}{HUECO}{valor}")
        })
        .collect()
}
