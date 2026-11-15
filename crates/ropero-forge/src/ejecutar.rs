//! La **tirada**: las seis fases de la forja, de verdad (R-GGUF-15, T-30).
//!
//! ## El orden, y por qué
//!
//! 1. **precondición** (T-29): el motor, **antes** de bajar nada;
//! 2. **preflight** (R-GGUF-8): la clase de los pesos (sólo su metadata,
//!    unos KB) y la viabilidad, **antes** de la descarga;
//! 3. las **seis fases**, cada una con su **checkpoint** en el directorio
//!    de trabajo: al reanudar, se sigue por la fase siguiente a la última
//!    completada;
//! 4. el **mensaje honesto** (R-GGUF-20): si no se cuantizó nada, el
//!    informe dice el quant **real** del GGUF.
//!
//! ## Lo que no se borra
//!
//! La limpieza es una **política aparte** ([`crate::limpieza`]): aquí sólo
//! se ejecuta lo que la política decidió. La regla de oro: al fallar la
//! cuantización no se borra nada (R-GGUF-16), y los pesos sólo se borran
//! si la verificación pasó (R-GGUF-17).

use std::path::{Path, PathBuf};
use std::process::Command;

use ropero_core::Environment;

use crate::Error;
use crate::fases::{Checkpoint, Fase};
use crate::limpieza::{self, EstadoTirada, Origen};
use crate::peticion::{Fuente, Imatrix, Peticion};
use crate::plan::{self, Clase, Plan};
use crate::procedencia::{ImatrixInfo, Procedencia};
use crate::quants;
use crate::toolchain::{self, Toolchain};
use crate::verificar;

/// El nombre del fichero de **checkpoint** de la tirada, en el work.
pub const CHECKPOINT: &str = ".ropero-forge.json";

/// La herramienta que el sidecar declara, según lo que **de verdad** pasó
/// (hallazgo 6): decir «llama-quantize» cuando no corrió es mentir.
pub const HERRAMIENTA_CUANTIZA: &str = "ropero gguffer (llama.cpp llama-quantize)";
pub const HERRAMIENTA_SIN_CUANTIZAR: &str = "ropero gguffer (sin cuantizar)";
/// La salida ya estaba hecha y no se rehizo: el cuantizador **no** corrió,
/// y el sidecar no puede decir que se cuantizó (R-GGUF-20).
pub const HERRAMIENTA_YA_ESTABA: &str = "ropero gguffer (el GGUF ya estaba: no se cuantizó)";

/// Lo que la fase 3 hizo de verdad (R-GGUF-20: el gguffer no miente).
///
/// Es lo que cuenta el informe y el sidecar: **no** lo que la tirada iba a
/// hacer. Si el cuantizador no llegó a correr, aquí no hay quantizados, y
/// el informe no dice «cuantizado a …».
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cuantizacion {
    /// Los quants que el cuantizador **cuantizó** (salieron de
    /// `llama-quantize`).
    pub cuantizados: Vec<String>,
    /// Los quants que se **colocaron**: el GGUF de origen, tal cual (ya
    /// estaba en ese quant, o es un dtype plano: no hay quant que hacer).
    pub colocados: Vec<String>,
    /// Los quants cuya salida **ya existía** y no se rehizo.
    pub ya_existentes: Vec<String>,
}

impl Cuantizacion {
    /// Si el cuantizador llegó a correr.
    pub fn cuantizo(&self) -> bool {
        !self.cuantizados.is_empty()
    }
}

/// El resultado de una tirada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Informe {
    /// Los GGUF producidos (verificados, con su sidecar escrito).
    pub salidas: Vec<PathBuf>,
    /// Si se cuantizó de verdad (R-GGUF-20: si no, el mensaje lo dice).
    pub cuantizo: bool,
    /// El quant **real** del GGUF final, por nombre.
    pub quant_real: String,
    /// El quant (o los quants) que se cuantizaron, si se cuantizó.
    pub quant_cuantizado: Option<String>,
    /// Dónde quedaron los pesos, si se conservaron.
    pub pesos_conservados: Option<PathBuf>,
    /// Ya estaba: todas las salidas existían y no se hizo nada.
    pub ya_estaba: bool,
    /// La tirada **cambió** respecto al checkpoint: la petición es otra (o el
    /// checkpoint no dice de qué petición es) y se rehace desde el principio.
    pub cambio: Option<String>,
    /// Lo que hizo la fase 3: los quants cuantizados, los colocados y los
    /// que ya estaban (R-GGUF-20).
    pub cuantizacion: Cuantizacion,
}

/// Fabrica los GGUFs que pide la petición.
pub fn forja(peticion: &Peticion, env: &Environment) -> Result<Informe, Error> {
    // Una fuente que ya es un GGUF no pasa por el conversor: el conversor
    // sólo hace falta si hay que convertir (R-GGUF-55), y un motor con sólo
    // el cuantizador puede fabricar.
    let es_entrada = matches!(peticion.fuente, Fuente::Gguf(_));

    // ── la precondición, ANTES de bajar nada (T-29) ────────────────────
    let toolchain = toolchain::buscar(env, !es_entrada)?;

    // ── el preflight: clase y viabilidad, ANTES de la descarga (R-GGUF-8)
    let clase = clase_de_fuente(peticion, env)?;
    let plan = Plan::de(peticion, env, clase)?;
    let viab = plan::viabilidad(clase, peticion.intermedio, &peticion.quants)?;
    if viab.cuantizar && peticion.imatrix.is_none() {
        return Err(Error::FaltaImatrix {
            faltan: viab.faltan.clone(),
        });
    }

    // ── el checkpoint: por dónde reanudar, y **de qué petición** ───────
    // El checkpoint sólo vale si es de esta tirada: guardaba sólo la fase, así
    // que una petición distinta sobre la misma fuente reanudaba con el
    // checkpoint de la anterior y anunciaba un GGUF que no estaba hecho
    // (T-30, R-GGUF-20). Con `--force` se descarta siempre: force es pedir
    // que se rehaga.
    let fichero_cp = plan.work.join(CHECKPOINT);
    let huella = peticion.huella();
    let guardado = Checkpoint::de_json(&lee(&fichero_cp));
    let cambio = if peticion.force {
        Some("la petición es con --force: el checkpoint se descarta y se rehace".to_string())
    } else if guardado.as_ref().is_some_and(|cp| !cp.es_de(&huella)) {
        Some(format!(
            "la petición cambió ({}): el checkpoint es de otra tirada, se rehace desde el principio",
            peticion.canon()
        ))
    } else {
        None
    };
    let checkpoint = if peticion.force {
        None
    } else {
        guardado.filter(|cp| cp.es_de(&huella))
    };

    // ── si ya existe todo, no se hace nada (ni se baja) ────────────────
    if !peticion.force && !peticion.diagnostico && ya_estan_todas(&plan) {
        return Ok(Informe {
            salidas: plan.salidas.iter().map(|s| s.ruta.clone()).collect(),
            cuantizo: false,
            quant_real: clase.nombre_quant(peticion.intermedio),
            quant_cuantizado: None,
            pesos_conservados: Some(plan.work.clone()),
            ya_estaba: true,
            cambio,
            cuantizacion: Cuantizacion::default(),
        });
    }

    let mut siguiente = match &checkpoint {
        Some(cp) => cp.fase.siguiente(),
        None => Some(Fase::Fuente),
    };

    // Lo que la cuantización hizo de verdad: en esta tirada, o en una tirada
    // anterior de esta misma (el checkpoint lo guarda). Antes esto se fijaba a
    // `viab.cuantizar` —lo que la tirada iba a hacer— y el informe decía
    // «cuantizado a Q4_K_M» sin que el cuantizador llegara a correr (R-GGUF-20:
    // no mentir sobre el resultado).
    let mut cuantizacion = Cuantizacion::default();
    if let Some(cp) = checkpoint.as_ref()
        && cp.fase.orden() >= Fase::Cuantizacion.orden()
    {
        cuantizacion.cuantizados = cp.cuantizados.clone();
    }

    // ── FASE 1: la fuente ──────────────────────────────────────────────
    if siguiente.is_some_and(|f| f == Fase::Fuente) {
        fase_fuente(peticion, env, &plan)?;
        escribe_cp(
            &fichero_cp,
            Fase::Fuente,
            &huella,
            &cuantizacion.cuantizados,
        )?;
    }
    siguiente = siguiente_y_cp(siguiente, Fase::Fuente);

    // ── FASE 2: la conversión ──────────────────────────────────────────
    let es_entrada = matches!(peticion.fuente, Fuente::Gguf(_));
    // ¿El intermedio es **alguna** de las salidas? (R-GGUF-18: si el F16
    // se pidió **como salida**, es un GGUF final y no se toca.) Mirar
    // sólo la primera salía mal: con `--quant Q4_K_M --quant F16` la
    // ruta del F16 coincide con la del intermedio, y la fase 5 lo
    // borraba: se destruía un GGUF recién producido. La comprobación va
    // **antes** de la fase 5, que es la que borra.
    let intermedio_es_resultado = plan.salidas.iter().any(|s| s.ruta == plan.intermedio);
    if siguiente.is_some_and(|f| f == Fase::Conversion) {
        if !es_entrada {
            fase_conversion(peticion, env, &toolchain, &plan, clase, &viab)?;
        }
        // La fuente `--gguf` no pasa por el conversor, pero la fase está
        // hecha igual: sin este checkpoint, el fichero se queda atrás
        // («fuente») y la reanudación no sabe por dónde seguir.
        escribe_cp(
            &fichero_cp,
            Fase::Conversion,
            &huella,
            &cuantizacion.cuantizados,
        )?;
    }
    siguiente = siguiente_y_cp(siguiente, Fase::Conversion);

    // ── FASE 3: la cuantización (log aparte, R-GGUF-15.3) ──────────────
    // De donde salen los pesos que se cuantizan: el GGUF que trajo el
    // usuario (un `--gguf` no pasa por la conversión), o el intermedio.
    let entrada: Option<PathBuf> = match &peticion.fuente {
        Fuente::Gguf(gguf) if Path::new(gguf).is_file() => Some(PathBuf::from(gguf)),
        Fuente::Gguf(gguf) => {
            // En el work el fichero conserva el nombre con el que está en el
            // repo, con su extensión: `--gguf acme/modelo-IQ2_M` (un repo, no
            // un fichero) deja `modelo-IQ2_M.gguf`. Sin la extensión, `origen`
            // apuntaba a un nombre sin `.gguf` y la colocación no encontraba
            // el fichero.
            let hoja = gguf.rsplit('/').next().unwrap_or(gguf);
            Some(plan.work.join(if hoja.ends_with(".gguf") {
                hoja.to_string()
            } else {
                format!("{hoja}.gguf")
            }))
        }
        _ => None,
    };
    let origen_quant = entrada.unwrap_or_else(|| plan.intermedio.clone());
    if siguiente.is_some_and(|f| f == Fase::Cuantizacion) {
        if let Some(imx) = peticion.imatrix.as_ref()
            && viab.cuantizar
        {
            baja_imatrix(imx, env, &plan)?;
        }
        cuantizacion = fase_cuantizacion(peticion, &toolchain, &plan, clase, &viab, &origen_quant)?;
        escribe_cp(
            &fichero_cp,
            Fase::Cuantizacion,
            &huella,
            &cuantizacion.cuantizados,
        )?;
    }
    siguiente = siguiente_y_cp(siguiente, Fase::Cuantizacion);
    let cuantizo = cuantizacion.cuantizo();

    // ── FASE 4: la verificación ────────────────────────────────────────
    if siguiente.is_some_and(|f| f == Fase::Verificacion) {
        for salida in &plan.salidas {
            if let Some(motivo) = verificar::verificar(&salida.ruta).err() {
                return Err(Error::Verificacion {
                    gguf: salida.ruta.to_string_lossy().into_owned(),
                    motivo,
                    pesos: format!(
                        "NO borro los pesos: están en {}",
                        plan.work.to_string_lossy()
                    ),
                });
            }
        }
        escribe_cp(
            &fichero_cp,
            Fase::Verificacion,
            &huella,
            &cuantizacion.cuantizados,
        )?;
    }
    siguiente = siguiente_y_cp(siguiente, Fase::Verificacion);

    // ── FASE 5: la limpieza (la política decide, aquí se ejecuta) ──────
    // Lo que decide la política es si el intermedio **es** un intermedio: si
    // la tirada cuantiza, lo es (y se borra después de todos los quants); si
    // no, el intermedio **es** el GGUF final y no se toca (R-GGUF-18). Eso
    // lo dice el **plan** (lo que la tirada cuantiza), no lo que el
    // cuantizador llegó a hacer en esta tirada: si la salida ya estaba y no
    // se rehízo, el intermedio sigue siendo un sobrante y se borra igual.
    let estado = EstadoTirada {
        origen: origen_de(peticion),
        verifico: true,
        cuantizo: viab.cuantizar,
        intermedio_es_resultado,
        intermedio_es_entrada: es_entrada,
        keep_source: peticion.keep_source,
        keep_intermedio: peticion.keep_intermedio,
    };
    let ordenes = limpieza::ordenes(&estado);
    if siguiente.is_some_and(|f| f == Fase::Limpieza) {
        // El checkpoint se escribe ANTES de los borrados: la limpieza
        // puede borrar el work con el propio checkpoint dentro (R-GGUF-16
        // y R-GGUF-17), y entonces ya no hay nada que reanudar.
        escribe_cp(
            &fichero_cp,
            Fase::Limpieza,
            &huella,
            &cuantizacion.cuantizados,
        )?;
        if ordenes.borrar_clon {
            borra_dir(&plan.work, "borrar el clon de los pesos")?;
        }
        if ordenes.borrar_intermedio {
            // La política ya tiene en cuenta que el intermedio sea una
            // salida (R-GGUF-18): aquí sólo se ejecuta lo que se decidió.
            borra_fichero(&plan.intermedio, "borrar el GGUF intermedio")?;
        }
    }
    siguiente = siguiente_y_cp(siguiente, Fase::Limpieza);

    // ── FASE 6: la procedencia (R-GGUF-19: si no se puede, falla) ──────
    if siguiente.is_some_and(|f| f == Fase::Procedencia) {
        for salida in &plan.salidas {
            escribe_procedencia(salida, peticion, clase, es_entrada, &cuantizacion)?;
        }
        // El checkpoint final sólo si el work sigue ahí: la limpieza
        // pudo borrarlo con el checkpoint dentro, y entonces no hay nada
        // que reanudar — un reintento con los GGUF ya hechos cae en «ya
        // estaba» (T-30).
        if plan.work.is_dir() {
            escribe_cp(
                &fichero_cp,
                Fase::Procedencia,
                &huella,
                &cuantizacion.cuantizados,
            )?;
        }
    }

    // El informe no puede enseñar un GGUF que no está (T-30, R-GGUF-20).
    // Un checkpoint de la última fase **sin la petición de la que es** no vale
    // (se descartó arriba), pero un checkpoint de la MISMA petición cuya salida
    // se borró después sí dejaba la tirada en 0 anunciando un fichero inexistente:
    // la última fase no se ejecuta, el sidecar no se escribe y nadie lo comprueba.
    let faltantes: Vec<String> = plan
        .salidas
        .iter()
        .filter(|s| !s.ruta.is_file())
        .map(|s| s.ruta.to_string_lossy().into_owned())
        .collect();
    if !faltantes.is_empty() {
        return Err(Error::SalidaFaltante {
            ggufos: faltantes,
            motivo: format!(
                "el checkpoint decía «{}» (de la petición {}) pero esos GGUFs no están en disco",
                checkpoint
                    .as_ref()
                    .map(|cp| cp.fase.nombre())
                    .unwrap_or("ninguna fase"),
                huella
            ),
        });
    }

    let pesos_conservados = if ordenes.borrar_clon {
        None
    } else {
        Some(plan.work.clone())
    };
    Ok(Informe {
        salidas: plan.salidas.iter().map(|s| s.ruta.clone()).collect(),
        cuantizo,
        quant_real: clase.nombre_quant(peticion.intermedio),
        quant_cuantizado: quant_cuantizado(&cuantizacion),
        pesos_conservados,
        ya_estaba: false,
        cambio,
        cuantizacion,
    })
}

/// El quant (o los quants) que se cuantizaron de verdad.
///
/// Son los que el cuantizador **cuantizó**: los planos (F16, BF16) son
/// conversión de dtype, no cuantización, y los que ya estaban en su quant
/// no se tocan.
fn quant_cuantizado(cuantizacion: &Cuantizacion) -> Option<String> {
    if cuantizacion.cuantizados.is_empty() {
        return None;
    }
    Some(cuantizacion.cuantizados.join(", "))
}

/// El informe de una tirada, en líneas (R-GGUF-20: el mensaje honesto).
///
/// Es el dominio quien compone las líneas: la CLI las imprime y ya
/// (R-ARQ-1).
pub fn lineas(informe: &Informe) -> Vec<String> {
    let mut lineas = Vec::new();
    for salida in &informe.salidas {
        lineas.push(format!("{}", salida.to_string_lossy()));
    }
    if let Some(cambio) = &informe.cambio {
        // Lo que cambió se dice: una tirada que se rehizo no puede parecer una
        // reanudación (T-30).
        lineas.push(cambio.clone());
    }
    if informe.cuantizo {
        // La verdad: se cuantizó, y a qué quant (R-GGUF-20).
        if let Some(quant) = &informe.quant_cuantizado {
            lineas.push(format!("cuantizado a {quant}"));
        }
    } else if !informe.ya_estaba {
        // Lo que **no** se cuantizó se dice como fue (R-GGUF-20): el mensaje
        // viejo («el quant pedido se perdió en la conversión») era mentira
        // cuando no hubo conversión (la colocación de un `--gguf`).
        if !informe.cuantizacion.colocados.is_empty() {
            lineas.push(format!(
                "no se cuantizó: {} ya estaba en ese quant y se colocó tal cual",
                informe.cuantizacion.colocados.join(", ")
            ));
        } else if !informe.cuantizacion.ya_existentes.is_empty() {
            lineas.push(format!(
                "no se cuantizó: {} ya estaba hecho y no se rehizo",
                informe.cuantizacion.ya_existentes.join(", ")
            ));
        } else {
            lineas.push(format!(
                "no se cuantizó: no hay quant que hacer (el GGUF está en {})",
                informe.quant_real
            ));
        }
    }
    if informe.ya_estaba {
        lineas.push("ya estaba: no se ha hecho nada (usa --force para rehacer)".to_string());
    }
    if let Some(pesos) = &informe.pesos_conservados {
        lineas.push(format!("pesos conservados en: {}", pesos.to_string_lossy()));
    }
    lineas
}

/// La fase siguiente, si la que venía ya no es la que se acaba de hacer.
fn siguiente_y_cp(siguiente: Option<Fase>, hecha: Fase) -> Option<Fase> {
    match siguiente {
        Some(f) if f == hecha => f.siguiente(),
        Some(f) => Some(f),
        None => None,
    }
}

/// La clase de los pesos, leída de la metadata **antes** de descargar.
///
/// Para un `--repo` se baja **sólo** `config.json` (un par de KB) si no
/// está ya en el clon: es lo que permite fallar sin bajar 60 GB
/// (R-GGUF-8). Para un `--path` se lee del directorio. Para un `--gguf`
/// se deduce del nombre del fichero (`modelo-NVFP4.gguf`), que es la
/// convención de los repos de cuantizaciones.
fn clase_de_fuente(peticion: &Peticion, env: &Environment) -> Result<Clase, Error> {
    match &peticion.fuente {
        Fuente::Repo(repo) => {
            let work_root = env.env_path("GGUF_WORK_DIR", ".cache/ropero/gguffer");
            let work = work_root.join(repo.replace('/', "__"));
            let mut clase = Clase::Ninguno;
            for fichero in ["config.json", "hf_quant_config.json"] {
                let ruta = work.join(fichero);
                if !ruta.is_file() {
                    // La metadata puede no estar (los repos GGUF de
                    // fábrica no la llevan): si no baja, se sigue con la
                    // clase vacía — unos KB, no los pesos (R-GGUF-8).
                    let _ = baja_fichero(env, repo, fichero, &work);
                }
                if ruta.is_file() {
                    clase = clase.de(&lee(&ruta), &lee(&work.join("hf_quant_config.json")));
                }
            }
            Ok(clase)
        }
        Fuente::Path(path) => Ok(Clase::Ninguno.de(
            &lee(&path.join("config.json")),
            &lee(&path.join("hf_quant_config.json")),
        )),
        Fuente::Gguf(gguf) => Ok(clase_de_gguf(gguf)),
    }
}

/// La clase de un GGUF: primero su **cabecera**, y el nombre sólo como respaldo.
///
/// `general.file_type` es lo que el fichero *es* (R-GGUF-7): `modelo-BF16.gguf`
/// puede contener un IQ2_M, y el nombre es la convención de los repos, no el
/// contenido. Si la cabecera no se puede leer (no existe, o no es un GGUF), se
/// cae al nombre con su separador.
fn clase_de_gguf(gguf: &str) -> Clase {
    let ruta = Path::new(gguf);
    if let Some(numero) = verificar::file_type(ruta) {
        return Clase::de_numero(numero);
    }
    let base = gguf.trim_end_matches(".gguf");
    let hoja = base.rsplit(['/', '-']).next().unwrap_or(base);
    match quants::numero_de(hoja) {
        Some(numero) => Clase::de_numero(numero),
        None => Clase::Ninguno,
    }
}

impl Clase {
    /// La clase, según la metadata de `config.json` y
    /// `hf_quant_config.json`.
    ///
    /// Es el clasificador de la v1 (`_gguffer_quant_fuente`), reducido a
    /// lo que el motor distingue: lo que conserva (NVFP4, MXFP4), lo que
    /// dequantiza (FP8, AWQ, GPTQ) y lo que viene en claro.
    pub fn de(self, config: &str, hqc: &str) -> Clase {
        let metodo = cadena(config, "quant_method")
            .or_else(|| cadena(hqc, "quant_method"))
            .or_else(|| cadena(hqc, "name"))
            .unwrap_or_default();
        let formato = cadena(config, "format")
            .or_else(|| cadena(hqc, "format"))
            .unwrap_or_default();
        let algo = cadena(config, "quant_algo")
            .or_else(|| cadena(hqc, "quant_algo"))
            .unwrap_or_default();
        if formato.contains("nvfp4") || algo.to_ascii_uppercase().contains("NVFP4") {
            return Clase::Nvfp4;
        }
        match metodo.as_str() {
            "mxfp4" => Clase::Mxfp4,
            "modelopt" | "compressed-tensors" => {
                if formato.is_empty() {
                    Clase::Dequant
                } else {
                    // float-quantized (FP8), int-quantized, mxfp4-pack…
                    // y las mezclas que el motor no sabe conservar.
                    Clase::Dequant
                }
            }
            "awq" | "gptq" | "bnb" | "bitsandbytes" | "fp8" => Clase::Dequant,
            _ => Clase::Ninguno,
        }
    }
}

/// El valor **string** de un campo de un JSON plano, buscado a mano.
///
/// Un parser completo (con arrays y anidado) es tarea de una
/// dependencia que la forja no quiere; esto encuentra `"campo": "valor"`
/// y `"campo": {…}` de un nivel, que es lo que traen `config.json` y
/// `hf_quant_config.json` de los quants de fábrica.
fn cadena(json: &str, campo: &str) -> Option<String> {
    let clave = format!("\"{campo}\"");
    let desde = json.find(&clave)? + clave.len();
    let resto = &json[desde..];
    let dos_puntos = resto.find(':')?;
    let despues = &resto[dos_puntos + 1..];
    let valor = despues.trim_start();
    let valor = valor.strip_prefix('"')?;
    let fin = valor.find('"')?;
    Some(valor[..fin].to_string())
}

/// La fase 1: la fuente, en sus tres formas.
fn fase_fuente(peticion: &Peticion, env: &Environment, plan: &Plan) -> Result<(), Error> {
    match &peticion.fuente {
        Fuente::Repo(repo) => {
            if plan.sentinel.is_file() {
                return Ok(());
            }
            crea_dir(&plan.work)?;
            baja_todo(env, repo, &plan.work)?;
            escribe_sentinel(&plan.sentinel)?;
            Ok(())
        }
        Fuente::Path(path) => {
            if !path.is_dir() {
                return Err(Error::FuenteNoEsta {
                    fuente: peticion.fuente.describe(),
                    ruta: path.to_string_lossy().into_owned(),
                });
            }
            Ok(())
        }
        Fuente::Gguf(gguf) => {
            if Path::new(gguf).is_file() {
                return Ok(());
            }
            if gguf.contains('/') {
                let work = &plan.work;
                crea_dir(work)?;
                baja_todo(env, gguf, work)?;
                Ok(())
            } else {
                Err(Error::FuenteNoEsta {
                    fuente: peticion.fuente.describe(),
                    ruta: gguf.to_string(),
                })
            }
        }
    }
}

/// La fase 2: la conversión a GGUF.
fn fase_conversion(
    peticion: &Peticion,
    env: &Environment,
    toolchain: &Toolchain,
    plan: &Plan,
    clase: Clase,
    viab: &plan::Viabilidad,
) -> Result<(), Error> {
    // Si va a haber cuantización, la conversión produce el **intermedio**
    // (de ahí salen los quants). Si no —colocación o conversión plana—,
    // produce directamente el GGUF final.
    let destino = if viab.cuantizar || plan.salidas.len() > 1 {
        plan.intermedio.clone()
    } else {
        plan.salidas[0].ruta.clone()
    };
    if destino.is_file() {
        return Ok(());
    }
    // Sin conversor no hay conversión (R-GGUF-55): `convert` es `None` cuando
    // la fuente ya es un GGUF y sólo hay que cuantizar. Llegar aquí con `None`
    // significa que la tirada pidió convertir con un motor que sólo tiene el
    // cuantizador: se dice, y no se finge un fichero que no existe.
    let convert =
        match toolchain.convert.as_ref() {
            Some(convert) => convert,
            None => return Err(Error::MotorIncompleto {
                falta: "convert_hf_to_gguf.py".to_string(),
                donde:
                    "el motor encontrado no tiene conversor: sin él no hay conversión (R-GGUF-55)"
                        .to_string(),
            }),
        };
    crea_dir(destino.parent().unwrap_or(Path::new(".")))?;
    let mut args = vec![
        plan.source_dir.to_string_lossy().into_owned(),
        "--outfile".to_string(),
        destino.to_string_lossy().into_owned(),
        "--outtype".to_string(),
        peticion.intermedio.outtype().to_string(),
    ];
    // `--no-mtp` sólo si el conversor lo entiende: un flag que no existe
    // hace fallar el conversor con "unrecognized arguments" (R-GGUF-36).
    if peticion.no_mtp && !el_conversor_entiende(convert, "no-mtp") {
        // Se avisa y se sigue sin el: el conversor viejo no lo tiene.
        crate::avisa("este conversor no sabe excluir la MTP (--no-mtp); sigo sin el");
    } else if peticion.no_mtp {
        args.push("--no-mtp".to_string());
    }
    // `--no-dflash` igual que `--no-mtp`: se pasa si el conversor lo
    // entiende, y si no, **se dice** (no se acepta en silencio: el
    // usuario tiene que saber que el GGUF lleva las cabeceras DFlash).
    if peticion.no_dflash && !el_conversor_entiende(convert, "no-dflash") {
        crate::avisa(
            "este conversor no sabe excluir las cabeceras DFlash (--no-dflash); sigo con ellas",
        );
    } else if peticion.no_dflash {
        args.push("--no-dflash".to_string());
    }
    let _ = clase;
    let log = plan.work.join("conversion.log");
    let linea = format!("python3 {} {}", convert.to_string_lossy(), args.join(" "));
    let mut argv = vec![convert.to_string_lossy().into_owned()];
    argv.extend(args);
    ejecuta(&python3(env)?, &argv, &log).map_err(|cola| Error::Conversion {
        cola,
        reintento: linea.clone(),
    })?;
    Ok(())
}

/// Si el conversor entiende un flag (lo busca en su texto).
fn el_conversor_entiende(convert: &Path, flag: &str) -> bool {
    lee(convert).contains(flag)
}

/// La fase 3: la cuantización, quant a quant.
///
/// Devuelve **lo que hizo de verdad**: los quants que el cuantizador
/// cuantizó, los que se colocaron (copiados) y los que ya estaban hechos
/// (R-GGUF-20: el informe y el sidecar cuentan esto, no lo que la tirada iba
/// a hacer).
fn fase_cuantizacion(
    peticion: &Peticion,
    toolchain: &Toolchain,
    plan: &Plan,
    clase: Clase,
    viab: &plan::Viabilidad,
    origen: &Path,
) -> Result<Cuantizacion, Error> {
    let mut hecho = Cuantizacion::default();
    if !viab.cuantizar {
        // Los quants que ya están en el GGUF (omitidos) y los que son planos
        // (F32/F16/BF16: no hay quant que hacer) se **colocan**: el GGUF de
        // origen se copia a su ruta (R-GGUF-7: se sale 0 sin borrar nada).
        for salida in &plan.salidas {
            if (viab.omitidos.contains(&salida.quant) || viab.planos.contains(&salida.quant))
                && salida.ruta != origen
                && origen.is_file()
            {
                // El directorio de salida puede no existir (con una fuente
                // `--gguf` no hay conversión que lo cree): sin esto, la
                // colocación fallaba con «no such file or directory».
                crea_dir(salida.ruta.parent().unwrap_or(Path::new(".")))?;
                borra_fichero(&salida.ruta, "quitar el GGUF que se rehace")?;
                copia(
                    origen,
                    &salida.ruta,
                    "colocar el GGUF (ya está en ese quant)",
                )?;
                hecho.colocados.push(salida.quant.clone());
            }
        }
        return Ok(hecho);
    }
    let imx = peticion.imatrix.as_ref().map(|imx| ruta_imatrix(imx, plan));
    let imx = match imx {
        Some(Ok(ruta)) => Some(ruta),
        Some(Err(error)) => return Err(error),
        None => None,
    };
    let log = plan.work.join("cuantizacion.log");
    for salida in &plan.salidas {
        if viab.omitidos.contains(&salida.quant) {
            continue;
        }
        if salida.ruta.is_file() {
            // El que ya existe de una tirada anterior: no se rehace, y no se
            // dice que se cuantizó (el cuantizador no corrió).
            hecho.ya_existentes.push(salida.quant.clone());
            continue;
        }
        crea_dir(salida.ruta.parent().unwrap_or(Path::new(".")))?;
        let mut args: Vec<String> = Vec::new();
        if let Some(ruta) = imx.as_ref() {
            // La importance matrix va **delante** de los posicionales
            // (R-GGUF-39: `llama-quantize` sólo parsea opciones mientras
            // el argumento empiece por `--`).
            args.push("--imatrix".to_string());
            args.push(ruta.to_string_lossy().into_owned());
        }
        args.push(origen.to_string_lossy().into_owned());
        args.push(salida.ruta.to_string_lossy().into_owned());
        args.push(salida.quant.clone());
        let linea = format!(
            "{} {}",
            toolchain.quantize.to_string_lossy(),
            args.join(" ")
        );
        if let Err(cola_error) = ejecuta(&toolchain.quantize, &args, &log) {
            // R-GGUF-16: NO se borra nada: el clon y el GGUF intermedio
            // se quedan, para poder reintentar **sólo** la cuantización
            // (que es la parte cara).
            //
            // Lo único que se borra es el GGUF **a medias** que dejó el
            // cuantizador (está corrupto): si se quedara, la siguiente
            // tirada creería que la salida está hecha (`ya_estan_todas`)
            // y no la reharía. Y nunca es el intermedio: el intermedio es
            // justamente lo que hace falta para reintentar.
            if salida.ruta != plan.intermedio {
                let _ = std::fs::remove_file(&salida.ruta);
            }
            return Err(Error::Cuantizacion {
                quant: salida.quant.clone(),
                cola: cola_error.clone(),
                reintento: linea,
                traduccion: traduce_imatrix(&cola_error),
            });
        }
        let _ = clase;
        // El cuantizador **sí** corrió: el quant se cuenta como cuantizado
        // (y el informe lo dice).
        hecho.cuantizados.push(salida.quant.clone());
    }
    Ok(hecho)
}

/// La ruta del imatrix que se le pasa al cuantizador.
///
/// Un imatrix **local**: el fichero, tal cual (lo trajo el usuario). Uno
/// **remoto**: el fichero de la **caché** (el que se acaba de bajar), que
/// es lo único que el cuantizador puede leer. Si la descarga no lo dejó
/// donde debía, es un error: cuantizar sin imatrix es cuantizar a ciegas,
/// y el GGUF sale peor sin que nadie lo diga.
fn ruta_imatrix(imx: &Imatrix, plan: &Plan) -> Result<PathBuf, Error> {
    match imx {
        // Un imatrix local también se comprueba: antes esto sólo comprobaba el
        // remoto, y se cuantizaba a ciegas con un fichero que no existía.
        Imatrix::Local(fichero) => {
            if !fichero.is_file() {
                return Err(Error::ImatrixNoEsta {
                    origen: "el fichero local".to_string(),
                    fichero: fichero
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    ruta: fichero.to_string_lossy().into_owned(),
                });
            }
            Ok(fichero.clone())
        }
        Imatrix::Remota { repo, file } => {
            let ruta = cache_imatrix(plan, repo, file);
            if !ruta.is_file() {
                return Err(Error::ImatrixNoEsta {
                    origen: repo.clone(),
                    fichero: file.clone(),
                    ruta: ruta.to_string_lossy().into_owned(),
                });
            }
            Ok(ruta)
        }
    }
}

/// La ruta del imatrix remoto en la caché: `<work>/imatrix/<repo>/<fichero>`.
///
/// El repo va con las barras cambiadas por dos guiones bajos (el mismo
/// truco que el work: un directorio por repo, sin.makedirs raros).
fn cache_imatrix(plan: &Plan, repo: &str, fichero: &str) -> PathBuf {
    plan.work
        .join("imatrix")
        .join(repo.replace('/', "__"))
        .join(fichero)
}

/// **R-GGUF-14**: el error de imatrix del cuantizador, **traducido**.
///
/// «Missing importance matrix for tensor blk.64.…» significa: es el
/// bloque 64, el de la cabeza MTP; el imatrix se calcula sobre el modelo
/// base, que no la tiene. Las dos salidas: dejar ese bloque en Q4_0 con
/// `--tensor-type`, o convertir sin MTP con `--no-mtp`.
fn traduce_imatrix(cola: &[String]) -> Option<String> {
    let bloque = cola.iter().find_map(|linea| {
        let desde = linea.find("tensor blk.")?;
        let resto = &linea[desde + "tensor blk.".len()..];
        let fin = resto.find(|c: char| !c.is_ascii_digit())?;
        resto[..fin].parse::<u32>().ok()
    })?;
    Some(format!(
        "es el bloque {bloque}, el de la cabeza MTP: el imatrix se calcula sobre el \
         modelo base, que no la tiene.\n  \
         opciones: dejar ese bloque en Q4_0 con --tensor-type, \
         o convertir sin MTP con --no-mtp"
    ))
}

/// La fase 6: el sidecar de procedencia de cada salida.
fn escribe_procedencia(
    salida: &plan::Salida,
    peticion: &Peticion,
    clase: Clase,
    es_entrada: bool,
    cuantizacion: &Cuantizacion,
) -> Result<(), Error> {
    let imatrix = peticion.imatrix.as_ref().map(|imx| match imx {
        Imatrix::Local(fichero) => ImatrixInfo {
            fichero: fichero
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            repo: None,
        },
        Imatrix::Remota { repo, file } => ImatrixInfo {
            fichero: file.clone(),
            repo: Some(repo.clone()),
        },
    });
    let procedencia = Procedencia {
        gguf: salida
            .ruta
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        quant: quants::nombre_de(salida.numero)
            .unwrap_or(salida.quant.as_str())
            .to_string(),
        fuente_repo: fuente_repo(peticion),
        // El sidecar no afirma lo que no pasó (hallazgo 6, R-GGUF-20): si la
        // fuente ya era un GGUF no hubo intermedio, y si el cuantizador no
        // corrió no se dice que se cuantizó.
        intermedio: if es_entrada {
            None
        } else {
            Some(clase.sufijo_intermedio(peticion.intermedio).to_string())
        },
        herramienta: if cuantizacion.cuantizados.contains(&salida.quant) {
            HERRAMIENTA_CUANTIZA.to_string()
        } else if cuantizacion.colocados.contains(&salida.quant) {
            HERRAMIENTA_SIN_CUANTIZAR.to_string()
        } else {
            HERRAMIENTA_YA_ESTABA.to_string()
        },
        imatrix,
        mtp: None,
        fecha: fecha_utc(),
    };
    let dest = Procedencia::ruta(&salida.ruta);
    escribe(&dest, &procedencia.a_json()).map_err(|detalle| Error::Procedencia {
        dest: dest.to_string_lossy().into_owned(),
        detalle,
    })
}

/// Cómo se escribió la fuente, para el sidecar.
fn fuente_repo(peticion: &Peticion) -> String {
    match &peticion.fuente {
        Fuente::Repo(repo) => repo.clone(),
        Fuente::Path(path) => path.to_string_lossy().into_owned(),
        Fuente::Gguf(gguf) => gguf.clone(),
    }
}

/// La fecha de ahora, en UTC e ISO (la que va al sidecar).
fn fecha_utc() -> String {
    let ahora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Días desde 1970-01-01 → Y-M-D (el calendario gregoriano, a mano).
    let dias = ahora / 86_400;
    let resto = ahora % 86_400;
    let (hora, minuto, segundo) = (resto / 3600, (resto % 3600) / 60, resto % 60);
    let mut año: i64 = 1970;
    let mut quedan = dias as i64;
    loop {
        let bisiesto = (año % 4 == 0 && año % 100 != 0) || año % 400 == 0;
        let largo = if bisiesto { 366 } else { 365 };
        if quedan < largo {
            break;
        }
        quedan -= largo;
        año += 1;
    }
    let bisiesto = (año % 4 == 0 && año % 100 != 0) || año % 400 == 0;
    let largos = [
        31,
        if bisiesto { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut mes = 1;
    for largo in largos {
        if quedan < largo {
            break;
        }
        quedan -= largo;
        mes += 1;
    }
    format!("{año:04}-{mes:02}-{quedan:02}T{hora:02}:{minuto:02}:{segundo:02}Z")
}

/// Baja un repo entero con `hf`.
fn baja_todo(env: &Environment, repo: &str, dest: &Path) -> Result<(), Error> {
    let hf = hf_de(env)?;
    baja_con(env, &hf, repo, None, dest).map_err(|codigo| Error::Descarga {
        repo: repo.to_string(),
        codigo,
    })
}

/// Baja un solo fichero de un repo (la metadata, o el imatrix).
fn baja_fichero(env: &Environment, repo: &str, fichero: &str, dest: &Path) -> Result<(), Error> {
    let hf = hf_de(env)?;
    baja_con(env, &hf, repo, Some(fichero), dest).map_err(|codigo| Error::Descarga {
        repo: repo.to_string(),
        codigo,
    })
}

/// El `hf` del entorno (el `PATH`), o el error que lo nombra.
fn hf_de(env: &Environment) -> Result<PathBuf, Error> {
    let path = env.get("PATH").ok_or_else(|| Error::Disco {
        que: "buscar 'hf'".to_string(),
        detalle: "no hay PATH en el entorno".to_string(),
    })?;
    std::env::split_paths(path)
        .map(|dir| dir.join("hf"))
        .find(|candidato| toolchain::es_ejecutable(candidato))
        .ok_or_else(|| Error::Disco {
            que: "buscar 'hf'".to_string(),
            detalle: "no está 'hf' en el PATH (instálalo con: uv tool install \
                      'huggingface_hub[cli]')"
                .to_string(),
        })
}

/// Baja con `hf download`, que es reanudable con `--local-dir` fijo.
fn baja_con(
    env: &Environment,
    hf: &Path,
    repo: &str,
    fichero: Option<&str>,
    dest: &Path,
) -> Result<(), Option<i32>> {
    let mut args = vec![
        "download".to_string(),
        repo.to_string(),
        "--local-dir".to_string(),
        dest.to_string_lossy().into_owned(),
    ];
    if let Some(fichero) = fichero {
        args.push(fichero.to_string());
    }
    let estado = con_reintento(|| Command::new(hf).args(&args).status()).map_err(|_| None)?;
    if estado.success() {
        Ok(())
    } else {
        // La descarga del clon **completo** a medias no sirve: se limpia
        // el clon parcial y la cache de HF. La de un fichero suelto (la
        // metadata) no: borrar el `work` mataría el clon que ya vive
        // ahí.
        if fichero.is_none() {
            let _ = std::fs::remove_dir_all(dest);
            if let Some(home) = env.get("HF_HOME") {
                let cache = std::path::PathBuf::from(home)
                    .join("hub")
                    .join(format!("models--{}", repo.replace('/', "--")));
                let _ = std::fs::remove_dir_all(cache);
            }
        }
        Err(estado.code())
    }
}

/// Baja el imatrix remoto al cache (que va por repo de imatrix).
fn baja_imatrix(imx: &Imatrix, env: &Environment, plan: &Plan) -> Result<(), Error> {
    let (repo, file) = match imx {
        Imatrix::Local(_) => return Ok(()),
        Imatrix::Remota { repo, file } => (repo.clone(), file.clone()),
    };
    let destino = cache_imatrix(plan, &repo, &file);
    if destino.is_file() {
        return Ok(());
    }
    let cache = destino
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| plan.work.join("imatrix"));
    crea_dir(&cache)?;
    baja_fichero(env, &repo, &file, &cache)
}

/// Ejecuta un comando capturando su salida en un **log aparte**.
///
/// Devuelve la **cola** del log (las últimas líneas) si falló: es lo que
/// el error enseña, porque el último error del conversor o del
/// cuantizador dice por qué.
fn ejecuta(programa: &Path, args: &[String], log: &Path) -> Result<(), Vec<String>> {
    let salida =
        con_reintento(|| Command::new(programa).args(args).output()).map_err(|_| Vec::new())?;
    let texto = format!(
        "{}{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
    let _ = std::fs::write(log, &texto);
    if salida.status.success() {
        return Ok(());
    }
    Err(cola(&texto, 20))
}

/// Los intentos cuando el **exec** falla con ETXTBSY.
const INTENTOS_EXEC: usize = 10;

/// La pausa base entre intentos (crece con cada uno).
const PAUSA_EXEC: std::time::Duration = std::time::Duration::from_millis(20);

/// Ejecuta algo, reintentando si falla con **ETXTBSY**.
///
/// «Text file busy» pasa cuando el fichero que se ejecuta lo tiene otro
/// proceso abierto para escribir. Es una **carrera** de milisegundos (un
/// `fork` en paralelo hereda el descriptor abierto), no un error del
/// comando: lo correcto es reintentar. Sin esto, una tirada puede morir
/// porque otro proceso escribía un script en ese instante.
fn con_reintento<T>(mut accion: impl FnMut() -> std::io::Result<T>) -> std::io::Result<T> {
    let mut intento = 0;
    loop {
        let resultado = accion();
        let ocupado = matches!(
            &resultado,
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy
        );
        if !ocupado || intento >= INTENTOS_EXEC {
            return resultado;
        }
        intento += 1;
        std::thread::sleep(PAUSA_EXEC * intento as u32);
    }
}

/// El `python3` del `PATH`, con el que corre el conversor.
fn python3(env: &Environment) -> Result<PathBuf, Error> {
    toolchain::buscar_en_path(env, "python3").ok_or_else(|| Error::Disco {
        que: "buscar 'python3'".to_string(),
        detalle: "el conversor necesita python3 en el PATH".to_string(),
    })
}

/// Las últimas líneas de un texto (la cola del log).
fn cola(texto: &str, cuantas: usize) -> Vec<String> {
    let lineas: Vec<String> = texto.lines().map(str::to_string).collect();
    let desde = lineas.len().saturating_sub(cuantas);
    lineas[desde..].to_vec()
}

/// Si **todas** las salidas ya están (y no hace falta rehacer nada).
fn ya_estan_todas(plan: &Plan) -> bool {
    plan.salidas.iter().all(|s| s.ruta.is_file())
}

/// El origen de la tirada, para la política de limpieza.
fn origen_de(peticion: &Peticion) -> Origen {
    match &peticion.fuente {
        Fuente::Repo(_) => Origen::ClonPropio,
        Fuente::Path(_) => Origen::PesosDelUsuario,
        Fuente::Gguf(gguf) => {
            if Path::new(gguf).is_file() {
                Origen::GgufDelUsuario
            } else {
                Origen::ClonPropio
            }
        }
    }
}

// ── Los primitivos de disco ─────────────────────────────────────────────────

/// Lee un fichero de texto (vacío si no está o no es texto).
fn lee(ruta: &Path) -> String {
    std::fs::read_to_string(ruta).unwrap_or_default()
}

/// Escribe un fichero de texto.
fn escribe(ruta: &Path, texto: &str) -> Result<(), String> {
    std::fs::write(ruta, texto).map_err(|e| e.to_string())
}

/// Escribe el checkpoint de una fase completada, **atómicamente**.
///
/// Se escribe a un temporal y se renombra: el rename es atómico, así que
/// el checkpoint nunca queda a medias (dos tiradas a la vez no pueden
/// corromperlo) y un corte de luz no lo deja truncado.
///
/// El directorio puede no existir todavía (un `--gguf` local no crea el
/// work hasta la limpieza).
fn escribe_cp(ruta: &Path, fase: Fase, huella: &str, cuantizados: &[String]) -> Result<(), Error> {
    if let Some(padre) = ruta.parent() {
        crea_dir(padre)?;
    }
    let temporal = temporal_de(ruta);
    escribe(
        &temporal,
        &Checkpoint::con_cuantizados(fase, huella, cuantizados).a_json(),
    )
    .map_err(|detalle| Error::Disco {
        que: format!("escribir el checkpoint ({})", fase.nombre()),
        detalle,
    })?;
    std::fs::rename(&temporal, ruta).map_err(|detalle| Error::Disco {
        que: format!("renombrar el checkpoint ({})", fase.nombre()),
        detalle: detalle.to_string(),
    })
}

/// El nombre del temporal de un fichero: `<nombre>.tmp`, al lado.
fn temporal_de(ruta: &Path) -> PathBuf {
    let mut nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    nombre.push_str(".tmp");
    ruta.with_file_name(nombre)
}

/// Crea un directorio (y sus padres).
fn crea_dir(ruta: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(ruta).map_err(|detalle| Error::Disco {
        que: format!("crear el directorio {}", ruta.to_string_lossy()),
        detalle: detalle.to_string(),
    })
}

/// Borra un fichero, si está.
fn borra_fichero(ruta: &Path, que: &str) -> Result<(), Error> {
    if ruta.is_file() {
        std::fs::remove_file(ruta).map_err(|detalle| Error::Disco {
            que: format!("{que} ({})", ruta.to_string_lossy()),
            detalle: detalle.to_string(),
        })?;
    }
    Ok(())
}

/// Borra un directorio (y su contenido), si está.
fn borra_dir(ruta: &Path, que: &str) -> Result<(), Error> {
    if ruta.is_dir() {
        std::fs::remove_dir_all(ruta).map_err(|detalle| Error::Disco {
            que: format!("{que} ({})", ruta.to_string_lossy()),
            detalle: detalle.to_string(),
        })?;
    }
    Ok(())
}

/// Copia un fichero (los GGUF son grandes, pero copiar es lo seguro: un
/// `mv` del intermedio dejaría sin intermedio al siguiente quant).
fn copia(de: &Path, a: &Path, que: &str) -> Result<(), Error> {
    std::fs::copy(de, a)
        .map(|_| ())
        .map_err(|detalle| Error::Disco {
            que: format!("{que}: {} → {}", de.to_string_lossy(), a.to_string_lossy()),
            detalle: detalle.to_string(),
        })
}

/// Escribe el sentinel del clon completo.
fn escribe_sentinel(ruta: &Path) -> Result<(), Error> {
    escribe(ruta, "ok\n").map_err(|detalle| Error::Disco {
        que: format!("escribir el sentinel {}", ruta.to_string_lossy()),
        detalle,
    })
}
