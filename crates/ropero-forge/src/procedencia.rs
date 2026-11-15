//! La **procedencia**: el sidecar `<out>.fuente.json` (R-GGUF-19).
//!
//! > «Un GGUF sin saber CÓMO se hizo no se puede publicar con vergüenza»
//!
//! Todo GGUF producido deja, al lado, un fichero JSON con de dónde salió
//! el modelo, con qué quant (**por nombre**, no por número: es lo que va
//! al model card), con qué importance matrix, y si la cabeza MTP quedó en
//! un quant aparte.
//!
//! ## El formato
//!
//! Se escribe **a mano** (sin serde): es un JSON plano de doce líneas, el
//! mismo que escribía la v1, y así el crate no arrastra dependencias. Si
//! no se puede escribir, **la tirada falla** (T-34: un GGUF sin sidecar se
//! rechaza).

use std::path::{Path, PathBuf};

/// La cabeza MTP, si quedó en un quant aparte (R-GGUF-11…14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtpInfo {
    /// El bloque de la MTP (el 64 en el Qwen3.8 denso).
    pub bloque: u32,
    /// El quant en que quedó (Q4_0, que no exige imatrix).
    pub quant: String,
}

/// La importance matrix usada, tal y como va al sidecar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImatrixInfo {
    /// El nombre del fichero (sin la ruta: la ruta es local).
    pub fichero: String,
    /// El repo de donde vino, si fue remota.
    pub repo: Option<String>,
    /// El **sha256** de la matriz: el mismo nombre puede ser otra matriz, y
    /// con el hash se sabe cuál se usó (la v1 lo tenía; el modelo card lo
    /// enseña).
    pub sha256: Option<String>,
}

/// La procedencia de un GGUF producido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Procedencia {
    /// El nombre del GGUF (`<nombre>.gguf`).
    pub gguf: String,
    /// El quant, **por nombre** (R-GGUF-19).
    pub quant: String,
    /// De dónde salió: el repo, la ruta de los pesos o el GGUF de origen.
    pub fuente_repo: String,
    /// El **repo del modelo base** (el de los pesos sin cuantizar). `None`
    /// si no se puede saber: no se inventa (R-GGUF-19).
    pub base_model: Option<String>,
    /// El **nombre del modelo base**. `None` si no se puede saber.
    pub base_model_name: Option<String>,
    /// La **arquitectura** del modelo (la de su cabecera, `general.architecture`).
    /// `None` si la cabecera no lo dice.
    pub arch: Option<String>,
    /// Los **bloques** del modelo (los de su cabecera,
    /// `<arquitectura>.block_count`). `None` si la cabecera no lo dice.
    pub block_count: Option<u64>,
    /// Qué intermedio se usó (el sufijo: F16, BF16, NVFP4…). `None` si **no lo
    /// hubo**: con una fuente que ya es un GGUF no hay conversión, y el sidecar
    /// no puede afirmar un intermedio que no existió (R-GGUF-19).
    pub intermedio: Option<String>,
    /// La importance matrix, si se usó.
    pub imatrix: Option<ImatrixInfo>,
    /// La herramienta que **se usó** (R-GGUF-19: lo que pasó, no lo que se
    /// supone). `null` en el JSON cuando no se registró.
    pub herramienta: String,
    /// La cabeza MTP, si quedó en un quant aparte. `None` significa **no se
    /// registró** (la forja no puede conocer el bloque de la cabeza sin leer
    /// el GGUF entero: T-33), no que no la haya.
    pub mtp: Option<MtpInfo>,
    /// La fecha de la fabricación, en formato ISO (la pasa quien llama,
    /// porque el reloj es E/S).
    pub fecha: String,
}

impl Procedencia {
    /// La ruta del sidecar: `<out>.fuente.json`.
    pub fn ruta(gguf: &Path) -> PathBuf {
        let mut ruta = gguf.as_os_str().to_owned();
        ruta.push(".fuente.json");
        PathBuf::from(ruta)
    }

    /// El sidecar, como texto JSON.
    ///
    /// Los valores que llevan comillas se escapan (una ruta con comillas
    /// no debe romper el JSON). Los campos que **no** se pueden calcular
    /// **no están**: un `null` o una cadena vacía serían afirmaciones
    /// falsas (R-GGUF-19: la procedencia dice lo que pasó, no lo que se
    /// supone).
    pub fn a_json(&self) -> String {
        let imx = match &self.imatrix {
            None => "null".to_string(),
            Some(imx) => {
                let mut partes = vec![format!("\"fichero\": \"{}\"", escapa(&imx.fichero))];
                if let Some(repo) = &imx.repo {
                    partes.push(format!("\"repo\": \"{}\"", escapa(repo)));
                }
                if let Some(sha) = &imx.sha256 {
                    partes.push(format!("\"sha256\": \"{}\"", escapa(sha)));
                }
                format!("{{{}}}", partes.join(", "))
            }
        };
        let mtp = match &self.mtp {
            None => "null".to_string(),
            Some(mtp) => format!(
                "{{\"bloque\": {}, \"quant\": \"{}\", \
                 \"nota\": \"la cabeza MTP no la cubre el imatrix (se calcula sobre el \
                 modelo base), asi que queda en este quant\"}}",
                mtp.bloque,
                escapa(&mtp.quant)
            ),
        };
        let intermedio = match &self.intermedio {
            None => "null".to_string(),
            Some(intermedio) => format!("\"{}\"", escapa(intermedio)),
        };
        // Los campos opcionales: si están, van; si no, **no están**.
        let mut campos = vec![
            format!("  \"gguf\": \"{}\"", escapa(&self.gguf)),
            format!("  \"quant\": \"{}\"", escapa(&self.quant)),
            format!("  \"fuente_repo\": \"{}\"", escapa(&self.fuente_repo)),
        ];
        if let Some(base) = &self.base_model {
            campos.push(format!("  \"base_model\": \"{}\"", escapa(base)));
        }
        if let Some(nombre) = &self.base_model_name {
            campos.push(format!("  \"base_model_name\": \"{}\"", escapa(nombre)));
        }
        if let Some(arch) = &self.arch {
            campos.push(format!("  \"arch\": \"{}\"", escapa(arch)));
        }
        if let Some(bloques) = self.block_count {
            campos.push(format!("  \"block_count\": {bloques}"));
        }
        campos.push(format!("  \"intermedio\": {intermedio}"));
        campos.push(format!("  \"imatrix\": {imx}"));
        campos.push(format!("  \"mtp\": {mtp}"));
        campos.push(format!(
            "  \"herramienta\": \"{}\"",
            escapa(&self.herramienta)
        ));
        campos.push(format!("  \"fecha\": \"{}\"", escapa(&self.fecha)));
        format!("{{\n{}\n}}\n", campos.join(",\n"))
    }
}

/// Escapa los caracteres que romperían un valor JSON (comillas y barras).
fn escapa(texto: &str) -> String {
    texto.replace('\\', "\\\\").replace('"', "\\\"")
}
