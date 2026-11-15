//! La rúbrica de la batería: el índice, parseado y **validado**.
//!
//! `docs/tests/indice.md` declara 16 tests en 6 grupos, el peso de cada test,
//! el recuento y el peso de cada grupo, y el perfil de 12 dimensiones. Un
//! índice que no cuadra **no se interpreta**: se dice la línea y el valor
//! recibido. Aquí no se inventa ningún peso, ningún total ni ninguna
//! dimensión.
//!
//! La forma que se lee es la forma real del índice:
//!
//! - `## Grupo A — Razonamiento lógico-deductivo`
//! - `### A1 · `test-logico-deductivo.md` — Puzle de casas (tipo Einstein)`
//! - `**Peso sugerido:** 5.`
//! - `| A · Lógico-deductivo | 5 | 22 |` y `| **Total** | **16** | **71** |`
//! - `| Dimensión | Tests que la miden |` (dos columnas)

use std::fmt;

use crate::normalizar;

/// Un test de la batería, tal como lo declara el índice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBateria {
    pub id: String,
    pub grupo: char,
    pub fichero: String,
    pub titulo: String,
    pub objetivo: String,
    pub peso: u32,
    /// La línea del índice donde está el encabezado del test.
    pub linea: usize,
}

/// Un grupo de la batería.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Grupo {
    pub letra: char,
    pub nombre: String,
    pub ids: Vec<String>,
    /// El peso total que declara la tabla del índice.
    pub peso_total: u32,
}

/// Una dimensión del perfil de salida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dimension {
    pub nombre: String,
    pub ids: Vec<String>,
}

/// La batería entera, más los avisos que el índice deja.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bateria {
    pub tests: Vec<TestBateria>,
    pub grupos: Vec<Grupo>,
    pub dimensiones: Vec<Dimension>,
    /// Lo que el índice no rompe, pero deja incompleto. Un aviso **no** es
    /// un error: la batería sigue siendo una batería.
    pub avisos: Vec<String>,
}

/// Un índice que no cuadra: la línea y el motivo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorIndice {
    pub linea: usize,
    pub mensaje: String,
}

impl fmt::Display for ErrorIndice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "el índice de la batería no cuadra (línea {}): {}",
            self.linea, self.mensaje
        )
    }
}

impl std::error::Error for ErrorIndice {}

impl Bateria {
    /// Un test por su id.
    pub fn test(&self, id: &str) -> Option<&TestBateria> {
        self.tests.iter().find(|t| t.id == id)
    }

    /// Un grupo por su letra.
    pub fn grupo(&self, letra: char) -> Option<&Grupo> {
        self.grupos.iter().find(|g| g.letra == letra)
    }

    /// Una dimensión por su nombre literal del índice.
    pub fn dimension(&self, nombre: &str) -> Option<&Dimension> {
        self.dimensiones.iter().find(|d| d.nombre == nombre)
    }

    /// Los tests de un grupo, en el orden del índice.
    pub fn tests_del_grupo(&self, letra: char) -> Vec<&TestBateria> {
        self.grupos
            .iter()
            .find(|g| g.letra == letra)
            .map(|g| {
                self.tests
                    .iter()
                    .filter(|t| t.grupo == g.letra)
                    .collect::<Vec<&TestBateria>>()
            })
            .unwrap_or_default()
    }
}

/// Parsea y valida el índice. Lo que no cuadra es `ErrorIndice` con la línea.
pub fn parse_indice(texto: &str) -> Result<Bateria, ErrorIndice> {
    let lineas: Vec<&str> = texto.split('\n').collect();
    let mut tests: Vec<TestBateria> = Vec::new();
    let mut grupos: Vec<Grupo> = Vec::new();
    let mut filas: Vec<(char, usize, u32, usize)> = Vec::new();
    let mut total: Option<(usize, u32, usize)> = None;
    let mut dimensiones: Vec<Dimension> = Vec::new();
    let mut hay_tabla = false;
    let mut hay_perfil = false;
    let mut grupo_actual: Option<char> = None;
    let mut seccion = Seccion::Nada;
    let mut i = 0usize;

    while i < lineas.len() {
        let l = lineas[i].trim();
        if let Some(resto) = l.strip_prefix("## ") {
            let resto = resto.trim();
            if let Some((letra, nombre)) = encabezado_grupo(resto) {
                if let Some(primera) = grupos.iter().find(|g| g.letra == letra) {
                    return Err(ErrorIndice {
                        linea: i + 1,
                        mensaje: format!(
                            "el grupo {letra} aparece dos veces: el índice ya abrió el grupo {letra} con `{}`. \
                             Cada grupo se abre una sola vez",
                            primera.nombre
                        ),
                    });
                }
                grupos.push(Grupo {
                    letra,
                    nombre,
                    ids: Vec::new(),
                    peso_total: 0,
                });
                grupo_actual = Some(letra);
                seccion = Seccion::Grupo;
            } else if resto == "Resumen de pesos por grupo" {
                hay_tabla = true;
                seccion = Seccion::Tabla;
                grupo_actual = None;
            } else if resto == "Perfil de salida recomendado" {
                hay_perfil = true;
                seccion = Seccion::Perfil;
                grupo_actual = None;
            } else {
                seccion = Seccion::Nada;
                grupo_actual = None;
            }
            i += 1;
            continue;
        }
        if seccion == Seccion::Tabla && l.starts_with('|') {
            lee_fila_tabla(l, i + 1, &grupos, &mut filas, &mut total)?;
            i += 1;
            continue;
        }
        if seccion == Seccion::Perfil && l.starts_with('|') {
            lee_fila_perfil(l, i + 1, &tests, &mut dimensiones)?;
            i += 1;
            continue;
        }
        if l.starts_with("### ") {
            let test = lee_test(&lineas, i, grupo_actual, &tests, &grupos)?;
            // El test se apunta en su grupo: el orden de los ids es el
            // orden del índice.
            if let Some(grupo) = grupos.iter_mut().find(|g| g.letra == test.grupo) {
                grupo.ids.push(test.id.clone());
            }
            tests.push(test);
            i = fin_bloque(&lineas, i + 1);
            continue;
        }
        i += 1;
    }

    if tests.is_empty() {
        return Err(ErrorIndice {
            linea: 1,
            mensaje: "el índice no declara ningún test: sin tests no hay batería que puntuar. \
                      Escribe los tests como `### A1 · `test-x.md` — Título` bajo su grupo"
                .to_string(),
        });
    }
    if !hay_tabla {
        return Err(ErrorIndice {
            linea: 1,
            mensaje: "el índice no tiene la sección `Resumen de pesos por grupo`: sin la tabla de \
                      grupos no se puede validar el peso de ningún grupo"
                .to_string(),
        });
    }
    if !hay_perfil {
        return Err(ErrorIndice {
            linea: 1,
            mensaje:
                "el índice no tiene la sección `Perfil de salida recomendado`: sin el perfil no \
                      hay dimensiones que medir"
                    .to_string(),
        });
    }

    // La tabla de grupos contra lo que el índice realmente declara.
    for grupo in &mut grupos {
        let Some(fila) = filas.iter().find(|f| f.0 == grupo.letra) else {
            return Err(ErrorIndice {
                linea: 1,
                mensaje: format!(
                    "la tabla de grupos no menciona al grupo {}: la sección `Resumen de pesos por grupo` \
                     tiene que tener una fila por grupo",
                    grupo.letra
                ),
            });
        };
        let linea = fila.3;
        let peso_sumado: u32 = tests
            .iter()
            .filter(|t| t.grupo == grupo.letra)
            .map(|t| t.peso)
            .sum();
        let tests_sumados = tests.iter().filter(|t| t.grupo == grupo.letra).count();
        if fila.1 != tests_sumados {
            return Err(ErrorIndice {
                linea,
                mensaje: format!(
                    "el grupo {}: la tabla dice {} tests y el índice declara {}",
                    grupo.letra, fila.1, tests_sumados
                ),
            });
        }
        if fila.2 != peso_sumado {
            return Err(ErrorIndice {
                linea,
                mensaje: format!(
                    "el grupo {}: la tabla dice un peso de {} y los pesos de sus tests suman {}",
                    grupo.letra, fila.2, peso_sumado
                ),
            });
        }
        grupo.peso_total = fila.2;
    }
    for (letra, _tests, _peso, linea) in &filas {
        if !grupos.iter().any(|g| g.letra == *letra) {
            return Err(ErrorIndice {
                linea: *linea,
                mensaje: format!(
                    "la tabla de grupos menciona al grupo {letra}, que el índice no abrió con `## Grupo {letra} — nombre`"
                ),
            });
        }
    }
    let Some((tests_tabla, peso_tabla, linea_total)) = total else {
        return Err(ErrorIndice {
            linea: 1,
            mensaje: "la tabla de grupos no tiene la fila `| **Total** | **N** | **P** |`: \
                      sin el total no se puede validar la batería"
                .to_string(),
        });
    };
    let tests_sumados = tests.len();
    let peso_sumado: u32 = tests.iter().map(|t| t.peso).sum();
    if tests_tabla != tests_sumados {
        return Err(ErrorIndice {
            linea: linea_total,
            mensaje: format!(
                "el total de tests: la tabla dice {tests_tabla} y el índice declara {tests_sumados}"
            ),
        });
    }
    if peso_tabla != peso_sumado {
        return Err(ErrorIndice {
            linea: linea_total,
            mensaje: format!(
                "el peso total: la tabla dice {peso_tabla} y los pesos de los tests suman {peso_sumado}"
            ),
        });
    }

    // El perfil: qué tests quedan fuera de las dimensiones.
    let mut avisos: Vec<String> = Vec::new();
    for test in &tests {
        if !dimensiones
            .iter()
            .any(|d| d.ids.iter().any(|id| id == &test.id))
        {
            avisos.push(format!(
                "aviso: el test '{}' no alimenta ninguna dimensión (perfil incompleto)",
                test.id
            ));
        }
    }

    Ok(Bateria {
        tests,
        grupos,
        dimensiones,
        avisos,
    })
}

/// Las secciones del índice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seccion {
    Grupo,
    Tabla,
    Perfil,
    Nada,
}

/// `Grupo A — nombre` → la letra y el nombre.
fn encabezado_grupo(resto: &str) -> Option<(char, String)> {
    let despues = resto.strip_prefix("Grupo ")?;
    let (letra, nombre) = despues.split_once('—')?;
    let letra = letra.trim().chars().next()?;
    if !letra.is_ascii_uppercase() {
        return None;
    }
    Some((letra, nombre.trim().to_string()))
}

/// Una fila de la tabla de grupos: `| A · Corto | 5 | 22 |` o `| **Total** | **16** | **71** |`.
fn lee_fila_tabla(
    l: &str,
    linea: usize,
    grupos: &[Grupo],
    filas: &mut Vec<(char, usize, u32, usize)>,
    total: &mut Option<(usize, u32, usize)>,
) -> Result<(), ErrorIndice> {
    let celdas = celdas(l);
    // La cabecera de la tabla y la línea de guiones no son filas: se
    // saltan sin mirarlas.
    let cabecera = celdas[0].trim();
    if cabecera.starts_with('-') || normalizar(cabecera) == "grupo" {
        return Ok(());
    }
    if celdas.len() != 3 {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "la fila de la tabla de grupos tiene que tener tres celdas (grupo, tests, peso) y la \
                 recibida tiene {}: `{l}`",
                celdas.len()
            ),
        });
    }
    let primera = celdas[0].replace('*', "");
    let primera = primera.trim();
    if normalizar(primera) == "total" {
        let tests = cuenta(
            celdas[1].replace('*', "").trim(),
            linea,
            "el recuento de tests del total",
        )?;
        let peso = entero(celdas[2].replace('*', "").trim(), linea, "el peso total")?;
        *total = Some((tests, peso, linea));
        return Ok(());
    }
    let Some(letra_celda) = primera.split_once('·').map(|(l, _corto)| l.trim()) else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "la primera celda de la fila de grupos tiene la forma `A · Corto` y la recibida es `{}`",
                celdas[0]
            ),
        });
    };
    if letra_celda.len() != 1 {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "la fila de grupos nombra al grupo `{letra_celda}`, y la letra de un grupo es una \
                 sola letra: la fila es `{l}`"
            ),
        });
    }
    let Some(letra) = letra_celda.chars().next() else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("la fila de grupos no dice qué grupo es: `{l}`"),
        });
    };
    if !grupos.iter().any(|g| g.letra == letra) {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "la tabla de grupos menciona al grupo {letra}, que el índice no abrió con `## Grupo {letra} — nombre`"
            ),
        });
    }
    let tests = cuenta(&celdas[1], linea, "el recuento de tests del grupo")?;
    let peso = entero(&celdas[2], linea, "el peso del grupo")?;
    filas.push((letra, tests, peso, linea));
    Ok(())
}

/// Una fila del perfil: `| Deducción formal | A1, A5 |`.
fn lee_fila_perfil(
    l: &str,
    linea: usize,
    tests: &[TestBateria],
    dimensiones: &mut Vec<Dimension>,
) -> Result<(), ErrorIndice> {
    let celdas = celdas(l);
    if celdas.len() == 2 && celdas[0].starts_with('-') {
        return Ok(());
    }
    if celdas.len() != 2 {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "la fila del perfil de dimensiones tiene dos columnas (dimensión, tests) y la recibida \
                 tiene {}: `{l}`",
                celdas.len()
            ),
        });
    }
    let nombre = celdas[0].trim();
    if nombre == "Dimensión" {
        return Ok(());
    }
    let ids: Vec<String> = nombre_ids(&celdas[1]);
    if ids.is_empty() {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "la dimensión `{nombre}` no declara ningún test: la fila `{l}` no alimenta nada del perfil"
            ),
        });
    }
    for id in &ids {
        if !tests.iter().any(|t| t.id == *id) {
            return Err(ErrorIndice {
                linea,
                mensaje: format!(
                    "la dimensión `{nombre}` menciona el test '{id}', que no es un test de la batería"
                ),
            });
        }
    }
    dimensiones.push(Dimension {
        nombre: nombre.to_string(),
        ids,
    });
    Ok(())
}

/// Los ids de tests de una celda: `A1, A5` → ["A1", "A5"].
fn nombre_ids(celda: &str) -> Vec<String> {
    celda
        .split([',', '+', '/'])
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
}

/// Las celdas de una fila de tabla.
fn celdas(l: &str) -> Vec<String> {
    let mut partes: Vec<&str> = l.split('|').collect();
    if partes.first().is_some_and(|p| p.trim().is_empty()) {
        partes.remove(0);
    }
    if partes.last().is_some_and(|p| p.trim().is_empty()) {
        partes.pop();
    }
    partes.iter().map(|p| p.trim().to_string()).collect()
}

/// Un número de la tabla: si no lo es, se dice el valor recibido.
fn entero(valor: &str, linea: usize, que: &str) -> Result<u32, ErrorIndice> {
    match valor.parse::<u32>() {
        Ok(n) => Ok(n),
        Err(_) => Err(ErrorIndice {
            linea,
            mensaje: format!("{que} tiene que ser un número y la tabla dice `{valor}`"),
        }),
    }
}

/// Un **recuento** de la tabla: si no lo es, se dice el valor recibido.
fn cuenta(valor: &str, linea: usize, que: &str) -> Result<usize, ErrorIndice> {
    match valor.parse::<usize>() {
        Ok(n) => Ok(n),
        Err(_) => Err(ErrorIndice {
            linea,
            mensaje: format!("{que} tiene que ser un número y la tabla dice `{valor}`"),
        }),
    }
}

/// El bloque de un test: del encabezado hasta el siguiente encabezado.
fn fin_bloque(lineas: &[&str], desde: usize) -> usize {
    let mut j = desde;
    while j < lineas.len() {
        let t = lineas[j].trim();
        if t.starts_with("## ") || t.starts_with("### ") {
            return j;
        }
        j += 1;
    }
    j
}

/// Un test: su encabezado, su peso y su objetivo.
fn lee_test(
    lineas: &[&str],
    i: usize,
    grupo_actual: Option<char>,
    tests: &[TestBateria],
    grupos: &[Grupo],
) -> Result<TestBateria, ErrorIndice> {
    let linea = i + 1;
    let Some(resto) = lineas[i].trim().strip_prefix("### ") else {
        return Err(ErrorIndice {
            linea,
            mensaje: "el encabezado de un test empieza con `### `".to_string(),
        });
    };
    let (id, fichero, titulo) = encabezado_test(resto, linea)?;
    if let Some(primero) = tests.iter().find(|t| t.id == id) {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "el id '{id}' está repetido: el índice lo declara primero en la línea {}",
                primero.linea
            ),
        });
    }
    let Some(letra) = grupo_actual else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "el test '{id}' está fuera de un grupo: en el índice cada test va bajo su \
                 `## Grupo X — nombre`"
            ),
        });
    };
    if id.chars().next().is_some_and(|c| c != letra) {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "el test '{id}' está bajo el grupo {letra}: el id de un test empieza con la letra \
                 del grupo en el que está"
            ),
        });
    }
    if !grupos.iter().any(|g| g.letra == letra) {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("el grupo {letra} no está abierto en el índice"),
        });
    }
    let fin = fin_bloque(lineas, i + 1);
    let mut peso: Option<&str> = None;
    let mut objetivo: String = String::new();
    let mut j = i + 1;
    while j < fin {
        let t = lineas[j].trim();
        if let Some(resto) = t.strip_prefix("**Peso sugerido:**") {
            peso = Some(resto.trim().trim_end_matches('.').trim());
        }
        if let Some(resto) = t.strip_prefix("**Objetivo:**") {
            let mut partes = vec![resto.trim().to_string()];
            let mut k = j + 1;
            while k < fin && !lineas[k].trim().is_empty() {
                partes.push(lineas[k].trim().to_string());
                k += 1;
            }
            objetivo = partes.join(" ");
        }
        j += 1;
    }
    let Some(valor) = peso else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "el test '{id}' no declara peso: añade `**Peso sugerido:**` con un número de 1 a 5 \
                 bajo el encabezado del test"
            ),
        });
    };
    let peso = match valor.parse::<u32>() {
        Ok(n) => n,
        Err(_) => {
            return Err(ErrorIndice {
                linea,
                mensaje: format!("el test '{id}' declara un peso que no es un número: `{valor}`"),
            });
        }
    };
    if !(1..=5).contains(&peso) {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "el test '{id}' declara un peso de {valor}: el peso de un test es un número de 1 a 5"
            ),
        });
    }
    Ok(TestBateria {
        id,
        grupo: letra,
        fichero,
        titulo,
        objetivo,
        peso,
        linea,
    })
}

/// `A1 · `test-x.md` — Título`: el id, el fichero y el título.
fn encabezado_test(resto: &str, linea: usize) -> Result<(String, String, String), ErrorIndice> {
    let formato =
        "el encabezado de un test tiene la forma `### A1 · `test-x.md` — Título` en el índice";
    let Some(primera) = resto.split_once(" · ") else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("el encabezado `{resto}` no tiene el formato del índice: {formato}"),
        });
    };
    let id = primera.0.trim();
    let Some(con_fichero) = primera.1.split_once('`') else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!(
                "el encabezado `{resto}` no pone el fichero del test entre comillas de \
                             acento grave: {formato}"
            ),
        });
    };
    let Some((fichero, tras_fichero)) = con_fichero.1.split_once('`') else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("el encabezado `{resto}` no cierra el nombre del fichero: {formato}"),
        });
    };
    let Some(titulo) = tras_fichero.trim().strip_prefix('—').map(str::trim) else {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("el encabezado `{resto}` no separa el título con una raya: {formato}"),
        });
    };
    let id = id.to_string();
    if id.len() != 2
        || !id.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        || !id.chars().next().is_some_and(|c| c.is_ascii_uppercase())
    {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("el id `{id}` no tiene la forma del índice (`A1`): {formato}"),
        });
    }
    if !id.chars().nth(1).is_some_and(|c| c.is_ascii_digit()) {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("el id `{id}` no tiene la forma del índice (`A1`): {formato}"),
        });
    }
    if fichero.is_empty() || titulo.is_empty() {
        return Err(ErrorIndice {
            linea,
            mensaje: format!("el encabezado `{resto}` no declara fichero y título: {formato}"),
        });
    }
    Ok((id, fichero.to_string(), titulo.to_string()))
}
