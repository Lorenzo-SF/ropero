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
    /// Qué intermedio se usó (el sufijo: F16, BF16, NVFP4…).
    pub intermedio: String,
    /// La importance matrix, si se usó.
    pub imatrix: Option<ImatrixInfo>,
    /// La cabeza MTP, si quedó en un quant aparte.
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
    /// no debe romper el JSON).
    pub fn a_json(&self) -> String {
        let imx = match &self.imatrix {
            None => "null".to_string(),
            Some(imx) => {
                let mut partes = vec![format!("\"fichero\": \"{}\"", escapa(&imx.fichero))];
                if let Some(repo) = &imx.repo {
                    partes.push(format!("\"repo\": \"{}\"", escapa(repo)));
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
        format!(
            "{{\n  \"gguf\": \"{}\",\n  \"quant\": \"{}\",\n  \"fuente_repo\": \"{}\",\n  \
             \"intermedio\": \"{}\",\n  \"imatrix\": {},\n  \"mtp\": {},\n  \
             \"herramienta\": \"ropero gguffer (llama.cpp llama-quantize)\",\n  \"fecha\": \"{}\"\n}}\n",
            escapa(&self.gguf),
            escapa(&self.quant),
            escapa(&self.fuente_repo),
            escapa(&self.intermedio),
            imx,
            mtp,
            escapa(&self.fecha)
        )
    }
}

/// Escapa los caracteres que romperían un valor JSON (comillas y barras).
fn escapa(texto: &str) -> String {
    texto.replace('\\', "\\\\").replace('"', "\\\"")
}
