//! La selección de la batería: por tipo, por grupo, uno solo, o todos.
//!
//! El doc CLI v2 §13 dice que el comando ofrece lanzar **por tipo, por
//! grupo, uno solo, o todos**. El índice tiene dos taxonomías: los **grupos**
//! (A–F) y las **dimensiones** (las doce del perfil de salida). El índice no
//! declara ningún «tipo», así que `--tipo` es el **alias** de `--dimension`:
//! es la misma taxonomía con otro nombre, y por eso no pueden ir los dos a
//! la vez.
//!
//! ## Lo que no existe se rechaza con el valor recibido
//!
//! Ninguna selección se interpreta: un grupo que no existe, un test que no
//! existe o una dimensión que no existe son **errores de uso** (código 2) que
//! nombran lo que se recibió y dicen qué hay. El valor se conserva tal cual
//! llega desde la línea de órdenes hasta que se resuelve, para que el error
//! pueda nombrarlo.

use std::fmt;

use crate::indice::Bateria;
use crate::normalizar;

/// Un error de uso: lo que se pidió no existe, o no se puede pedir a la vez.
///
/// Es un error de **uso** (código de salida 2), no de ejecución: el
/// programa está bien, la pregunta no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorUso {
    /// El mensaje: dice qué se recibió y qué hacer.
    pub mensaje: String,
}

impl ErrorUso {
    /// Un error de uso con su mensaje.
    fn nuevo(mensaje: impl Into<String>) -> Self {
        Self {
            mensaje: mensaje.into(),
        }
    }
}

impl fmt::Display for ErrorUso {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.mensaje)
    }
}

impl std::error::Error for ErrorUso {}

/// Lo que pidió el usuario en la línea de órdenes.
///
/// Los cuatro valores vienen tal cual: la comprobación de qué existe se
/// hace al resolver, contra la batería, y no antes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Opciones {
    /// `--grupo`: la letra del grupo (A–F).
    pub grupo: Option<String>,

    /// `--test`: el id (`F2`) o el fichero (`test-calibracion.md`).
    pub test: Option<String>,

    /// `--dimension`: el nombre de la dimensión del índice.
    pub dimension: Option<String>,

    /// `--tipo`: el alias de `--dimension`.
    pub tipo: Option<String>,

    /// `--todos`: toda la batería.
    pub todos: bool,
}

/// Qué tests se lanzan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seleccion {
    /// Un grupo por su letra.
    Grupo(char),

    /// Un valor de grupo que **no** es una letra: se rechaza al resolver,
    /// diciendo qué se recibió.
    GrupoIlegible(String),

    /// Un test por su id o por su fichero.
    Test(String),

    /// Una dimensión por su nombre (o por su prefijo único).
    Dimension(String),

    /// Todos los tests, en el orden del índice.
    Todos,
}

/// Traduce las opciones en una selección.
///
/// Lo que se puede pedir a la vez es **una** cosa: grupo, test, dimensión o
/// todos. Dos a la vez es un error de uso que nombra las dos.
pub fn seleccion_de(opciones: Opciones) -> Result<Seleccion, ErrorUso> {
    let Opciones {
        grupo,
        test,
        dimension,
        tipo,
        todos,
    } = opciones;

    // `--dimension` y `--tipo` son lo mismo: no pueden ir juntos.
    if let (Some(dimension), Some(tipo)) = (&dimension, &tipo) {
        return Err(ErrorUso::nuevo(format!(
            "'--dimension' y '--tipo' son la misma cosa (el índice no declara \
             'tipos', sino dimensiones): usa sólo uno de los dos \
             (recibidos: '{dimension}' y '{tipo}')"
        )));
    }
    let dimension = dimension.or(tipo);

    // Los valores vacíos son un error de uso: no se puede pedir «el grupo ».
    for (flag, valor) in [
        ("--grupo", &grupo),
        ("--test", &test),
        ("--dimension", &dimension),
    ] {
        if let Some(valor) = valor
            && valor.trim().is_empty()
        {
            return Err(ErrorUso::nuevo(format!(
                "el valor de '{flag}' está vacío: di qué quieres (por ejemplo \
                 '{flag} F' o 'ropero test --help')"
            )));
        }
    }

    // Cuántas cosas se piden: sólo se puede pedir una.
    let mut pedidas: Vec<String> = Vec::new();
    if todos {
        pedidas.push("todos".to_string());
    }
    if let Some(grupo) = &grupo {
        pedidas.push(format!("grupo '{grupo}'"));
    }
    if let Some(test) = &test {
        pedidas.push(format!("test '{test}'"));
    }
    if let Some(dimension) = &dimension {
        pedidas.push(format!("dimensión '{dimension}'"));
    }
    if pedidas.len() > 1 {
        return Err(ErrorUso::nuevo(format!(
            "no se pueden pedir dos cosas a la vez: {} (elige una: --grupo, \
             --test, --dimension/--tipo o --todos)",
            pedidas.join(" y ")
        )));
    }

    if let Some(grupo) = grupo {
        let letra = grupo.trim().chars().next();
        let es_letra =
            grupo.trim().chars().count() == 1 && letra.is_some_and(|c| c.is_ascii_alphabetic());
        return Ok(match (es_letra, letra) {
            (true, Some(c)) => Seleccion::Grupo(c.to_ascii_uppercase()),
            _ => Seleccion::GrupoIlegible(grupo.trim().to_string()),
        });
    }
    if let Some(test) = test {
        return Ok(Seleccion::Test(test.trim().to_string()));
    }
    if let Some(dimension) = dimension {
        return Ok(Seleccion::Dimension(dimension.trim().to_string()));
    }
    Ok(Seleccion::Todos)
}

/// Los ids de los tests de la selección, **en el orden del índice.
///
/// Lo que no existe se rechaza con el valor recibido: el mensaje dice qué
/// se pidió y qué hay.
pub fn selecciona(bateria: &Bateria, seleccion: &Seleccion) -> Result<Vec<String>, ErrorUso> {
    match seleccion {
        Seleccion::Todos => Ok(bateria.tests.iter().map(|t| t.id.clone()).collect()),
        Seleccion::Grupo(letra) => match bateria.grupo(*letra) {
            Some(grupo) => Ok(grupo.ids.clone()),
            None => Err(ErrorUso::nuevo(format!(
                "el grupo '{letra}' no existe: los grupos de la batería son {}",
                letras(bateria).join(", ")
            ))),
        },
        Seleccion::GrupoIlegible(valor) => Err(ErrorUso::nuevo(format!(
            "'{valor}' no es un grupo: un grupo es una sola letra ({})",
            letras(bateria).join(", ")
        ))),
        Seleccion::Test(valor) => match test_de(bateria, valor) {
            Some(id) => Ok(vec![id]),
            None => Err(ErrorUso::nuevo(format!(
                "el test '{valor}' no existe en la batería: los ids van de A1 a \
                 F2 (ropero test --help)"
            ))),
        },
        Seleccion::Dimension(nombre) => match dimension_de(bateria, nombre) {
            Ok(Some(dimension)) => Ok(dimension.ids.clone()),
            Ok(None) => Err(ErrorUso::nuevo(format!(
                "la dimensión '{nombre}' no existe en el índice: las dimensiones \
                 de la batería son {}",
                bateria
                    .dimensiones
                    .iter()
                    .map(|d| d.nombre.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            ))),
            Err(ambiguas) => Err(ErrorUso::nuevo(format!(
                "'{nombre}' no dice qué dimensión: es el prefijo de {}",
                ambiguas.join(", ")
            ))),
        },
    }
}

/// Las letras de los grupos de la batería, en orden.
fn letras(bateria: &Bateria) -> Vec<String> {
    bateria.grupos.iter().map(|g| g.letra.to_string()).collect()
}

/// El id del test que se pidió: por su id o por su fichero.
fn test_de(bateria: &Bateria, valor: &str) -> Option<String> {
    bateria
        .tests
        .iter()
        .find(|t| t.id.eq_ignore_ascii_case(valor) || t.fichero.eq_ignore_ascii_case(valor))
        .map(|t| t.id.clone())
}

/// La dimensión que se pidió: por su nombre literal o por su prefijo único.
///
/// `Ok(None)` es que no hay ninguna; `Err` es que el prefijo dice más de una
/// (y el error las nombra).
fn dimension_de<'a>(
    bateria: &'a Bateria,
    nombre: &str,
) -> Result<Option<&'a crate::indice::Dimension>, Vec<String>> {
    let pedido = normalizar(nombre);
    if let Some(dimension) = bateria
        .dimensiones
        .iter()
        .find(|d| normalizar(&d.nombre) == pedido)
    {
        return Ok(Some(dimension));
    }
    let candidatas: Vec<&crate::indice::Dimension> = bateria
        .dimensiones
        .iter()
        .filter(|d| normalizar(&d.nombre).starts_with(&pedido))
        .collect();
    match candidatas.len() {
        0 => Ok(None),
        1 => Ok(Some(candidatas[0])),
        _ => Err(candidatas
            .iter()
            .map(|d| d.nombre.clone())
            .collect::<Vec<String>>()),
    }
}
