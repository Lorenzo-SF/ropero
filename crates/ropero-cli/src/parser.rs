//! El parser de la línea de comandos.
//!
//! El contrato de los argumentos —qué comandos hay, qué flags acepta cada
//! uno y qué frase tiene— vive en el dominio
//! ([`ropero_core::surface`]). Aquí sólo se **traduce** ese contrato a
//! `clap`, y se lee lo que `clap` devuelve.
//!
//! ## Por qué el builder y no el derive
//!
//! Porque el derive **duplicaría** el contrato: cada flag volvería a
//! estar escrito en el `#[arg(...)]`, y un cambio en la especificación
//! obligaría a tocar dos sitios (y a recordar el segundo). Con el builder,
//! la tabla de `surface` es la única fuente: los flags del doc y los flags
//! que el parser acepta son **la misma cosa** por construcción.
//!
//! ## El arranque de un modelo
//!
//! `ropero <modelo>` no es un comando con nombre fijo: lo que sigue al
//! binario es el **nombre del modelo**. Por eso el parser tiene un
//! `external_subcommand`: lo que no reconoce como comando se devuelve tal
//! cual, y es el arranque de un modelo (doc CLI v2 §8).

use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use ropero_core::surface::{self, FlagKind};

/// La versión, con el build y el autor.
///
/// El doc pide que la versión, la fecha de compilación y el autor sean
/// accesibles desde `--version`: por eso la cadena del flag lleva las tres,
/// no sólo el número de la versión.
///
/// `clap` quiere un `&'static str`: la cadena se construye una vez (la
/// primera vez que se llama) y se guarda en un [`OnceLock`], así que no se
/// fuga memoria en un programa de una sola vida.
pub fn version() -> &'static str {
    static VERSION: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    VERSION.get_or_init(|| {
        format!(
            "{} (build {}) — {}",
            env!("ROPERO_VERSION"),
            env!("ROPERO_BUILD_DATE"),
            env!("ROPERO_AUTHOR")
        )
    })
}

/// El binario `ropero`.
///
/// `allow_external_subcommands` es lo que permite `ropero <modelo>`: lo
/// que no es un comando conocido se devuelve como externo.
///
/// La plantilla de ayuda está en español (doc CLI v2 §2): `clap` trae
/// "Usage:", "Commands:" y "Options:" en inglés, y un CLI en español que
/// enseña cabeceras en inglés está a medio hacer.
pub fn build() -> ClapCommand {
    ClapCommand::new("ropero")
        .version(version())
        .about("motores y modelos de inferencia, en una sola herramienta")
        .disable_help_subcommand(true)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .help_template(TEMPLATE)
        .arg(flag_help())
        .arg(flag_version())
        .subcommands(comandos())
}

/// Si un token es el nombre de un comando del contrato.
///
/// Es la pregunta que decide si lo que sigue al binario es un **comando** o
/// el **nombre de un modelo** (doc CLI v2 §8).
pub fn es_comando(token: &str) -> bool {
    surface::COMMANDS.iter().any(|c| c.name == token)
}

/// El flag `--help`, en español.
///
/// `clap` lo pone en inglés ("Print help"): el doc pide las frases en
/// español, así que se sustituye.
fn flag_help() -> Arg {
    Arg::new("help")
        .long("help")
        .short('h')
        .action(ArgAction::Help)
        .help("esta ayuda")
}

/// El flag `--version`, en español.
///
/// `clap` lo pone en inglés ("Print version").
fn flag_version() -> Arg {
    Arg::new("version")
        .long("version")
        .short('V')
        .action(ArgAction::Version)
        .help("la versión")
}

/// La plantilla de la ayuda, en español.
const TEMPLATE: &str = "\
{about-with-newline}
USO: {usage}

COMANDOS:
{subcommands}
{options}
más: ropero <comando> --help
";

/// Los subcomandos del contrato, con sus flags y sus argumentos.
fn comandos() -> Vec<ClapCommand> {
    let mut todos: Vec<ClapCommand> = Vec::new();
    for comando in surface::COMMANDS {
        let mut clap = ClapCommand::new(comando.name)
            .about(comando.summary)
            .long_about(comando.summary)
            .help_template(TEMPLATE_SUB)
            .disable_help_flag(true)
            .arg(flag_help());
        for flag in surface::flags_de(comando.name) {
            clap = clap.arg(flag_de(flag));
        }
        clap = clap.args(posicionales(comando.name));
        todos.push(clap);
    }
    todos
}

/// El parser del **arranque de un modelo**: `ropero <modelo> [--force] …`.
///
/// Es un parser aparte porque `ropero <modelo>` no es un comando con nombre
/// fijo: lo que sigue al binario puede ser un comando o el nombre de un
/// modelo. La decisión de cuál de las dos cosas es se toma **antes** de
/// parsear (en [`crate::dispatch`]), y este parser sólo se usa en el
/// segundo caso.
pub fn arranque() -> ClapCommand {
    let mut clap = ClapCommand::new("ropero <modelo>")
        .about(surface::help::RUN)
        .long_about(surface::help::RUN)
        .help_template(TEMPLATE_SUB)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(flag_help())
        .arg(Arg::new("modelo").value_name("MODELO").required(true));
    for flag in surface::flags_de("run") {
        clap = clap.arg(flag_de(flag));
    }
    clap
}

/// La plantilla de la ayuda de un subcomando, en español.
const TEMPLATE_SUB: &str = "\
{about-with-newline}
USO: {usage}

{options}
más: ropero <comando> --help
";

/// Los argumentos posicionales de un comando.
///
/// Los del contrato: `install` recibe qué instalar y la lista;
/// `stop` y `levels` y `path` reciben un modelo; `args` recibe el modelo y
/// los tokens sueltos.
fn posicionales(cmd: &str) -> Vec<Arg> {
    match cmd {
        "install" => vec![
            Arg::new("que")
                .value_name("QUE")
                .help("qué instalar: engines, models, o los dos"),
            Arg::new("lista")
                .value_name("MODELO")
                .num_args(0..)
                .help("los ids o alias de lo que se instala"),
        ],
        "stop" => vec![
            Arg::new("modelo")
                .value_name("MODELO")
                .help("el modelo cuyo motor se detiene"),
        ],
        "levels" | "path" => vec![
            Arg::new("modelo")
                .value_name("MODELO")
                .required(true)
                .help("el modelo del catálogo"),
        ],
        "args" => vec![
            Arg::new("modelo")
                .value_name("MODELO")
                .required(true)
                .help("el modelo del catálogo"),
            Arg::new("tokens")
                .value_name("ARGUMENTOS")
                .num_args(0..)
                .allow_hyphen_values(true)
                .help("los tokens del modelo: niveles, flags y opciones"),
        ],
        _ => Vec::new(),
    }
}

/// Un flag del contrato, como `clap` lo entiende.
fn flag_de(flag: &surface::Flag) -> Arg {
    let mut arg = Arg::new(flag.name).long(flag.name).help(flag.help);
    arg = match flag.kind {
        FlagKind::Switch => arg.action(ArgAction::SetTrue),
        FlagKind::Value => arg.action(ArgAction::Set),
        FlagKind::Many => arg.action(ArgAction::Append),
    };
    if let Some(valor) = flag.value_name {
        arg = arg.value_name(valor);
    }
    if let Some(defecto) = flag.default {
        arg = arg.default_value(defecto);
    }
    if let Some(requiere) = flag.requires {
        arg = arg.requires(requiere);
    }
    arg
}

/// El modelo de un comando que opera sobre un modelo del catálogo.
pub fn modelo(matches: &ArgMatches) -> String {
    matches
        .get_one::<String>("modelo")
        .cloned()
        .unwrap_or_default()
}

/// Los tokens sueltos de `ropero args`.
///
/// Los niveles, flags y opciones del modelo son **dinámicos**: los declara
/// su TOML, así que no pueden estar en el parser. Se capturan como
/// tokens sueltos y los valida el dominio.
pub fn tokens(matches: &ArgMatches) -> Vec<String> {
    matches
        .get_many::<String>("tokens")
        .map(|v| v.cloned().collect())
        .unwrap_or_default()
}

/// Los tokens de `ropero install`, **en el orden en que los escribió el
/// usuario**.
///
/// El orden **importa**: el `--prio` global va antes de la lista de modelos
/// y el por modelo detrás (doc B §4.10.7). `clap` separa los flags de los
/// posicionales, así que el orden se reconstruye con los **índices** que
/// `clap` guarda de cada valor.
pub fn install_tokens(matches: &ArgMatches) -> Vec<ropero_core::install::Token> {
    let mut tokens: Vec<(usize, ropero_core::install::Token)> = Vec::new();
    let lista = matches.get_many::<String>("lista");
    let indices = matches.indices_of("lista");
    if let (Some(valores), Some(indices)) = (lista, indices) {
        for (valor, indice) in valores.zip(indices) {
            tokens.push((
                indice,
                ropero_core::install::Token::Modelo(valor.to_string()),
            ));
        }
    }
    let prios = matches.get_many::<String>("prio");
    let indices = matches.indices_of("prio");
    if let (Some(valores), Some(indices)) = (prios, indices) {
        for (valor, indice) in valores.zip(indices) {
            tokens.push((indice, ropero_core::install::Token::Prio(valor.to_string())));
        }
    }
    tokens.sort_by_key(|(indice, _)| *indice);
    tokens.into_iter().map(|(_, token)| token).collect()
}
