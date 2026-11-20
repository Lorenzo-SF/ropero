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
//!
//! ## Idioma de la interfaz
//!
//! `clap` trae sus mensajes en inglés («Usage:», «unexpected argument»,
//! «For more information») y su ayuda descuadra las descripciones: los
//! comandos se describen en una columna y las opciones en otra. Por eso
//! aquí `clap` **sólo parsea**: la ayuda la compone el dominio
//! ([`ropero_core::surface`]) y los errores de uso los traduce
//! [`Parser`] al vocabulario del CLI.
//!
//! Los códigos de salida no cambian: la ayuda y la versión salen con `0` y
//! un error de uso con `2` (doc CLI v2 §3).

use std::ffi::OsString;

use clap::error::{ContextKind, ErrorKind};
use clap::{Arg, ArgAction, ArgMatches, Command as ClapCommand};

use ropero_core::surface::{self, FlagKind, resumen_ingles, uso_ingles, valor_ingles};

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

/// Qué ayuda se enseña cuando el usuario la pide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ayuda {
    Binario,
    Arranque,
}

/// El parser del CLI, con la salida en inglés por defecto y español configurable.
///
/// Es el mismo `clap` de siempre, envuelto: parsea igual, pero la ayuda y
/// los errores de uso los pone el dominio. Así el CLI habla **un** idioma
/// (doc CLI v2 §2).
pub struct Parser {
    comando: ClapCommand,
    ayuda: Ayuda,
}

impl Parser {
    /// Parsea los argumentos.
    ///
    /// Si se pide la ayuda o la versión, las imprime y sale con `0`; si el
    /// uso está mal, el error se presenta en el idioma configurado y se sale con `2`.
    pub fn get_matches_from<I, T>(&self, itr: I) -> ArgMatches
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let args: Vec<OsString> = itr.into_iter().map(Into::into).collect();
        match self.comando.clone().try_get_matches_from(args.clone()) {
            Ok(matches) => {
                self.atiende_ayuda(&matches);
                matches
            }
            Err(error) => self.atiende_error(&error, &args),
        }
    }

    /// Si se pidió la ayuda o la versión, las imprime y sale con `0`.
    fn atiende_ayuda(&self, matches: &ArgMatches) {
        let pide_ayuda = matches.get_flag("help")
            || matches
                .subcommand()
                .is_some_and(|(_, sub)| sub.get_flag("help"));
        if pide_ayuda {
            ropero_core::out(&self.texto_de_ayuda(matches), false);
            std::process::exit(0);
        }
        if matches.try_contains_id("version_flag").unwrap_or(false)
            && matches.get_flag("version_flag")
        {
            ropero_core::out_line(&format!("ropero {}", version()));
            std::process::exit(0);
        }
    }

    /// El texto de la ayuda que corresponde a lo que se ha pedido.
    fn texto_de_ayuda(&self, matches: &ArgMatches) -> String {
        match matches.subcommand() {
            Some((cmd, _)) => ayuda_de_comando(cmd),
            None => match self.ayuda {
                Ayuda::Binario => ayuda_del_binario(),
                Ayuda::Arranque => ayuda_de_arranque(),
            },
        }
    }

    /// Traduce el error de `clap` y sale con `2`.
    fn atiende_error(&self, error: &clap::Error, args: &[OsString]) -> ! {
        match error.kind() {
            ErrorKind::DisplayHelp => {
                ropero_core::out(&self.texto_de_ayuda_error(args), false);
                std::process::exit(0);
            }
            ErrorKind::DisplayVersion => {
                ropero_core::out_line(&format!("ropero {}", version()));
                std::process::exit(0);
            }
            _ => {
                if args.iter().any(|a| a == "--help" || a == "-h") {
                    ropero_core::out(&self.texto_de_ayuda_error(args), false);
                    std::process::exit(0);
                }
                let mensaje = surface::usage_message(&self.comando_de_uso(args), &traduce(error));
                ropero_core::err_line(&mensaje);
                std::process::exit(2);
            }
        }
    }

    /// La ayuda que corresponde cuando el parseo falla pero se pidió ayuda.
    fn texto_de_ayuda_error(&self, args: &[OsString]) -> String {
        match self.ayuda {
            Ayuda::Arranque => ayuda_de_arranque(),
            Ayuda::Binario => args
                .iter()
                .skip(1)
                .map(|arg| arg.to_string_lossy().into_owned())
                .find(|arg| es_comando(arg))
                .map(|cmd| ayuda_de_comando(&cmd))
                .unwrap_or_else(ayuda_del_binario),
        }
    }

    /// El comando al que pertenece el error: la pista de dónde está el uso.
    ///
    /// Es el primer token que es un comando del contrato: `ropero install
    /// --bogus` habla del uso de `install`, no del binario entero.
    fn comando_de_uso(&self, args: &[OsString]) -> String {
        match self.ayuda {
            Ayuda::Binario => args
                .iter()
                .skip(1)
                .map(|arg| arg.to_string_lossy().into_owned())
                .find(|arg| es_comando(arg))
                .unwrap_or_default(),
            Ayuda::Arranque => "<modelo>".to_string(),
        }
    }
}

/// Traduce el error de `clap` al vocabulario del CLI.
///
/// `clap` habla inglés; aquí el error dice **qué falla** y **qué hacer**
/// (P-13), en el mismo idioma que el resto de la salida.
fn traduce(error: &clap::Error) -> String {
    let contexto = |kind: ContextKind| {
        error
            .get(kind)
            .map(|valor| valor.to_string())
            .unwrap_or_default()
    };
    match error.kind() {
        ErrorKind::UnknownArgument => {
            format!(
                "opción desconocida: '{}'",
                contexto(ContextKind::InvalidArg)
            )
        }
        ErrorKind::InvalidSubcommand => format!(
            "comando desconocido: '{}'",
            contexto(ContextKind::InvalidSubcommand)
        ),
        ErrorKind::InvalidValue => format!(
            "valor inválido para {}: '{}'",
            contexto(ContextKind::InvalidArg),
            contexto(ContextKind::InvalidValue)
        ),
        ErrorKind::MissingRequiredArgument => {
            format!("falta el argumento {}", contexto(ContextKind::InvalidArg))
        }
        ErrorKind::TooManyValues | ErrorKind::WrongNumberOfValues => {
            format!(
                "demasiados valores para {}",
                contexto(ContextKind::InvalidArg)
            )
        }
        ErrorKind::ArgumentConflict => {
            format!(
                "opciones en conflicto: {}",
                contexto(ContextKind::InvalidArg)
            )
        }
        _ => "no se pudo entender la línea de comandos".to_string(),
    }
}

/// El binario `ropero`.
///
/// `allow_external_subcommands` es lo que permite `ropero <modelo>`: lo
/// que no es un comando conocido se devuelve como externo.
///
/// La plantilla de ayuda y los errores los pone el dominio, no `clap`:
/// `clap` trae "Usage:", "Commands:" y "Options:" en inglés, y un CLI en
/// español que enseña cabeceras en inglés está a medio hacer.
pub fn build() -> Parser {
    let comando = ClapCommand::new("ropero")
        .version(version())
        .about("motores y modelos de inferencia, en una sola herramienta")
        .disable_help_subcommand(true)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .subcommand_required(false)
        .arg_required_else_help(false)
        .arg(flag_help())
        .arg(flag_version())
        .subcommands(comandos());
    Parser {
        comando,
        ayuda: Ayuda::Binario,
    }
}

/// Si un token es el nombre de un comando del contrato.
///
/// Es la pregunta que decide si lo que sigue al binario es un **comando** o
/// el **nombre de un modelo** (doc CLI v2 §8).
pub fn es_comando(token: &str) -> bool {
    surface::COMMANDS.iter().any(|c| c.name == token)
}

/// La ayuda de un comando concreto.
fn ayuda_de_comando(cmd: &str) -> String {
    let Some(comando) = surface::COMMANDS.iter().find(|c| c.name == cmd) else {
        return surface::help_text();
    };
    let english = idioma() == "EN";
    let palette = ropero_core::Palette::color();
    let title = if english {
        resumen_ingles(cmd)
    } else {
        comando.summary
    };
    let mut out = format!(
        "{}\n\n{}  ropero {}\n",
        palette.paint_hex(ropero_core::palette::HELP_HEADING, title),
        palette.paint_hex(
            ropero_core::palette::HELP_HEADING,
            if english { "USAGE:" } else { "USO:" }
        ),
        if english {
            uso_ingles(comando.usage)
        } else {
            comando.usage.to_string()
        }
    );
    let flags = surface::flags_de(cmd);
    if !flags.is_empty() {
        out.push_str(&format!(
            "\n{}\n",
            palette.paint_hex(
                ropero_core::palette::HELP_HEADING,
                if english { "OPTIONS" } else { "OPCIONES" }
            )
        ));
        for flag in flags {
            let nombre = match flag.kind {
                surface::FlagKind::Switch => format!("--{}", flag.name),
                surface::FlagKind::Value | surface::FlagKind::Many => {
                    let valor = if english {
                        valor_ingles(flag.value_name.unwrap_or("VALOR"))
                    } else {
                        flag.value_name.unwrap_or("VALOR")
                    };
                    format!("--{} <{}>", flag.name, valor)
                }
            };
            out.push_str(&format!(
                "  {}:  {}\n",
                palette.paint_hex(ropero_core::palette::HELP_HEADING, &nombre),
                if english {
                    ayuda_flag_ingles(cmd, flag.name)
                } else {
                    flag.help
                }
            ));
        }
        out.push_str(if english {
            "  -h, --help:  show this help\n"
        } else {
            "  -h, --help:  esta ayuda\n"
        });
    }
    if cmd == "config" {
        out.push('\n');
        out.push_str(surface::CONFIG_KEYS_USAGE);
        out.push('\n');
    }
    if cmd == "run" {
        out.push_str(if english {
            "\nModel-specific levels, flags, and options come from the model TOML.\nUse `ropero info <model>` to inspect them; undeclared options are ignored.\n"
        } else {
            "\nLos niveles, flags y opciones propios del modelo salen de su TOML.\nUsa `ropero info <modelo>` para consultarlos; los no declarados se ignoran.\n"
        });
    }
    if cmd == "models" {
        out.push_str(if english {
            "\nAdd a model with an interactive wizard, flags, `--path FILE`, or `--stdin`.\nExamples:\n  ropero models add\n  ropero models add --path model.toml\n  cat model.toml | ropero models add --stdin\n"
        } else {
            "\nAñade un modelo con el asistente, flags, `--path FICHERO` o `--stdin`.\nEjemplos:\n  ropero models add\n  ropero models add --path modelo.toml\n  cat modelo.toml | ropero models add --stdin\n"
        });
    }
    out.push_str(&format!(
        "\n  {}: ropero {cmd} --help\n",
        if english { "more" } else { "más" }
    ));
    out
}

/// La ayuda del arranque de un modelo.
fn ayuda_de_arranque() -> String {
    let english = idioma() == "EN";
    let mut out = format!(
        "{}\n\n{}:  ropero {}\n\n{}\n",
        if english {
            "Start a model using its configured engine"
        } else {
            surface::help::RUN
        },
        if english { "USAGE" } else { "USO" },
        if english {
            uso_ingles(surface::RUN_USAGE)
        } else {
            surface::RUN_USAGE.to_string()
        },
        if english { "OPTIONS" } else { "OPCIONES" },
    );
    for flag in surface::flags_de("run") {
        let nombre = match flag.kind {
            surface::FlagKind::Switch => format!("--{}", flag.name),
            surface::FlagKind::Value | surface::FlagKind::Many => {
                let valor = if english {
                    valor_ingles(flag.value_name.unwrap_or("VALOR"))
                } else {
                    flag.value_name.unwrap_or("VALOR")
                };
                format!("--{} <{}>", flag.name, valor)
            }
        };
        out.push_str(&format!(
            "  {nombre}:  {}\n",
            if english {
                ayuda_flag_ingles("run", flag.name)
            } else {
                flag.help
            }
        ));
    }
    out.push_str(if english {
        "  -h, --help:  show this help\n\nModel-specific levels, flags, and options are read from the model TOML.\nUse `ropero info <model>` to inspect them; undeclared options are ignored.\n\n  more: ropero run <model> --help\n"
    } else {
        "  -h, --help:  esta ayuda\n\nLos niveles, flags y opciones propios del modelo salen de su TOML.\nUsa `ropero info <modelo>` para consultarlos; los no declarados se ignoran.\n\n  más: ropero run <modelo> --help\n"
    });
    out
}

/// La ayuda completa del binario.
fn ayuda_del_binario() -> String {
    let english = idioma() == "EN";
    let mut entradas: Vec<(String, String)> = surface::COMMANDS
        .iter()
        .map(|c| (c.usage.to_string(), c.summary.to_string()))
        .collect();
    entradas.push((
        surface::RUN_USAGE.to_string(),
        surface::RUN_SUMMARY.to_string(),
    ));
    entradas.push(("--help".to_string(), "esta ayuda".to_string()));
    let mut out = if english {
        String::from(
            "local inference engines and models in one tool\n\nUSAGE: ropero <command> [options]\n\nCOMMANDS:\n",
        )
    } else {
        String::from(
            "motores y modelos de inferencia, en una sola herramienta\n\nUSO: ropero <comando> [opciones]\n\nCOMANDOS:\n",
        )
    };
    for (uso, resumen) in &entradas {
        let name = uso.split_whitespace().next().unwrap_or_default();
        out.push_str(&format!(
            "  {}\n    {}\n",
            if english {
                uso_ingles(uso)
            } else {
                uso.clone()
            },
            if english {
                resumen_ingles(name)
            } else {
                resumen
            }
        ));
    }
    out.push_str(if english {
        "\n  ropero <command> --help\n"
    } else {
        "\n  ropero <comando> --help\n"
    });
    out
}

pub(crate) fn idioma() -> String {
    let env = crate::discovery::capture_environment();
    ropero_runtime::carga_config(&env)
        .map(|(_, config, _)| config.lang)
        .unwrap_or_else(|_| "EN".into())
}

fn ayuda_flag_ingles(command: &str, name: &str) -> &'static str {
    match (command, name) {
        ("status", "live") => "refreshes the terminal view; interval in seconds (default: 1)",
        ("config", "lang") => "interface language: ES or EN",
        ("connect", "list") => "lists supported integrations",
        ("connect", "model") => "model names separated by commas or semicolons",
        ("connect", "agent") => "target agent",
        ("connect", "remove") => "removes the selected connections",
        ("models", "all") | ("engines", "all") => "also shows unavailable engines and models",
        ("engine", "params") => "shows common arguments configured for engines",
        ("models", "fired") => "shows only fired models",
        ("model", "fired") => "deactivates the model and keeps its TOML",
        ("model", "hired") => "reactivates the model from fired/",
        ("models", "path") => "reads a model TOML file",
        ("models", "stdin") => "reads the complete TOML from standard input",
        ("models", "name") => "internal model name and filename",
        ("models", "engine") => "model engine",
        ("models", "format") => "artifact format",
        ("models", "artifact") => "model artifact path or filename",
        ("models", "repo") => "download or build repository",
        ("models", "context") => "context window in tokens",
        ("models", "alias") => "public engine-facing model id",
        ("models", "set") => "extra TOML value, repeatable; use block paths such as args.base",
        ("run", "force") => "stops Ropero-managed processes occupying the device",
        ("run", "cpu") => "forces all offload to CPU and RAM",
        ("run", "port") => "sets the listener port",
        ("run", "api-key") => "sets the API gateway key",
        ("run", "no-api-key") => "starts without an API key",
        ("run", "ctx") => "overrides the context window",
        ("run", "log-level") => "log level: error, warn, info, debug, or trace",
        ("run", "background") => "detaches and saves engine logs",
        ("install", "prio") => "installation method: download, build, or tool (repeatable)",
        ("install", "force") => "removes and reinstalls the selected engine sources",
        ("gguffer", "repo") => "Hugging Face source repository",
        ("gguffer", "path") => "local source weights directory",
        ("gguffer", "out") => "output GGUF name or path",
        ("gguffer", "quant") => "quantization type(s)",
        ("gguffer", "force") => "overwrites the output and forces conversion",
        ("gguffer", "turbo-llama") => "uses the compiled Turbo-Llama tools",
        ("gguffer", "gguf") => "existing GGUF source file or repository",
        ("gguffer", "imatrix") => "local importance matrix file",
        ("gguffer", "imatrix-repo") => {
            "remote importance-matrix repository; requires --imatrix-file"
        }
        ("gguffer", "imatrix-file") => "remote importance-matrix filename; requires --imatrix-repo",
        ("gguffer", "out-principal") => "exact output path for the first quantization",
        ("gguffer", "intermedio") => "intermediate format: F16 or BF16",
        ("gguffer", "keep-source") => "keeps the source weights",
        ("gguffer", "keep-intermedio") => "keeps the intermediate file",
        ("gguffer", "no-mtp") => "builds without MTP headers",
        ("gguffer", "no-dflash") => "builds without DFlash headers",
        ("gguffer", "list-quants") => "lists available quantization types",
        ("gguffer", "diagnostico") => "shows planned actions without writing files",
        ("test", "model") => "one available model; its engine is stopped when testing ends",
        ("test", "all-tests") => "run every test for the selected model(s)",
        ("test", "all-test") => "run every test for every available model",
        ("test", "all-models") => {
            "test every available model in sequence and stop its engine after each run"
        }
        ("test", "test-group") => "run one test group (exclusive with --test)",
        ("test", "test") => "run one test by id or filename",
        ("test", "path") => "directory for test reports (default: current directory)",
        _ => "see the command documentation",
    }
}

/// El flag `--help`, en español.
///
/// `clap` lo pone en inglés ("Print help") y además lo pinta él: aquí el
/// flag sólo dice que se pidió la ayuda y la pinta el dominio.
fn flag_help() -> Arg {
    Arg::new("help")
        .long("help")
        .short('h')
        .action(ArgAction::SetTrue)
        .help("esta ayuda")
}

/// El flag `--version`, en español.
///
/// `clap` lo pone en inglés ("Print version").
fn flag_version() -> Arg {
    Arg::new("version_flag")
        .long("version")
        .short('V')
        .action(ArgAction::SetTrue)
        .help("la versión")
}

/// Los subcomandos del contrato, con sus flags y sus argumentos.
fn comandos() -> Vec<ClapCommand> {
    let mut todos: Vec<ClapCommand> = Vec::new();
    for comando in surface::COMMANDS {
        let mut clap = ClapCommand::new(comando.name)
            .about(comando.summary)
            .long_about(comando.summary)
            .disable_help_flag(true)
            .arg(flag_help());
        for flag in surface::flags_de(comando.name) {
            if comando.name == "status" && flag.name == "live" {
                clap = clap.arg(
                    Arg::new("live")
                        .long("live")
                        .num_args(0..=1)
                        .default_missing_value("1")
                        .value_parser(clap::value_parser!(f64))
                        .help(flag.help),
                );
            } else {
                let mut arg = flag_de(flag);
                if comando.name == "gguffer" && flag.name == "turbo-llama" {
                    arg = arg.visible_alias("turbo");
                }
                if comando.name == "test" && flag.name == "model" {
                    arg = arg.conflicts_with("all-models").value_delimiter(',');
                } else if comando.name == "test" && flag.name == "all-models" {
                    arg = arg.conflicts_with("model");
                } else if comando.name == "test"
                    && (flag.name == "test" || flag.name == "test-group")
                {
                    arg = arg.value_delimiter(',');
                } else if comando.name == "model" && flag.name == "fired" {
                    arg = arg.conflicts_with("hired").required_unless_present("hired");
                } else if comando.name == "model" && flag.name == "hired" {
                    arg = arg.conflicts_with("fired").required_unless_present("fired");
                }
                clap = clap.arg(arg);
            }
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
pub fn arranque() -> Parser {
    let mut comando = ClapCommand::new("ropero <modelo>")
        .about(surface::help::RUN)
        .long_about(surface::help::RUN)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .arg(flag_help())
        .arg(Arg::new("modelo").value_name("MODELO").required(true));
    for flag in surface::flags_de("run") {
        comando = comando.arg(flag_de(flag));
    }
    Parser {
        comando,
        ayuda: Ayuda::Arranque,
    }
}

/// Los argumentos posicionales de un comando.
///
/// Los del contrato: `install` recibe qué instalar y la lista;
/// `stop`, `info` y `command` reciben un modelo.
fn posicionales(cmd: &str) -> Vec<Arg> {
    match cmd {
        "models" => vec![
            Arg::new("action")
                .value_name("ACCIÓN")
                .num_args(0..=1)
                .conflicts_with_all(["all", "fired"]),
        ],
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
        "engine" => vec![Arg::new("engine").value_name("ENGINE")],
        "info" | "command" | "model" => vec![
            Arg::new("modelo")
                .value_name("MODELO")
                .required(true)
                .help("el modelo del catálogo"),
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
