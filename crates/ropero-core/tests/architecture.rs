//! Test de arquitectura (T-02).
//!
//! Comprueba tres cosas del contrato estructural del workspace:
//!
//! 1. **`ropero-core` es dominio puro**: no lee disco, no consulta el
//!    entorno, no lanza procesos y no habla con la red (doc B §3.2.3).
//! 2. **El grafo de dependencias del workspace no tiene ciclos** y
//!    `ropero-core` no depende de ningún crate del workspace.
//! 3. **`ropero-cli` mide menos de 1200 líneas** (R-ARQ-1): es un
//!    dispatcher, no un sitio donde viva la lógica. El umbral subió de 400
//!    a 800 en el [ADR-005](../../docs/adr/ADR-005-umbral-de-lineas-del-cli.md),
//!    a 1100 en el
//!    [ADR-006](../../docs/adr/ADR-006-umbral-de-lineas-del-cli-tras-install.md)
//!    y a 1200 en el
//!    [ADR-007](../../docs/adr/ADR-007-umbral-de-lineas-del-cli-tras-gguffer.md).
//!
//! ## Cómo se detecta la E/S de verdad
//!
//! Buscar la cadena literal `"std::fs"` no basta: una fuga puede venir en
//! forma de `fs::read_to_string`, `Path::is_dir()`, `std::env::var`,
//! `env::current_dir()` o `std::process::Command`, sin escribir nunca
//! `std::fs`. Por eso el examen quita primero los comentarios (los
//! comentarios **hablan** de E/S: describen la que no hay) y luego busca
//! los **patrones de la E/S real**, no una cadena.
//!
//! Cada test puede fallar: reintroducir una fuga (o un ciclo, o una línea de
//! más) pone el test en rojo.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// La raíz del workspace: `<repo>/`.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

fn collect_rs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.extend(collect_rs(&path));
            } else if path.extension().map(|ext| ext == "rs").unwrap_or(false) {
                out.push(path);
            }
        }
    }
    out
}

/// El código de un fichero Rust, **sin los comentarios**.
///
/// Los comentarios de la v1 describen la E/S que no hay ("no lee disco",
/// "std::env::var"): si no se quitaran, un fichero perfectamente puro
/// parecería una fuga. Se quitan los de línea (`//`) y los de bloque
/// (`/* … */`).
fn code_without_comments(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_block = false;
    let mut in_string = false;
    let mut in_char = false;
    while let Some(c) = chars.next() {
        if in_block {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_block = false;
            }
            continue;
        }
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
            if c == '"' {
                in_string = false;
            }
            continue;
        }
        if in_char {
            out.push(c);
            if c == '\\' {
                if let Some(next) = chars.next() {
                    out.push(next);
                }
                continue;
            }
            if c == '\'' {
                in_char = false;
            }
            continue;
        }
        match c {
            '/' if chars.peek() == Some(&'/') => {
                // Comentario de línea: hasta el fin de la línea.
                for next in chars.by_ref() {
                    if next == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                in_block = true;
            }
            '"' => {
                in_string = true;
                out.push(c);
            }
            '\'' => {
                in_char = true;
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Las rutas prohibidas en `ropero-core`: la E/S y la red.
///
/// Cada entrada es un **patrón** de código, no una cadena literal: así se
/// detecta `fs::read_to_string` tanto si se escribe `std::fs::read_to_string`
/// como si se importa `use std::fs;` y se llama `fs::read_to_string`.
const IO_PATTERNS: &[(&str, &str)] = &[
    ("std::fs", "la E/S de disco"),
    ("fs::", "la E/S de disco"),
    ("std::env", "la consulta del entorno"),
    ("env::", "la consulta del entorno"),
    ("std::process", "el lanzamiento de procesos"),
    ("process::", "el lanzamiento de procesos"),
    ("Command::", "el lanzamiento de procesos"),
    ("std::net", "la E/S de red"),
    ("tokio", "el runtime asíncrono"),
    ("reqwest", "el cliente HTTP"),
    ("ureq", "el cliente HTTP"),
    ("hyper::", "el cliente HTTP"),
];

/// Los métodos de `Path`/`PathBuf` que **tocan el disco**.
///
/// Éstos son los que se escapaban: `is_dir()` no escribe `std::fs` en
/// ninguna parte y, sin embargo, pregunta al disco.
///
/// Se buscan como **llamada** (`.nombre(`), no como palabra suelta: así
/// `canonicalize_engine` (una función propia que no toca el disco) no se
/// confunde con `Path::canonicalize` (que sí lo hace).
const DISK_METHODS: &[&str] = &[
    "is_dir",
    "is_file",
    "is_symlink",
    "read_dir",
    "read_to_string",
    "read_to_end",
    "write(",
    "create_dir",
    "remove_file",
    "remove_dir",
    "metadata",
    "symlink_metadata",
    "canonicalize",
    "exists(",
    "current_dir",
    "current_exe",
    "temp_dir",
];

/// Los métodos de `Path`/`PathBuf` que **NO** tocan el disco.
///
/// `file_name` y `extension` son operaciones sobre la ruta, no sobre lo que
/// hay en disco: se pueden usar sin E/S (y el core los usa). La lista está
/// aquí para que la exclusión sea **explícita** y no un olvido.
const PATH_METHODS_SIN_ES: &[&str] = &["file_name", "extension"];

/// `ropero-core` no hace E/S, ni consulta el entorno, ni lanza procesos.
///
/// El examen es sobre el **código**, no sobre el texto: los comentarios
/// hablan de la E/S que no hay.
#[test]
fn ropero_core_has_no_forbidden_io_or_network() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = collect_rs(&src);
    files.sort();
    assert!(!files.is_empty(), "no se encontró el código del core");
    for file in &files {
        let content = fs::read_to_string(file).expect("read source file");
        let code = code_without_comments(&content);
        for (pattern, what) in IO_PATTERNS {
            assert!(
                !code.contains(pattern),
                "{}: `{pattern}` ({what}) no puede aparecer en ropero-core: \
                 el dominio es puro (doc B §3.2.3)",
                file.display()
            );
        }
        for method in DISK_METHODS {
            if PATH_METHODS_SIN_ES.contains(method) {
                continue;
            }
            // Se busca la **llamada**: `.is_dir(`, `.read_to_string(`.
            // Así `canonicalize_engine` (función propia) no se confunde con
            // `Path::canonicalize` (que sí toca el disco).
            let call = format!(".{method}(");
            assert!(
                !code.contains(&call),
                "{}: `{call}` toca el disco: ropero-core no lee disco \
                 (doc B §3.2.3). Lo que haga falta entra por parámetro.",
                file.display()
            );
        }
        // `HOME` y el directorio de trabajo también son del entorno.
        for token in ["\"HOME\"", "current_dir()"] {
            assert!(
                !code.contains(token),
                "{}: `{token}` lee el entorno o el disco: en ropero-core \
                 eso llega por parámetro (doc B §3.2.3).",
                file.display()
            );
        }
    }
}

/// `ropero-core` no depende de ningún crate del workspace.
///
/// El core es la base: si dependiera de otro crate del workspace, el grafo
/// tendría un ciclo o el dominio dejaría de estar abajo del todo.
#[test]
fn ropero_core_does_not_depend_on_workspace_crates() {
    let cargo_toml = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let content = fs::read_to_string(&cargo_toml).expect("read Cargo.toml");
    let forbidden = [
        "ropero-runtime",
        "ropero-download",
        "ropero-forge",
        "ropero-server",
        "ropero-cli",
        "tokio",
        "reqwest",
    ];
    for token in forbidden {
        assert!(
            !content.contains(token),
            "ropero-core Cargo.toml must not mention `{token}`"
        );
    }
}

/// `ropero-cli` mide menos de 3400 líneas.
///
/// Es un dispatcher: la lógica vive en `ropero-core` (doc B §3.3) y la E/S
/// en `ropero-runtime`.
///
/// El umbral subió de 400 a 800 en el ADR-005, de 800 a 1100 en el ADR-006,
/// a 1200 en el ADR-007, a 1600 y 1700 en el ADR-009 (configuración
/// externa/bootstrap; `ropero <modelo> remove`). El comando `connect`
/// añade adaptadores de CLI por agente. La ampliación de `models add` añade
/// el flujo de entrada, asistente y validación visible al usuario; la lógica
/// de persistencia sigue separada de los adaptadores de configuración. Se
/// reserva una ampliación acotada para los conectores manuales documentados.
/// El listado filtrado por engine, los argumentos comunes y el método `tool`
/// añaden adaptadores de CLI; el límite se amplía a 3400 para esas rutas.
#[test]
fn ropero_cli_under_3400_lines() {
    let cli_src = repo_root().join("crates").join("ropero-cli").join("src");
    let mut lines = 0usize;
    for file in collect_rs(&cli_src) {
        let content = fs::read_to_string(&file).expect("read cli source");
        lines += content.lines().count();
    }
    assert!(
        lines < 3400,
        "ropero-cli has {lines} lines, expected fewer than 3400"
    );
}

/// El grafo de dependencias internas del workspace no tiene ciclos.
///
/// El grafo se deriva de los `Cargo.toml` **reales**: de cada sección
/// `[dependencies]` (y `[dev-dependencies]`, `[build-dependencies]`) sale el
/// nombre del crate del workspace del que depende.
///
/// El ciclo se busca con un recorrido en profundidad: si al recorrer se
/// vuelve a un nodo ya visitado, hay ciclo.
#[test]
fn workspace_dependency_graph_has_no_cycles() {
    let graph = workspace_graph();
    assert!(
        graph.contains_key("ropero-core"),
        "no se encontró ropero-core en el grafo del workspace"
    );
    let mut cycles: Vec<Vec<String>> = Vec::new();
    for start in graph.keys() {
        if let Some(cycle) = find_cycle(start, &graph, &mut Vec::new(), &mut BTreeSet::new()) {
            cycles.push(cycle);
        }
    }
    assert!(
        cycles.is_empty(),
        "el grafo de dependencias del workspace tiene ciclos: {:?}",
        cycles
    );
}

/// El detector de ciclos **no es vacuo**: encuentra el ciclo que se le da.
///
/// Sin esta prueba, un `find_cycle` que devolviera siempre `None` haría
/// pasar el test de verdad sin comprobar nada. Aquí se le pasa un grafo con
/// un ciclo conocido (`a → b → c → a`) y se exige que lo encuentre, y un
/// grafo sin ciclos y se exige que no halle ninguno.
#[test]
fn cycle_detector_finds_a_cycle_when_there_is_one() {
    let con_ciclo: BTreeMap<String, BTreeSet<String>> =
        [("a", vec!["b"]), ("b", vec!["c"]), ("c", vec!["a"])]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect()))
            .collect();
    let mut path = Vec::new();
    let mut visited = BTreeSet::new();
    let cycle = find_cycle("a", &con_ciclo, &mut path, &mut visited);
    assert!(
        cycle.is_some(),
        "el detector no vio el ciclo a → b → c → a que el grafo de prueba tiene"
    );

    let sin_ciclo: BTreeMap<String, BTreeSet<String>> =
        [("a", vec!["b"]), ("b", vec!["c"]), ("c", vec![])]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into_iter().map(String::from).collect()))
            .collect();
    let mut path = Vec::new();
    let mut visited = BTreeSet::new();
    assert!(
        find_cycle("a", &sin_ciclo, &mut path, &mut visited).is_none(),
        "el detector inventó un ciclo en un grafo que no lo tiene"
    );
}

/// `ropero-core` no depende de ningún crate del workspace.
///
/// Sale del `Cargo.toml` real, no de una lista escrita a mano: si mañana
/// `ropero-core` dependiera de `ropero-cli`, este test lo vería.
#[test]
fn ropero_core_depends_on_no_workspace_crate() {
    let graph = workspace_graph();
    let deps = graph
        .get("ropero-core")
        .expect("ropero-core está en el grafo");
    assert!(
        deps.is_empty(),
        "ropero-core depende de crates del workspace: {:?} — el dominio \
         puro es la base y no puede mirar a nadie",
        deps
    );
}

/// El grafo de dependencias internas del workspace, leído de los
/// `Cargo.toml`.
fn workspace_graph() -> BTreeMap<String, BTreeSet<String>> {
    let crates_dir = repo_root().join("crates");
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(&crates_dir).expect("leer crates/").flatten() {
        let path = entry.path();
        if path.is_dir()
            && let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_string())
            && name.starts_with("ropero-")
        {
            names.push(name);
        }
    }
    names.sort();
    let mut graph: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for name in &names {
        let cargo_toml = crates_dir.join(name).join("Cargo.toml");
        let content = fs::read_to_string(&cargo_toml).expect("leer Cargo.toml");
        let mut deps = BTreeSet::new();
        for section in ["dependencies", "dev-dependencies", "build-dependencies"] {
            for dep in dependencies_of(&content, section) {
                if names.contains(&dep) {
                    deps.insert(dep);
                }
            }
        }
        graph.insert(name.clone(), deps);
    }
    graph
}

/// Los nombres de los crates de los que depende un crate, en una sección
/// dada.
///
/// Se lee el `Cargo.toml` a mano (sin `toml`, que es una dependencia que el
/// core no debe tener): se buscan las claves de primer nivel de la sección
/// y, si la clave tiene forma de `ropero-algo`, se toma el nombre.
fn dependencies_of(content: &str, section: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_section = line == format!("[{}]", section);
            continue;
        }
        if !in_section || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let key = line
            .split_once('=')
            .map(|(k, _)| k.trim().to_string())
            .unwrap_or_default();
        let name = key.trim_matches('"').trim_matches('\'');
        if name.starts_with("ropero-") {
            out.push(name.to_string());
        }
    }
    out
}

/// El primer ciclo que se encuentra al recorrer desde `start`.
///
/// `path` es el camino que se está recorriendo; `visited` son los nodos ya
/// vistos en **este** recorrido. Si se llega a un nodo ya visitado, hay
/// ciclo.
fn find_cycle(
    node: &str,
    graph: &BTreeMap<String, BTreeSet<String>>,
    path: &mut Vec<String>,
    visited: &mut BTreeSet<String>,
) -> Option<Vec<String>> {
    if visited.contains(node) {
        let mut cycle = path.clone();
        cycle.push(node.to_string());
        return Some(cycle);
    }
    visited.insert(node.to_string());
    path.push(node.to_string());
    if let Some(deps) = graph.get(node) {
        for dep in deps {
            if let Some(cycle) = find_cycle(dep, graph, path, visited) {
                return Some(cycle);
            }
        }
    }
    path.pop();
    visited.remove(node);
    None
}

/// `ropero-core` no expone la misma función libre bajo varios nombres.
///
/// Los alias de una función (`artifact_location`, `expected_location`,
/// `location`, `where_lives`, `path_for`…) inflan la API sin añadir nada y
/// hacen imposible saber cuál es la buena: el mismo cuerpo con otro
/// nombre. Aquí se comprueba que no reaparezcan.
///
/// El examen es el de las **funciones libres** (las de módulo): un método
/// puede llamarse igual que otro si vive en otro tipo (`as_str` existe en
/// `ModelFormat` y en `EngineId`, y no son el mismo código).
#[test]
fn ropero_core_no_expose_alias_redundantes() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    // Nombres que ya son alias de otra función libre del core. Añadir uno
    // nuevo es ruido: se comprueba con el nombre, que es lo que un lector
    // ve.
    let alias_conocidos = [
        "artifact_location",
        "expected_location",
        "where_lives",
        "path_for",
        "artifact_path",
    ];
    let mut libres: Vec<(String, PathBuf)> = Vec::new();
    for file in collect_rs(&src) {
        let content = fs::read_to_string(&file).expect("read source file");
        for linea in content.lines() {
            let trimmed = linea.trim();
            // Una función libre empieza en el margen: `pub fn <nombre>(`.
            if linea.starts_with("pub fn ") {
                let nombre: String = trimmed["pub fn ".len()..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !nombre.is_empty() {
                    libres.push((nombre, file.clone()));
                }
            }
        }
    }
    // 1. El nombre de una función libre es único en todo el core.
    let mut nombres: Vec<&str> = libres.iter().map(|(n, _)| n.as_str()).collect();
    nombres.sort_unstable();
    for par in nombres.windows(2) {
        assert_ne!(
            par[0], par[1],
            "la función libre '{0}' está repetida en ropero-core: la API no lleva \
             alias redundantes de la misma función",
            par[0]
        );
    }
    // 2. Y no hay alias de una función que ya existe como método.
    for alias in alias_conocidos {
        let hits: Vec<&PathBuf> = libres
            .iter()
            .filter(|(n, _)| n == alias)
            .map(|(_, f)| f)
            .collect();
        assert!(
            hits.is_empty(),
            "'{alias}' es un alias redundante de una función que ya existe como \
             método: {:?}",
            hits
        );
    }
}
