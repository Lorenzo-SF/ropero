//! `ropero install`: los motores y los modelos.
//!
//! La CLI sólo **pega** (R-ARQ-1): el contrato de `--prio`, los menús y
//! los mensajes son del dominio ([`ropero_core::install`]), y la E/S —el
//! terminal, el binario del motor y el enlace— es del runtime
//! ([`ropero_runtime::install`]).
//!
//! ## Las tres formas de invocarlo
//!
//! | Invocación | Qué hace |
//! |---|---|
//! | `ropero install` | engines y luego models, en ese orden |
//! | `ropero install engines [<lista>]` | instala motores |
//! | `ropero install models [<lista>]` | resuelve modelos y decide su vía |
//!
//! Con **lista** se instala **directo** (modo desatendido, R-CLI-16); sin
//! lista hay **menú**, y sin terminal el menú es un **error de uso**
//! (código 2).

use clap::ArgMatches;

use ropero_core::Palette;
use ropero_core::install::{self, Decision, Token, Via};
use ropero_core::{EngineId, EngineRow};

/// `ropero install [<nada>|engines|models]`.
pub fn run(sub: &ArgMatches) -> i32 {
    let force = sub.get_flag("force");
    if force && sub.get_one::<String>("que").map(String::as_str) == Some("models") {
        return uso("'--force' sólo se admite con 'ropero install engines'");
    }
    let Ok(s) = crate::discovery::scan() else {
        return 1;
    };
    let tokens = crate::parser::install_tokens(sub);
    match sub.get_one::<String>("que").map(String::as_str) {
        Some("engines") => {
            // El `--prio` sólo vale para los modelos (doc CLI §9.3): aquí
            // no hay vías que elegir, así que no se ignora en silencio.
            if tokens.iter().any(|t| matches!(t, Token::Prio(_))) {
                return uso(&install::prio_solo_para_models());
            }
            engines(&s, &tokens, force)
        }
        Some("models") => models(&s, &tokens),
        Some(otro) => uso(&format!(
            "'install' sólo acepta 'engines' o 'models' (recibido: '{otro}')"
        )),
        None => {
            // `ropero install` a secas: engines y luego models (doc CLI v2 §9).
            let codigo = engines(&s, &tokens, force);
            if codigo != 0 {
                return codigo;
            }
            models(&s, &tokens)
        }
    }
}

/// `ropero install engines [<lista>]`.
fn engines(s: &crate::discovery::Scan, tokens: &[Token], force: bool) -> i32 {
    let sistema = ropero_runtime::capture_system_info();
    let filas = ropero_core::engine_rows(&s.engines, &sistema, true);
    let lista = install::nombres_de(tokens);
    let fastflow_manual = filas.iter().find(|fila| {
        fila.manifest.id == EngineId::FastFlowLm
            && fila.compatible
            && fila.manifest.install.as_ref().is_some_and(|spec| {
                ropero_runtime::install::gestor_paquetes_disponible(spec, &s.env).is_none()
            })
    });
    if fastflow_manual.is_some() {
        aviso_fastflow_manual();
        if lista
            .iter()
            .any(|nombre| matches!(nombre.as_str(), "fastflowlm" | "flm"))
        {
            return 1;
        }
    }
    let eleccion = if lista.is_empty() {
        match elegir_motores(&filas, &s.env) {
            Ok(elegidos) => elegidos,
            Err(codigo) => return codigo,
        }
    } else {
        lista
    };
    let elegidos = match install::motores_elegidos(&filas, &eleccion) {
        Ok(elegidos) => elegidos,
        Err(e) => return uso(&e.to_string()),
    };
    for aviso in &elegidos.avisos {
        ropero_core::err_line(&format!("aviso: {aviso}"));
    }
    for fila in elegidos.instalar {
        let codigo = enlaza_motor(&fila.manifest, &s.env, force);
        if codigo != 0 {
            return codigo;
        }
    }
    0
}

/// Compila el motor si su manifiesto declara receta y finalmente crea el enlace.
fn enlaza_motor(
    manifest: &ropero_core::EngineManifest,
    env: &ropero_core::Environment,
    force: bool,
) -> i32 {
    match ropero_runtime::install::instala_motor(manifest, env, force) {
        Ok(estado) => {
            let (mensaje, codigo) = install::mensaje_motor(manifest.id.as_str(), &estado);
            if codigo == 0 {
                ropero_core::out_line(&mensaje);
            } else {
                ropero_core::err_line(&mensaje);
            }
            codigo
        }
        Err(e) => {
            ropero_core::err_line(&format!("error: {e}"));
            1
        }
    }
}

/// El menú de motores: sin terminal es un **error de uso** (R-INST-26).
fn elegir_motores(filas: &[EngineRow], env: &ropero_core::Environment) -> Result<Vec<String>, i32> {
    if !ropero_runtime::install::hay_tty() {
        return Err(uso(&install::falta_tty_engines()));
    }
    let compatibles: Vec<install::MotorDelMenu<'_>> = filas
        .iter()
        .filter(|f| {
            f.compatible
                && !(f.manifest.id == EngineId::FastFlowLm
                    && f.manifest.install.as_ref().is_some_and(|spec| {
                        ropero_runtime::install::gestor_paquetes_disponible(spec, env).is_none()
                    }))
        })
        .map(|f| install::MotorDelMenu {
            manifest: &f.manifest,
            instalado: ropero_runtime::engine_installed_with_env(&f.manifest, env),
        })
        .collect();
    let menu = install::menu_engines(&compatibles, &Palette::color());
    if menu.lineas.is_empty() {
        ropero_core::err_line("aviso: no hay motores compatibles con esta máquina");
        return Err(0);
    }
    ropero_runtime::install::pregunta_menu(&menu.lineas, &menu.nombres).map_err(|e| uso(&e))
}

fn aviso_fastflow_manual() {
    ropero_core::err_line(
        "fastflowlm: hay una NPU compatible, pero no encuentro apt-get ni pacman; instala FLM y sus dependencias manualmente según la guía oficial de Linux",
    );
}

/// `ropero install models [<lista>]`.
fn models(s: &crate::discovery::Scan, tokens: &[Token]) -> i32 {
    let prios = match install::asocia_tokens(tokens) {
        Ok(prios) => prios,
        Err(e) => return uso(&e.to_string()),
    };
    let lista = install::nombres_de(tokens);
    // Con lista se instala **directo** (desatendido): el usuario ya ha
    // dicho qué quiere, así que no se pregunta (doc CLI v2 §9.2).
    let eleccion = if lista.is_empty() {
        match elegir_modelos(s) {
            Ok(elegidos) => elegidos,
            Err(codigo) => return codigo,
        }
    } else {
        lista
    };
    let elegidos = match install::modelos_elegidos(&s.catalog, &eleccion, &prios) {
        Ok(elegidos) => elegidos,
        Err(e) => return uso(&e.to_string()),
    };
    // R-MOD-27: **elegir un modelo ES consentir su descarga**. En el menú
    // y con lista, la pregunta del tamaño (con su default «no») no se
    // repite: la biblioteca conserva el modo interrogativo, pero el
    // comando no lo pide.
    let modo = ropero_download::Modo::Desatendido;
    let mut codigo = 0;
    for elegido in elegidos {
        // Un `--prio` ignorado **se avisa** y no aborta (doc B §4.10.7).
        if let Some(aviso) = &elegido.aviso {
            ropero_core::err_line(aviso);
        }
        let engine_manifest = s.engines.iter().find(|m| m.id == elegido.spec.model.engine);
        if elegido.spec.model.format == ropero_core::ModelFormat::Ninfer
            && elegido.spec.model.engine == ropero_core::EngineId::Ignis
        {
            let Some(manifest) = engine_manifest else {
                ropero_core::err_line("error: falta el manifiesto de Ignis");
                codigo = 1;
                continue;
            };
            match ropero_runtime::ignis::pull_model(manifest, elegido.spec, &s.env) {
                Ok(0) => {
                    ropero_core::out_line(&format!("{}: descargado con Ignis", elegido.nombre));
                }
                Ok(status) => {
                    ropero_core::err_line(&format!(
                        "error: '{}' no se pudo descargar con Ignis (código {status})",
                        elegido.nombre
                    ));
                    codigo = 1;
                }
                Err(error) => {
                    ropero_core::err_line(&format!("error: {error}"));
                    codigo = 1;
                }
            }
            continue;
        }
        let flm_delegado = ropero_core::flm::is_flm_model(elegido.spec)
            && engine_manifest.is_some_and(ropero_runtime::flm::delegated);
        if flm_delegado {
            let manifest = engine_manifest.expect("comprobado");
            if baja_modelo_flm(&elegido.nombre, elegido.spec, manifest) != 0 {
                codigo = 1;
            }
            continue;
        }
        let via = match install::elegir_via(elegido.spec, elegido.prio) {
            Ok(Decision::Via(via)) => via,
            Ok(Decision::Pregunta) => match preguntar_via(&elegido.nombre) {
                Ok(via) => via,
                Err(codigo) => return codigo,
            },
            Err(e) => {
                ropero_core::err_line(&format!("error: {e}"));
                return e.exit_code();
            }
        };
        match via {
            Via::Download => {
                if baja_modelo(&elegido.nombre, elegido.spec, &s.env, modo) != 0 {
                    codigo = 1;
                }
            }
            Via::Build => {
                if fabrica_modelo(&elegido.nombre, elegido.spec, &s.env) != 0 {
                    codigo = 1;
                }
            }
        }
    }
    codigo
}

/// Fabrica el artefacto de catálogo delegando toda la lógica a ropero-forge.
fn fabrica_modelo(
    nombre: &str,
    spec: &ropero_core::ModelSpec,
    env: &ropero_core::Environment,
) -> i32 {
    ropero_core::out_line(&format!("{nombre}: vía build"));
    let Some(build) = spec.build.as_ref() else {
        ropero_core::err_line(&format!("error: {nombre} no declara una receta build"));
        return 1;
    };
    let mut flags = ropero_forge::peticion::Flags {
        turbo_llama: spec.model.engine == ropero_core::EngineId::TurboLlama,
        keep_source: build.keep_source,
        keep_intermedio: build.keep_intermedio,
        no_mtp: build.no_mtp,
        force: build.force,
        ..Default::default()
    };
    if !build.repo.is_empty() {
        if build.gguf {
            let file = if build.gguf_file.is_empty() {
                spec.files
                    .gguf
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                build.gguf_file.clone()
            };
            flags.gguf = Some(if file.is_empty() {
                build.repo.clone()
            } else {
                format!("{}/{}", build.repo, file)
            });
        } else {
            flags.repo = Some(build.repo.clone());
        }
    }
    flags.quants = build
        .quant
        .split([',', ';'])
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(str::to_string)
        .collect();
    if !build.imatrix.is_empty() {
        flags.imatrix = Some(build.imatrix.clone());
    }
    if !build.imatrix_repo.is_empty() {
        flags.imatrix_repo = Some(build.imatrix_repo.clone());
    }
    if !build.imatrix_file.is_empty() {
        flags.imatrix_file = Some(build.imatrix_file.clone());
    }
    if !build.intermedio.is_empty() {
        flags.intermedio = Some(build.intermedio.clone());
    }
    if !build.out.is_empty() {
        flags.out = Some(build.out.clone());
    }
    if !build.out_principal.is_empty() {
        flags.out_principal = Some(build.out_principal.clone());
    }
    if flags.out.is_none() && flags.out_principal.is_none() && spec.files.gguf.is_some() {
        if let Some(gguf) = &spec.files.gguf {
            let target = env
                .engine_artifact_path(spec.model.engine, "GGUF_DIR", "models/gguf")
                .join(gguf);
            flags.out_principal = Some(target.to_string_lossy().into_owned());
        }
    }
    let reporte = ropero_forge::gguffer(flags, env);
    for linea in reporte.lineas {
        ropero_core::out_line(&linea);
    }
    if let Some(error) = reporte.error {
        ropero_core::err_line(&format!("error: {error}"));
        if let ropero_forge::Error::Conversion { cola, .. } = &error {
            for linea in cola.iter().rev().take(12).rev() {
                ropero_core::err_line(&format!("  {linea}"));
            }
        }
        if let Some(pista) = error.pista() {
            ropero_core::err_line(&pista);
        }
        1
    } else {
        ropero_core::out_line(&format!("{nombre}: construido"));
        0
    }
}

/// Baja un modelo de verdad: la vía `download` llama a `hf`.
///
/// El plan lo decide `ropero-download` (qué se baja, a dónde y con qué
/// argumentos) y la ejecución es suya (R-MOD-16). Aquí sólo se informa del
/// resultado, una línea por modelo.
fn baja_modelo(
    nombre: &str,
    spec: &ropero_core::ModelSpec,
    env: &ropero_core::Environment,
    modo: ropero_download::Modo,
) -> i32 {
    if spec.download.as_ref().is_some_and(|d| d.manual) {
        ropero_core::err_line(&format!(
            "aviso: '{nombre}' es de descarga manual (grande): se baja por petición explícita"
        ));
    }
    let plan = match ropero_download::plan_de_descarga(spec, env) {
        Ok(plan) => plan,
        Err(e) => return informa_error(nombre, &e),
    };
    match ropero_download::descarga(&plan, env, modo) {
        Ok(resultado) => {
            ropero_core::out_line(&ropero_download::linea(nombre, Via::Download, resultado));
            0
        }
        Err(e) => informa_error(nombre, &e),
    }
}

/// Informa de un error de descarga: la línea del modelo y su pista.
///
/// La línea va a **stderr**: stdout es para los datos (R-MOD-11) y un
/// error es todo lo humano.
fn informa_error(nombre: &str, e: &ropero_download::Error) -> i32 {
    ropero_core::err_line(&ropero_download::linea_error(nombre, Via::Download, e));
    if let Some(pista) = e.pista() {
        ropero_core::err_line(&pista);
    }
    1
}

/// Instala un modelo FLM delegando en el almacén del motor (`flm pull <tag>`).
fn baja_modelo_flm(
    nombre: &str,
    spec: &ropero_core::ModelSpec,
    manifest: &ropero_core::EngineManifest,
) -> i32 {
    match ropero_runtime::flm::pull_model(manifest, spec) {
        Ok(resultados) => {
            let mut ok = true;
            for res in &resultados {
                if !res.stdout.is_empty() {
                    ropero_core::out(&res.stdout, false);
                }
                if !res.stderr.is_empty() {
                    ropero_core::err_line(res.stderr.trim_end());
                }
                if !res.ok() {
                    ropero_core::err_line(&format!(
                        "error: '{nombre}' no se pudo instalar con FLM (código {})",
                        res.status
                    ));
                    ok = false;
                }
            }
            if ok {
                let tags = ropero_runtime::flm::tags(spec).join(", ");
                ropero_core::out_line(&format!("{nombre}: instalado con FLM ({tags})"));
                0
            } else {
                1
            }
        }
        Err(e) => {
            ropero_core::err_line(&format!("error: {e}"));
            1
        }
    }
}

/// El menú de modelos: sólo los de los motores **instalados** (T-25).
fn elegir_modelos(s: &crate::discovery::Scan) -> Result<Vec<String>, i32> {
    if !ropero_runtime::install::hay_tty() {
        return Err(uso(&install::falta_tty_models()));
    }
    let motores: Vec<install::MotorDelMenu<'_>> = s
        .engines
        .iter()
        .map(|manifest| install::MotorDelMenu {
            manifest,
            instalado: ropero_runtime::engine_installed_with_env(manifest, &s.env),
        })
        .collect();
    let menu = install::menu_modelos(&s.catalog, &motores, &Palette::color());
    if menu.lineas.is_empty() {
        ropero_core::err_line(
            "aviso: no hay modelos de motores instalados (ropero install engines)",
        );
        return Err(0);
    }
    ropero_runtime::install::pregunta_menu(&menu.lineas, &menu.nombres).map_err(|e| uso(&e))
}

/// La pregunta de qué vía se usa: sin terminal es un **error de uso**.
fn preguntar_via(modelo: &str) -> Result<Via, i32> {
    if !ropero_runtime::install::hay_tty() {
        return Err(uso(&install::falta_prio(modelo)));
    }
    ropero_runtime::install::pregunta_via(&install::pregunta_de_via(modelo)).map_err(|e| uso(&e))
}

/// Un **error de uso**: el mensaje dice qué hacer y el código es 2.
fn uso(mensaje: &str) -> i32 {
    ropero_core::err_line(&ropero_core::surface::usage_message("install", mensaje));
    2
}
