//! La E/S de la instalación: el terminal, el binario del motor y el enlace.
//!
//! ## Qué hay aquí y por qué
//!
//! Los motores con receta `install.source` se clonan y compilan primero; los
//! demás requieren un binario ya instalado. En ambos casos se crea el enlace
//! en `$BIN_DIR`.
//!
//! La **precedencia** del binario es la del doc A (R-INST-24):
//! **PATH → `$BIN_DIR` → rutas largas**. El motor que se usa es el que
//! está en el `PATH`, no el más nuevo ni el recién compilado.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

use ropero_core::EngineManifest;
use ropero_core::Environment;
use std::process::{Command, Stdio};

use crate::catalog_scan::es_ejecutable;

fn llama_server_cmake_args(env: &Environment) -> Result<Vec<String>, String> {
    let os = env.get("ROPERO_BUILD_OS").unwrap_or(std::env::consts::OS);
    match os {
        "macos" | "darwin" => Ok([
            "-DCMAKE_BUILD_TYPE=Release",
            "-DCMAKE_OSX_ARCHITECTURES=arm64",
            "-DCMAKE_C_COMPILER=clang",
            "-DCMAKE_CXX_COMPILER=clang++",
            "-DCMAKE_C_FLAGS=-O3 -mcpu=native",
            "-DCMAKE_CXX_FLAGS=-O3 -mcpu=native",
            "-DGGML_METAL=ON",
            "-DGGML_METAL_NDEBUG=ON",
            "-DGGML_METAL_EMBED_LIBRARY=ON",
            "-DGGML_NATIVE=ON",
            "-DGGML_ACCELERATE=ON",
            "-DGGML_OPENMP=OFF",
            "-DGGML_BLAS=OFF",
            "-DLLAMA_OPENSSL=ON",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DLLAMA_BUILD_SERVER=ON",
            "-DLLAMA_BUILD_TOOLS=ON",
            "-DLLAMA_BUILD_APP=OFF",
            "-DLLAMA_BUILD_EXAMPLES=OFF",
            "-DLLAMA_BUILD_TESTS=OFF",
            "-DLLAMA_BUILD_UI=OFF",
            "-DLLAMA_BUILD_HTML=OFF",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()),
        "linux" => llama_server_linux_cmake_args(env),
        other => Err(format!(
            "no hay receta de compilación de llama-server para {other}"
        )),
    }
}

fn llama_server_linux_cmake_args(env: &Environment) -> Result<Vec<String>, String> {
    let cuda = env
        .get("ROPERO_CUDA_PATH")
        .map(PathBuf::from)
        .or_else(|| ["/opt/cuda-13.3", "/opt/cuda"].iter().map(PathBuf::from).find(|p| p.join("bin/nvcc").is_file()))
        .ok_or_else(|| "no se encontró nvcc (ROPERO_CUDA_PATH o /opt/cuda); llama-server requiere CUDA en Linux".to_string())?;
    let nvcc = cuda.join("bin/nvcc");
    if !nvcc.is_file() {
        return Err(format!("no se encontró nvcc en {}", nvcc.display()));
    }
    let host_cxx = env
        .get("ROPERO_HOST_CXX")
        .map(PathBuf::from)
        .or_else(|| {
            ["/usr/bin/g++-14", "/usr/bin/g++-13", "/usr/bin/clang++"]
                .iter()
                .map(PathBuf::from)
                .find(|p| p.is_file())
        })
        .ok_or_else(|| {
            "no se encontró compilador C++ host (g++-14, g++-13 o clang++)".to_string()
        })?;
    let host_cc = host_cxx
        .to_string_lossy()
        .replace("g++", "gcc")
        .replace("clang++", "clang");
    let mut args = vec![
        "-DCMAKE_BUILD_TYPE=Release".to_string(),
        format!("-DCUDA_TOOLKIT_ROOT_DIR={}", cuda.display()),
        format!("-DCMAKE_CUDA_COMPILER={}", nvcc.display()),
        format!("-DCMAKE_CUDA_HOST_COMPILER={}", host_cxx.display()),
        format!("-DCMAKE_CXX_COMPILER={}", host_cxx.display()),
        format!("-DCMAKE_C_COMPILER={host_cc}"),
    ];
    args.extend(
        [
            "-DCMAKE_CXX_FLAGS=-march=native -O3",
            "-DCMAKE_C_FLAGS=-march=native -O3",
            "-DCMAKE_CUDA_FLAGS=--use_fast_math -O3",
            "-DCMAKE_CUDA_ARCHITECTURES=native",
            "-DCMAKE_INTERPROCEDURAL_OPTIMIZATION=ON",
            "-DGGML_CUDA=ON",
            "-DGGML_CUDA_FA=ON",
            "-DGGML_CUDA_FA_ALL_QUANTS=ON",
            "-DGGML_CUDA_GRAPHS=ON",
            "-DGGML_CUDA_FORCE_MMQ=OFF",
            "-DGGML_CUDA_FORCE_CUBLAS=OFF",
            "-DGGML_CUDA_COMPRESSION_MODE=speed",
            "-DGGML_RPC=ON",
            "-DGGML_OPENMP=ON",
            "-DGGML_NATIVE=ON",
            "-DGGML_AVX2=ON",
            "-DGGML_AVX512=ON",
            "-DGGML_AVX512_VBMI=ON",
            "-DGGML_AVX512_VNNI=ON",
            "-DGGML_AVX512_BF16=ON",
            "-DGGML_FMA=ON",
            "-DLLAMA_OPENSSL=ON",
            "-DBUILD_SHARED_LIBS=OFF",
            "-DLLAMA_BUILD_SERVER=ON",
            "-DLLAMA_BUILD_TOOLS=ON",
            "-DLLAMA_BUILD_APP=OFF",
            "-DLLAMA_BUILD_EXAMPLES=OFF",
            "-DLLAMA_BUILD_TESTS=OFF",
            "-DLLAMA_BUILD_UI=OFF",
            "-DLLAMA_BUILD_HTML=OFF",
        ]
        .into_iter()
        .map(str::to_string),
    );
    Ok(args)
}

/// Si hay terminal: los menús y las preguntas sólo tienen sentido con TTY.
///
/// Sin TTY, `ropero install engines` es un **error de uso** (código 2):
/// un menú que se traga un EOF acabaría eligiendo por su cuenta
/// (R-INST-26).
pub fn hay_tty() -> bool {
    std::io::stdin().is_terminal() && std::io::stdout().is_terminal()
}

/// Escribe una pregunta y lee la respuesta del usuario.
///
/// `None` si no hay nada que leer (EOF): el llamante decide qué hacer.
pub fn pregunta(texto: &str) -> Option<String> {
    print!("{texto}");
    let _ = std::io::stdout().flush();
    let mut linea = String::new();
    match std::io::stdin().lock().read_line(&mut linea) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(linea.trim().to_string()),
    }
}

/// El directorio de binarios: `$ROPERO_BIN_DIR` o `$HOME/.local/bin`.
pub fn bin_dir(env: &Environment) -> PathBuf {
    env.env_path("ROPERO_BIN_DIR", ".local/bin")
}

/// Busca un ejecutable en el `PATH` **del entorno capturado**.
///
/// Se usa el entorno capturado y no el del proceso para que el `PATH` que
/// cuenta sea el que ve el dominio (doc B §3.2.3).
pub fn which_en(env: &Environment, binario: &str) -> Option<PathBuf> {
    let path = env.get("PATH")?;
    std::env::split_paths(path)
        .map(|dir| dir.join(binario))
        .find(|candidato| es_ejecutable(candidato))
}

/// Resuelve el binario de un motor: **PATH → `$BIN_DIR` → rutas largas**
/// (R-INST-24).
pub fn resolve_binario(manifest: &EngineManifest, env: &Environment) -> Option<PathBuf> {
    let dir = bin_dir(env);
    for binario in &manifest.detect.binaries {
        if let Some(candidato) = which_en(env, binario) {
            return Some(candidato);
        }
    }
    for binario in &manifest.detect.binaries {
        let candidato = dir.join(binario);
        if candidato.is_file() {
            return Some(candidato);
        }
    }
    for ruta in &manifest.detect.paths {
        let candidato = PathBuf::from(ruta);
        if candidato.exists() {
            return Some(candidato);
        }
    }
    None
}

/// Lo que pasó al instalar un motor.
pub use ropero_core::install::EstadoMotor;

/// Instala un motor: **sólo crea el enlace** (R-INST-1).
///
/// El nombre del enlace usa el basename del binario resuelto; Turbo-Llama
/// lo prefija para no pisar el `llama-server` oficial (R-INST-25).
pub fn instala_motor(
    manifest: &EngineManifest,
    env: &Environment,
    force: bool,
) -> Result<EstadoMotor, String> {
    let dir = bin_dir(env);
    if manifest.id == ropero_core::EngineId::Ignis {
        return instala_ignis(manifest, env, force, &dir);
    }
    if manifest.id == ropero_core::EngineId::Basalt {
        return instala_basalt(manifest, env, force, &dir);
    }
    if manifest.id == ropero_core::EngineId::Colibri {
        return instala_colibri(manifest, env, force, &dir);
    }
    let (origen, source_dir) =
        if let Some(install) = manifest.install.as_ref().filter(|i| i.source.is_some()) {
            compila_motor(manifest, install, env, force)?
        } else if let Some(origen) = resolve_binario(manifest, env) {
            (origen, None)
        } else {
            return Ok(EstadoMotor::SinBinario);
        };
    enlaza_ejecutables(
        manifest,
        &origen,
        source_dir.as_deref(),
        source_dir.is_some(),
        &dir,
    )
}

/// Instala una distribución oficial de Colibri, valida su checksum y enlaza
/// el dispatcher `coli`. Los modelos siguen en el almacén que administra
/// Colibri, sin duplicar descargas en el catálogo de Ropero.
fn instala_colibri(
    manifest: &EngineManifest,
    env: &Environment,
    force: bool,
    bin_dir: &Path,
) -> Result<EstadoMotor, String> {
    if let Some(binary) = resolve_binario(manifest, env)
        && !force
    {
        return enlaza_ejecutables(manifest, &binary, None, false, bin_dir);
    }
    let asset = colibri_asset(std::env::consts::OS, std::env::consts::ARCH)?;
    let gh =
        which_en(env, "gh").ok_or_else(|| "falta la dependencia de Colibri: gh".to_string())?;
    let tar =
        which_en(env, "tar").ok_or_else(|| "falta la dependencia de Colibri: tar".to_string())?;
    let checksum_tool = which_en(env, "sha256sum")
        .map(|path| (path, false))
        .or_else(|| which_en(env, "shasum").map(|path| (path, true)))
        .ok_or_else(|| "falta sha256sum o shasum para validar Colibri".to_string())?;
    let release = Command::new(&gh)
        .args([
            "release",
            "view",
            "--repo",
            "JustVugg/colibri",
            "--json",
            "tagName",
            "--jq",
            ".tagName",
        ])
        .output()
        .map_err(|error| format!("no se pudo consultar el release de Colibri: {error}"))?;
    if !release.status.success() {
        return Err(format!(
            "gh release view de JustVugg/colibri terminó con {}: {}",
            release.status,
            String::from_utf8_lossy(&release.stderr).trim()
        ));
    }
    let version = String::from_utf8_lossy(&release.stdout).trim().to_string();
    if version.is_empty()
        || !version.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_')
        })
    {
        return Err(format!("tag de release de Colibri no válido: {version:?}"));
    }
    let archive = asset.replace("{version}", &version);
    let root = env.home.join(".local/share/ropero/engines/colibri");
    let version_dir = root.join(&version);
    if version_dir.exists() && !force {
        let metadata = std::fs::symlink_metadata(&version_dir).map_err(|error| {
            format!("no se pudo inspeccionar {}: {error}", version_dir.display())
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!(
                "la ruta de instalación de Colibri no es un directorio real: {}",
                version_dir.display()
            ));
        }
        if let Some(executable) =
            busca_archivo(&version_dir, "coli").filter(|path| es_ejecutable(path))
        {
            return enlaza_ejecutables(manifest, &executable, None, false, bin_dir);
        }
        return Err(format!(
            "la instalación existente de Colibri está incompleta: {}",
            version_dir.display()
        ));
    }
    let scratch =
        std::env::temp_dir().join(format!("ropero-colibri-{}-{version}", std::process::id()));
    if scratch.exists() {
        return Err(format!(
            "el directorio temporal ya existe y no se modificará: {}",
            scratch.display()
        ));
    }
    std::fs::create_dir_all(&scratch)
        .map_err(|error| format!("no se pudo crear {}: {error}", scratch.display()))?;
    let result = (|| {
        let status = Command::new(&gh)
            .args([
                "release",
                "download",
                &version,
                "--repo",
                "JustVugg/colibri",
                "--pattern",
                &archive,
                "--pattern",
                "SHA256SUMS.txt",
                "--dir",
            ])
            .arg(&scratch)
            .status()
            .map_err(|error| format!("no se pudo descargar el release de Colibri: {error}"))?;
        if !status.success() {
            return Err(format!(
                "gh release download de Colibri terminó con {status}"
            ));
        }
        let checksum = scratch.join("SHA256SUMS.txt");
        if !checksum.is_file() || !scratch.join(&archive).is_file() {
            return Err("el release de Colibri no contiene el archivo o SHA256SUMS.txt".into());
        }
        let checksum_text = std::fs::read_to_string(&checksum)
            .map_err(|error| format!("no se pudo leer SHA256SUMS.txt de Colibri: {error}"))?;
        let checksum_line = checksum_text
            .lines()
            .find(|line| {
                line.split_whitespace()
                    .last()
                    .is_some_and(|name| name.trim_start_matches('*') == archive)
            })
            .ok_or_else(|| format!("SHA256SUMS.txt no contiene un checksum para {archive}"))?;
        let selected_checksum = scratch.join("colibri.sha256");
        std::fs::write(&selected_checksum, format!("{checksum_line}\n"))
            .map_err(|error| format!("no se pudo preparar el checksum de Colibri: {error}"))?;
        let mut checksum_command = Command::new(&checksum_tool.0);
        if checksum_tool.1 {
            checksum_command.args(["-a", "256", "-c", "colibri.sha256"]);
        } else {
            checksum_command.args(["-c", "colibri.sha256"]);
        }
        let status = checksum_command
            .current_dir(&scratch)
            .status()
            .map_err(|error| format!("no se pudo validar SHA-256 de Colibri: {error}"))?;
        if !status.success() {
            return Err("la verificación SHA-256 del release de Colibri falló".into());
        }
        if version_dir.exists() && force {
            let metadata = std::fs::symlink_metadata(&version_dir).map_err(|error| {
                format!("no se pudo inspeccionar {}: {error}", version_dir.display())
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "no se limpia una ruta de instalación que no sea un directorio real: {}",
                    version_dir.display()
                ));
            }
            std::fs::remove_dir_all(&version_dir).map_err(|error| {
                format!("no se pudo limpiar {}: {error}", version_dir.display())
            })?;
        }
        std::fs::create_dir_all(&version_dir)
            .map_err(|error| format!("no se pudo crear {}: {error}", version_dir.display()))?;
        let status = Command::new(&tar)
            .args(["-xzf"])
            .arg(scratch.join(&archive))
            .arg("-C")
            .arg(&version_dir)
            .status()
            .map_err(|error| format!("no se pudo extraer Colibri: {error}"))?;
        if !status.success() {
            return Err(format!("tar de Colibri terminó con {status}"));
        }
        let executable = busca_archivo(&version_dir, "coli")
            .filter(|path| es_ejecutable(path))
            .ok_or_else(|| {
                format!(
                    "el release no contiene `coli` ejecutable bajo {}",
                    version_dir.display()
                )
            })?;
        enlaza_ejecutables(manifest, &executable, None, false, bin_dir)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn colibri_asset(os: &str, arch: &str) -> Result<&'static str, String> {
    match (os, arch) {
        ("linux", "x86_64") => Ok("colibri-{version}-linux-x86_64.tar.gz"),
        ("macos", "aarch64") => Ok("colibri-{version}-macos-arm64.tar.gz"),
        _ => Err(format!(
            "Colibri no publica aquí un engine para {os}/{arch}"
        )),
    }
}

/// Instala el release oficial de Ignis en el almacén de motores de Ropero.
/// Descarga el tarball y su checksum con `gh`, valida SHA-256, y enlaza el
/// servidor desde `~/.local/bin`; los pesos quedan fuera de esta instalación.
fn instala_ignis(
    manifest: &EngineManifest,
    env: &Environment,
    force: bool,
    bin_dir: &Path,
) -> Result<EstadoMotor, String> {
    let _ = force;
    if !cfg!(target_os = "linux") || !cfg!(target_arch = "x86_64") {
        return Err("Ignis sólo publica aquí un binario CUDA para Linux x86_64".to_string());
    }
    let gh = which_en(env, "gh").ok_or_else(|| "falta la dependencia de Ignis: gh".to_string())?;
    let tar =
        which_en(env, "tar").ok_or_else(|| "falta la dependencia de Ignis: tar".to_string())?;
    let sha256sum = which_en(env, "sha256sum")
        .ok_or_else(|| "falta la dependencia de Ignis: sha256sum".to_string())?;
    let ldd =
        which_en(env, "ldd").ok_or_else(|| "falta la dependencia de Ignis: ldd".to_string())?;
    let release = Command::new(&gh)
        .args([
            "release",
            "view",
            "--repo",
            "gpillon/ignis",
            "--json",
            "tagName",
            "--jq",
            ".tagName",
        ])
        .output()
        .map_err(|e| format!("no se pudo consultar el release de Ignis: {e}"))?;
    if !release.status.success() {
        return Err(format!(
            "gh release view de gpillon/ignis terminó con {}: {}",
            release.status,
            String::from_utf8_lossy(&release.stderr).trim()
        ));
    }
    let version = String::from_utf8_lossy(&release.stdout).trim().to_string();
    if version.is_empty()
        || !version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    {
        return Err(format!("tag de release de Ignis no válido: {version:?}"));
    }
    let archive_name = format!("ignis-{version}-x86_64-unknown-linux-gnu.tar.gz");
    let scratch =
        std::env::temp_dir().join(format!("ropero-ignis-{}-{version}", std::process::id()));
    if scratch.exists() {
        return Err(format!(
            "el directorio temporal ya existe y no se modificará: {}",
            scratch.display()
        ));
    }
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("no se pudo crear {}: {e}", scratch.display()))?;
    let result = (|| {
        let status = Command::new(&gh)
            .args([
                "release",
                "download",
                &version,
                "--repo",
                "gpillon/ignis",
                "--pattern",
                &format!("{archive_name}*"),
                "--dir",
            ])
            .arg(&scratch)
            .status()
            .map_err(|e| format!("no se pudo descargar el release de Ignis: {e}"))?;
        if !status.success() {
            return Err(format!("gh release download de Ignis terminó con {status}"));
        }
        let checksum = scratch.join(format!("{archive_name}.sha256"));
        if !checksum.is_file() {
            return Err(format!(
                "el release de Ignis no contiene {}",
                checksum.display()
            ));
        }
        let status = Command::new(&sha256sum)
            .args([
                "-c",
                checksum
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .as_ref(),
            ])
            .current_dir(&scratch)
            .status()
            .map_err(|e| format!("no se pudo verificar SHA-256 del release de Ignis: {e}"))?;
        if !status.success() {
            return Err("la verificación SHA-256 del release de Ignis falló".to_string());
        }
        let root = env.home.join(".local/share/ropero/engines/ignis");
        let version_dir = root.join(&version);
        if version_dir.exists() && force {
            let metadata = std::fs::symlink_metadata(&version_dir)
                .map_err(|e| format!("no se pudo inspeccionar {}: {e}", version_dir.display()))?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(format!(
                    "no se limpia una ruta de instalación que no sea un directorio real: {}",
                    version_dir.display()
                ));
            }
            std::fs::remove_dir_all(&version_dir).map_err(|e| {
                format!(
                    "no se pudo limpiar la instalación anterior {}: {e}",
                    version_dir.display()
                )
            })?;
        }
        std::fs::create_dir_all(&version_dir)
            .map_err(|e| format!("no se pudo crear {}: {e}", version_dir.display()))?;
        let status = Command::new(&tar)
            .args(["-xzf"])
            .arg(scratch.join(&archive_name))
            .arg("-C")
            .arg(&version_dir)
            .status()
            .map_err(|e| format!("no se pudo extraer el release de Ignis: {e}"))?;
        if !status.success() {
            return Err(format!("tar de Ignis terminó con {status}"));
        }
        let executable = busca_archivo(&version_dir, "ignis-server")
            .filter(|path| es_ejecutable(path))
            .ok_or_else(|| {
                format!(
                    "el release de Ignis no contiene un ignis-server ejecutable bajo {}",
                    version_dir.display()
                )
            })?;
        let libraries = Command::new(&ldd)
            .arg(&executable)
            .output()
            .map_err(|e| format!("no se pudo revisar las bibliotecas de Ignis con ldd: {e}"))?;
        let ldd_output = format!(
            "{}{}",
            String::from_utf8_lossy(&libraries.stdout),
            String::from_utf8_lossy(&libraries.stderr)
        );
        if !libraries.status.success() || ldd_output.contains("not found") {
            return Err(format!(
                "el binario de Ignis tiene bibliotecas compartidas ausentes:\n{}",
                ldd_output.trim()
            ));
        }
        enlaza_ejecutables(manifest, &executable, None, false, bin_dir)
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

/// Descarga el release Linux oficial de Basalt y deja su servidor Python listo.
/// El pack del modelo permanece en el almacén de modelos de Ropero.
fn instala_basalt(
    manifest: &EngineManifest,
    env: &Environment,
    force: bool,
    bin_dir: &Path,
) -> Result<EstadoMotor, String> {
    if !cfg!(target_os = "linux") || !cfg!(target_arch = "x86_64") {
        return Err("Basalt sólo se instala desde Ropero en Linux x86-64".to_string());
    }
    if !cpu_avx2_available() {
        return Err("Basalt requiere una CPU x86-64 con AVX2".to_string());
    }
    valida_hardware_basalt(env)?;
    if let Some(install) = manifest.install.as_ref()
        && gestor_paquetes_disponible(install, env).is_some()
    {
        prepara_dependencias_sistema(install, env)?;
    }
    let python = which_en(env, "python3")
        .ok_or_else(|| "falta python3 (Basalt requiere Python 3.10 o posterior)".to_string())?;
    valida_python_basalt(&python)?;

    let root = env.home.join(".local/share/ropero/engines/basalt");
    if root.exists() {
        let meta = std::fs::symlink_metadata(&root)
            .map_err(|e| format!("no se pudo inspeccionar {}: {e}", root.display()))?;
        if meta.file_type().is_symlink() || !meta.is_dir() {
            return Err(format!(
                "la instalación Basalt existente no es un directorio real: {}",
                root.display()
            ));
        }
        if force {
            std::fs::remove_dir_all(&root).map_err(|e| {
                format!(
                    "no se pudo reemplazar la instalación Basalt {}: {e}",
                    root.display()
                )
            })?;
        } else if !root.join("bin/basalt").is_file() {
            return Err(format!(
                "{} ya existe pero no parece un release Basalt; mueve o elimina ese directorio y vuelve a instalar",
                root.display()
            ));
        }
    }

    if !root.join("bin/basalt").is_file() {
        instala_release_basalt(&python, &root)?;
    }
    prepara_python_basalt(&python, &root, env)?;
    if !cuda13_runtime_disponible(env, &root) {
        return Err(
            "no se pudieron preparar libcudart.so.13 y libcublas.so.13 en el entorno de Basalt"
                .to_string(),
        );
    }
    let executable = root.join("bin/basalt");
    if !es_ejecutable(&executable) {
        return Err(format!(
            "el release Basalt no contiene un bin/basalt ejecutable: {}",
            executable.display()
        ));
    }
    enlaza_ejecutables(manifest, &executable, None, false, bin_dir)
}

const MODELO_BASALT: &str = include_str!("../../../models/basalt/qwen3.8-flash-next-basalt.toml");

/// Registra el TOML de Basalt sin reemplazar una definición del usuario.
pub fn registra_modelo_basalt(env: &Environment) -> Result<bool, String> {
    let dir = env
        .config_resuelta()
        .models_toml_dir(ropero_core::EngineId::Basalt)
        .valor;
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("no se pudo crear {}: {error}", dir.display()))?;
    let path = dir.join("qwen3.8-flash-next-basalt.toml");
    let mut file = match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error) => return Err(format!("no se pudo crear {}: {error}", path.display())),
    };
    if let Err(error) = file.write_all(MODELO_BASALT.as_bytes()) {
        drop(file);
        let _ = std::fs::remove_file(&path);
        return Err(format!("no se pudo escribir {}: {error}", path.display()));
    }
    Ok(true)
}

fn valida_python_basalt(python: &Path) -> Result<(), String> {
    let output = Command::new(python)
        .args([
            "-c",
            "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ])
        .output()
        .map_err(|e| format!("no se pudo ejecutar Python para Basalt: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Python no pudo comprobar su versión: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let version = String::from_utf8_lossy(&output.stdout);
    let mut parts = version
        .trim()
        .split('.')
        .filter_map(|part| part.parse::<u32>().ok());
    let major = parts.next().unwrap_or_default();
    let minor = parts.next().unwrap_or_default();
    if (major, minor) < (3, 10) {
        return Err(format!(
            "Basalt requiere Python 3.10 o posterior; se encontró {}",
            version.trim()
        ));
    }
    Ok(())
}

fn valida_hardware_basalt(env: &Environment) -> Result<(), String> {
    let nvidia_smi = which_en(env, "nvidia-smi")
        .ok_or_else(|| "Basalt requiere nvidia-smi y una GPU NVIDIA Blackwell".to_string())?;
    let output = Command::new(nvidia_smi)
        .args([
            "--query-gpu=compute_cap,driver_version",
            "--format=csv,noheader",
        ])
        .output()
        .map_err(|e| format!("no se pudo consultar la GPU para Basalt: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "nvidia-smi no pudo consultar la GPU: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let rows = String::from_utf8_lossy(&output.stdout);
    let compatible = rows.lines().any(|row| {
        let mut columns = row.split(',').map(str::trim);
        let capability = columns.next().and_then(|s| s.parse::<f32>().ok());
        let driver = columns.next().and_then(version_tuple);
        capability.is_some_and(|value| (value - 12.0).abs() < f32::EPSILON)
            && driver.is_some_and(|value| value >= (580, 95, 5))
    });
    if !compatible {
        return Err(format!(
            "Basalt requiere NVIDIA Blackwell (compute capability 12.0) y driver Linux 580.95.05 o posterior; nvidia-smi informó: {}",
            rows.trim()
        ));
    }
    Ok(())
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn cpu_avx2_available() -> bool {
    std::is_x86_feature_detected!("avx2")
}

#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn cpu_avx2_available() -> bool {
    false
}

fn version_tuple(raw: &str) -> Option<(u32, u32, u32)> {
    let mut parts = raw.trim().split('.').map(str::parse::<u32>);
    Some((
        parts.next()?.ok()?,
        parts.next()?.ok()?,
        parts.next()?.ok()?,
    ))
}

fn cuda13_runtime_disponible(env: &Environment, root: &Path) -> bool {
    let mut dirs = Vec::new();
    if let Some(path) = env.get("ROPERO_CUDA_PATH") {
        let root = PathBuf::from(path);
        dirs.extend([root.join("lib64"), root.join("targets/x86_64-linux/lib")]);
    }
    if let Some(paths) = env.get("LD_LIBRARY_PATH") {
        dirs.extend(std::env::split_paths(paths));
    }
    dirs.extend([
        PathBuf::from("/usr/local/cuda/lib64"),
        PathBuf::from("/usr/local/cuda/targets/x86_64-linux/lib"),
        PathBuf::from("/usr/lib/x86_64-linux-gnu"),
    ]);
    if let Ok(entries) = std::fs::read_dir("/usr/local") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with("cuda-13") && entry.path().is_dir() {
                dirs.extend([
                    entry.path().join("lib64"),
                    entry.path().join("targets/x86_64-linux/lib"),
                ]);
            }
        }
    }
    dirs.extend(cuda_wheel_library_dirs(root));
    ["libcudart.so.13", "libcublas.so.13"]
        .iter()
        .all(|name| dirs.iter().any(|dir| dir.join(name).exists()))
}

pub(crate) fn cuda_wheel_library_dirs(root: &Path) -> Vec<PathBuf> {
    let packages = root.join(".venv/lib");
    let Ok(versions) = std::fs::read_dir(packages) else {
        return Vec::new();
    };
    versions
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("python"))
        .map(|entry| entry.path().join("site-packages/nvidia"))
        .flat_map(|nvidia| {
            std::fs::read_dir(nvidia)
                .into_iter()
                .flatten()
                .flatten()
                .map(|package| package.path().join("lib"))
                .collect::<Vec<_>>()
        })
        .filter(|path| path.is_dir())
        .collect()
}

fn instala_release_basalt(python: &Path, root: &Path) -> Result<(), String> {
    const DESCARGA: &str = r#"
import hashlib, json, pathlib, sys, urllib.request
api = 'https://api.github.com/repos/jesdga95/basalt/releases/latest'
request = urllib.request.Request(api, headers={'User-Agent': 'ropero-engine-installer', 'Accept': 'application/vnd.github+json'})
try:
    with urllib.request.urlopen(request, timeout=60) as response:
        release = json.load(response)
    assets = [item for item in release.get('assets', []) if item.get('name', '').startswith('basalt-v') and item.get('name', '').endswith('-linux-x64.tar.gz')]
    if len(assets) != 1:
        raise RuntimeError('el último release no contiene un único archivo basalt-v*-linux-x64.tar.gz')
    asset = assets[0]
    url = asset.get('browser_download_url', '')
    if not url.startswith('https://github.com/jesdga95/basalt/releases/download/'):
        raise RuntimeError('la URL del archivo no pertenece al release oficial de Basalt')
    digest = asset.get('digest', '')
    if not digest.startswith('sha256:'):
        raise RuntimeError('GitHub no publicó el SHA-256 del release; no se instala un archivo sin verificar')
    destination = pathlib.Path(sys.argv[1])
    request = urllib.request.Request(url, headers={'User-Agent': 'ropero-engine-installer'})
    checksum = hashlib.sha256()
    with urllib.request.urlopen(request, timeout=120) as response, destination.open('wb') as output:
        while True:
            chunk = response.read(8 * 1024 * 1024)
            if not chunk:
                break
            output.write(chunk)
            checksum.update(chunk)
    actual = checksum.hexdigest()
    if actual != digest.removeprefix('sha256:'):
        destination.unlink(missing_ok=True)
        raise RuntimeError('el SHA-256 del release Basalt no coincide con el digest publicado por GitHub')
    print('Basalt ' + asset['name'] + ' descargado')
except Exception as error:
    print('error al descargar Basalt: ' + str(error), file=sys.stderr)
    sys.exit(1)
"#;
    const EXTRAE: &str = r#"
import pathlib, sys, tarfile
archive = pathlib.Path(sys.argv[1])
destination = pathlib.Path(sys.argv[2])
try:
    with tarfile.open(archive, 'r:gz') as bundle:
        members = bundle.getmembers()
        roots = set()
        for member in members:
            path = pathlib.PurePosixPath(member.name)
            if path.is_absolute() or '..' in path.parts or not path.parts:
                raise RuntimeError('ruta insegura en el archivo: ' + member.name)
            if not (member.isfile() or member.isdir()):
                raise RuntimeError('tipo de entrada no permitido: ' + member.name)
            roots.add(path.parts[0])
        if len(roots) != 1 or not next(iter(roots)).startswith('basalt-v'):
            raise RuntimeError('el archivo no tiene una única carpeta raíz de release Basalt')
        bundle.extractall(destination, members=members)
except Exception as error:
    print('error al extraer Basalt: ' + str(error), file=sys.stderr)
    sys.exit(1)
"#;
    let scratch = std::env::temp_dir().join(format!("ropero-basalt-{}", std::process::id()));
    if scratch.exists() {
        return Err(format!(
            "el directorio temporal ya existe y no se modificará: {}",
            scratch.display()
        ));
    }
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("no se pudo crear {}: {e}", scratch.display()))?;
    let staging = match directorio_staging_basalt(root) {
        Ok(staging) => staging,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&scratch);
            return Err(error);
        }
    };
    let result = (|| {
        let archive = scratch.join("basalt-release.tar.gz");
        let output = Command::new(python)
            .arg("-c")
            .arg(DESCARGA)
            .arg(&archive)
            .status()
            .map_err(|e| format!("no se pudo iniciar el instalador Python de Basalt: {e}"))?;
        if !output.success() {
            return Err(format!(
                "la descarga del release Basalt terminó con {output}"
            ));
        }
        // Extract beside the final install path. Renaming a release from the
        // system temp directory can cross filesystems and fail with EXDEV.
        let extracted = staging.join("unpacked");
        std::fs::create_dir_all(&extracted)
            .map_err(|e| format!("no se pudo crear {}: {e}", extracted.display()))?;
        let output = Command::new(python)
            .arg("-c")
            .arg(EXTRAE)
            .arg(&archive)
            .arg(&extracted)
            .status()
            .map_err(|e| format!("no se pudo iniciar la extracción de Basalt: {e}"))?;
        if !output.success() {
            return Err(format!(
                "la extracción del release Basalt terminó con {output}"
            ));
        }
        let entries = std::fs::read_dir(&extracted)
            .map_err(|e| format!("no se pudo leer {}: {e}", extracted.display()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("no se pudo inspeccionar el release Basalt: {e}"))?;
        if entries.len() != 1 || !entries[0].path().join("bin/basalt").is_file() {
            return Err("el release descargado no tiene la estructura esperada de Basalt".into());
        }
        let release_root = entries[0].path();
        if root.exists() {
            return Err(format!(
                "no se reemplaza la instalación existente de Basalt en {}",
                root.display()
            ));
        }
        std::fs::rename(&release_root, root).map_err(|e| {
            format!(
                "no se pudo instalar el release Basalt en {}: {e}",
                root.display()
            )
        })?;
        Ok(())
    })();
    let _ = std::fs::remove_dir_all(&staging);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}

fn directorio_staging_basalt(root: &Path) -> Result<PathBuf, String> {
    let parent = root
        .parent()
        .ok_or_else(|| "la ruta de instalación Basalt no tiene directorio padre".to_string())?;
    let nombre = root
        .file_name()
        .ok_or_else(|| "la ruta de instalación Basalt no tiene nombre".to_string())?
        .to_string_lossy();
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
    for intento in 0..100 {
        let staging = parent.join(format!(
            ".{nombre}.staging-{}-{intento}",
            std::process::id()
        ));
        match std::fs::create_dir(&staging) {
            Ok(()) => return Ok(staging),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "no se pudo crear el staging de Basalt {}: {error}",
                    staging.display()
                ));
            }
        }
    }
    Err(format!(
        "no se pudo reservar un directorio staging junto a {}",
        root.display()
    ))
}

fn prepara_python_basalt(python: &Path, root: &Path, env: &Environment) -> Result<(), String> {
    let requirements = root.join("requirements.txt");
    if !requirements.is_file() {
        return Err(format!(
            "el release Basalt no contiene requirements.txt: {}",
            requirements.display()
        ));
    }
    let venv = root.join(".venv");
    if !venv.join("bin/python").is_file() {
        let status = Command::new(python)
            .args(["-m", "venv"])
            .arg(&venv)
            .status()
            .map_err(|e| format!("no se pudo crear el entorno Python de Basalt: {e}"))?;
        if !status.success() {
            return Err(format!(
                "python3 -m venv terminó con {status}; en Debian/Ubuntu instala python3-venv"
            ));
        }
    }
    let venv_python = venv.join("bin/python");
    let status = Command::new(&venv_python)
        .args(["-m", "pip", "install", "--disable-pip-version-check", "-r"])
        .arg(requirements)
        .status()
        .map_err(|e| format!("no se pudo instalar los requisitos Python de Basalt: {e}"))?;
    if !status.success() {
        return Err(format!("pip install de Basalt terminó con {status}"));
    }
    if !cuda13_runtime_disponible(env, root) {
        let status = Command::new(&venv_python)
            .args([
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "nvidia-cuda-runtime<14",
                "nvidia-cublas<14",
            ])
            .status()
            .map_err(|e| format!("no se pudo instalar CUDA 13 para Basalt: {e}"))?;
        if !status.success() {
            return Err(format!(
                "pip no pudo instalar CUDA 13 para Basalt: {status}"
            ));
        }
    }
    Ok(())
}

fn busca_archivo(root: &Path, nombre: &str) -> Option<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).ok()?.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == nombre) && path.is_file() {
                return Some(path);
            }
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
    None
}

fn enlaza_ejecutables(
    manifest: &EngineManifest,
    principal: &Path,
    source_dir: Option<&Path>,
    compilado: bool,
    dir: &Path,
) -> Result<EstadoMotor, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("no puedo crear {}: {e}", dir.display()))?;
    let ejecutables = if let Some(source_dir) = source_dir {
        ejecutables_compilados(source_dir, principal)?
    } else {
        vec![principal.to_path_buf()]
    };
    let turbo = manifest.id == ropero_core::EngineId::TurboLlama;
    let enlaces: Vec<(PathBuf, PathBuf)> = ejecutables
        .iter()
        .map(|origen| {
            let nombre = origen
                .file_name()
                .ok_or_else(|| format!("el ejecutable no tiene nombre: {}", origen.display()))?;
            let nombre = if turbo {
                format!("turbo-{}", nombre.to_string_lossy())
            } else {
                nombre.to_string_lossy().into_owned()
            };
            Ok((origen.clone(), dir.join(nombre)))
        })
        .collect::<Result<_, String>>()?;

    // Inspecciona todos los destinos antes de cambiar ninguno: un fichero o
    // enlace ajeno nunca se pisa y un conflicto no deja enlaces parciales.
    let mut regenerar = Vec::new();
    for (origen, destino) in &enlaces {
        match std::fs::symlink_metadata(destino) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let target = std::fs::read_link(destino)
                    .map_err(|e| format!("no puedo leer el enlace {}: {e}", destino.display()))?;
                let target = normaliza_enlace(destino, &target);
                let origen_normalizado = normaliza_ruta(origen);
                let ignis_root = if manifest.id == ropero_core::EngineId::Ignis {
                    origen
                        .ancestors()
                        .find(|path| path.file_name().is_some_and(|name| name == "ignis"))
                        .map(normaliza_ruta)
                } else {
                    None
                };
                let es_propietario = target == origen_normalizado
                    || ignis_root.is_some_and(|root| target.starts_with(root))
                    || source_dir.is_some_and(|root| target.starts_with(normaliza_ruta(root)));
                if !es_propietario {
                    return Err(format!(
                        "destino ocupado por un enlace ajeno: {} -> {}",
                        destino.display(),
                        target.display()
                    ));
                }
                regenerar.push(destino.clone());
            }
            Ok(meta)
                if manifest.id == ropero_core::EngineId::FastFlowLm
                    && meta.is_file()
                    && wrapper_flm_propio(destino, origen) =>
            {
                regenerar.push(destino.clone());
            }
            Ok(_) => {
                return Err(format!(
                    "destino ocupado por un fichero ajeno: {}",
                    destino.display()
                ));
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("no puedo inspeccionar {}: {e}", destino.display())),
        }
    }

    for destino in &regenerar {
        std::fs::remove_file(destino)
            .map_err(|e| format!("no puedo regenerar {}: {e}", destino.display()))?;
    }
    let mut principal_estado = None;
    for (origen, destino) in &enlaces {
        if manifest.id == ropero_core::EngineId::FastFlowLm {
            escribe_wrapper_flm(destino, origen)?;
        } else {
            std::os::unix::fs::symlink(origen, destino)
                .map_err(|e| format!("no puedo enlazar {}: {e}", destino.display()))?;
        }
        if origen == principal {
            principal_estado = Some(if regenerar.contains(destino) {
                EstadoMotor::YaEnlazado(destino.clone())
            } else {
                EstadoMotor::Enlazado {
                    origen: origen.clone(),
                    destino: destino.clone(),
                }
            });
        }
    }
    if let Some(source_dir) = source_dir {
        elimina_enlaces_obsoletos(dir, source_dir, &enlaces)?;
    }
    if compilado {
        return Ok(EstadoMotor::Actualizado {
            origen: principal.to_path_buf(),
            destino: enlaces
                .iter()
                .find(|(origen, _)| origen == principal)
                .map(|(_, destino)| destino.clone())
                .ok_or_else(|| {
                    format!(
                        "no se enlazó el ejecutable principal de {}",
                        manifest.id.as_str()
                    )
                })?,
        });
    }
    principal_estado
        .ok_or_else(|| format!("no se pudo crear el enlace de {}", manifest.id.as_str()))
}

fn wrapper_flm_propio(wrapper: &Path, binary: &Path) -> bool {
    std::fs::read_to_string(wrapper).ok().is_some_and(|text| {
        text.lines()
            .any(|line| line == "# ROPERO_MANAGED_FASTFLOWLLM")
            && text.contains(&format!("exec {} \"$@\"", shell_quote(binary)))
    })
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn escribe_wrapper_flm(wrapper: &Path, binary: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let contenido = format!(
        "#!/bin/sh\n# ROPERO_MANAGED_FASTFLOWLLM\nXILINX_XRT=\"${{XILINX_XRT:-/opt/xilinx/xrt}}\"\nexport XILINX_XRT\nif [ -d \"$XILINX_XRT/lib\" ]; then\n  LD_LIBRARY_PATH=\"$XILINX_XRT/lib${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}\"\n  export LD_LIBRARY_PATH\nfi\nif [ -d \"$XILINX_XRT/bin\" ]; then\n  PATH=\"$XILINX_XRT/bin:$PATH\"\n  export PATH\nfi\nexec {} \"$@\"\n",
        shell_quote(binary)
    );
    let temporary = wrapper.with_extension(format!("flm-wrapper-{}.tmp", std::process::id()));
    std::fs::write(&temporary, contenido).map_err(|error| {
        format!(
            "no se pudo escribir el lanzador {}: {error}",
            temporary.display()
        )
    })?;
    std::fs::set_permissions(&temporary, std::fs::Permissions::from_mode(0o755)).map_err(
        |error| {
            format!(
                "no se pudo hacer ejecutable {}: {error}",
                temporary.display()
            )
        },
    )?;
    std::fs::rename(&temporary, wrapper).map_err(|error| {
        format!(
            "no se pudo instalar el lanzador {}: {error}",
            wrapper.display()
        )
    })
}

fn ejecutables_compilados(source_dir: &Path, principal: &Path) -> Result<Vec<PathBuf>, String> {
    let build_dir = source_dir.join("build-ropero");
    let mut por_nombre = std::collections::BTreeMap::new();
    for bin_dir in [build_dir.join("bin"), build_dir.join("install/bin")] {
        let entries = match std::fs::read_dir(&bin_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(format!(
                    "no puedo leer los ejecutables de {}: {e}",
                    bin_dir.display()
                ));
            }
        };
        for entry in entries {
            let entry = entry
                .map_err(|e| format!("no puedo leer una entrada de {}: {e}", bin_dir.display()))?;
            let path = entry.path();
            if path.is_file()
                && es_ejecutable(&path)
                && let Some(name) = path.file_name()
            {
                // El segundo directorio es install/bin y prevalece en
                // caso de que CMake haya instalado el mismo ejecutable.
                por_nombre.insert(name.to_os_string(), path);
            }
        }
    }
    if !por_nombre.values().any(|path| path == principal) {
        return Err(format!(
            "el ejecutable principal {} no aparece entre los bins compilados",
            principal.display()
        ));
    }
    Ok(por_nombre.into_values().collect())
}

fn elimina_enlaces_obsoletos(
    dir: &Path,
    source_dir: &Path,
    enlaces: &[(PathBuf, PathBuf)],
) -> Result<(), String> {
    let vigentes: std::collections::BTreeSet<PathBuf> =
        enlaces.iter().map(|(_, destino)| destino.clone()).collect();
    let entries = std::fs::read_dir(dir).map_err(|e| {
        format!(
            "no puedo revisar enlaces obsoletos en {}: {e}",
            dir.display()
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("no puedo leer una entrada de {}: {e}", dir.display()))?;
        let path = entry.path();
        if vigentes.contains(&path) {
            continue;
        }
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_symlink() => {}
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("no puedo inspeccionar {}: {e}", path.display())),
        }
        let target = std::fs::read_link(&path)
            .map_err(|e| format!("no puedo leer el enlace {}: {e}", path.display()))?;
        if normaliza_enlace(&path, &target).starts_with(normaliza_ruta(source_dir)) {
            std::fs::remove_file(&path).map_err(|e| {
                format!(
                    "no puedo retirar el enlace obsoleto {}: {e}",
                    path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn normaliza_enlace(link: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        normaliza_ruta(target)
    } else {
        normaliza_ruta(&link.parent().unwrap_or(Path::new(".")).join(target))
    }
}

fn normaliza_ruta(path: &Path) -> PathBuf {
    use std::path::Component;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

/// Clona y compila una receta declarada. Reutilizar o borrar un checkout
/// existente requiere decisión explícita; no ejecuta comandos de shell.
fn prepara_python_llama(source_dir: &Path, env: &Environment) -> Result<(), String> {
    let python = which_en(env, "python3")
        .ok_or_else(|| "falta python3 para preparar el conversor GGUF".to_string())?;
    let venv = source_dir.join(".venv-ropero");
    let venv_python = venv.join("bin/python");
    if !venv_python.is_file() {
        let status = Command::new(&python)
            .args(["-m", "venv"])
            .arg(&venv)
            .status()
            .map_err(|e| format!("no se pudo crear el entorno Python de llama.cpp: {e}"))?;
        if !status.success() {
            return Err(format!("python3 -m venv terminó con {status}"));
        }
    }
    let requirements = source_dir.join("requirements/requirements-convert_hf_to_gguf.txt");
    if !requirements.is_file() {
        return Err(format!("no se encontró {}", requirements.display()));
    }
    let status = Command::new(&venv_python)
        .args(["-m", "pip", "install", "--disable-pip-version-check", "-r"])
        .arg(requirements)
        .current_dir(source_dir)
        .status()
        .map_err(|e| format!("no se pudieron instalar dependencias de conversión: {e}"))?;
    if !status.success() {
        return Err(format!(
            "pip install de dependencias de conversión terminó con {status}"
        ));
    }
    Ok(())
}

fn compila_motor(
    manifest: &EngineManifest,
    install: &ropero_core::EngineInstallSpec,
    env: &Environment,
    force: bool,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    prepara_dependencias_sistema(install, env)?;
    prepara_dependencias_aur(install, env)?;
    if manifest.id == ropero_core::EngineId::FastFlowLm {
        valida_runtime_flm(env)?;
    }
    let source = install
        .source
        .as_deref()
        .ok_or_else(|| "el manifiesto no declara install.source".to_string())?;
    let source_root = env.home.join(".local/share/ropero/engines");
    let expected_source = source_root.join(manifest.id.as_str());
    let source_dir = install
        .source_dir
        .as_deref()
        .map(|raw| env.expand_path(raw))
        .unwrap_or_else(|| {
            let legacy = source_root.join("fastflowlm");
            if manifest.id == ropero_core::EngineId::FastFlowLm
                && !expected_source.exists()
                && legacy.exists()
            {
                legacy
            } else {
                expected_source.clone()
            }
        });
    let is_legacy_fastflow_path = manifest.id == ropero_core::EngineId::FastFlowLm
        && source_dir == source_root.join("fastflowlm");
    if source_dir != expected_source && !is_legacy_fastflow_path {
        return Err(format!(
            "ruta fuente no permitida para '{}': debe ser {} (manifest indicó {})",
            manifest.id.as_str(),
            expected_source.display(),
            source_dir.display()
        ));
    }
    valida_ruta_fuente_segura(&env.home, &source_dir)?;
    let git =
        which_en(env, "git").ok_or_else(|| "falta la dependencia de build: git".to_string())?;
    let cmake =
        which_en(env, "cmake").ok_or_else(|| "falta la dependencia de build: cmake".to_string())?;
    let _ninja =
        which_en(env, "ninja").ok_or_else(|| "falta la dependencia de build: ninja".to_string())?;
    let args = if matches!(
        manifest.id,
        ropero_core::EngineId::LlamaServer | ropero_core::EngineId::TurboLlama
    ) {
        let args = llama_server_cmake_args(env)?;
        valida_compiladores(manifest, env)?;
        args
    } else {
        install.cmake_args.clone()
    };
    if force && source_dir.exists() {
        std::fs::remove_dir_all(&source_dir).map_err(|e| {
            format!(
                "no se pudo borrar el código fuente {}: {e}",
                source_dir.display()
            )
        })?;
    }
    let cmake_source = if let Some(subdir) = install.source_subdir.as_deref() {
        let relative = Path::new(subdir);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("install.source_subdir debe ser una ruta relativa segura".to_string());
        }
        source_dir.join(relative)
    } else {
        source_dir.clone()
    };
    let mut source_exists = cmake_source.join("CMakeLists.txt").is_file();
    if source_exists && !force {
        if !hay_tty() {
            return Err(format!(
                "ya existe un checkout en {}; usa --force para borrarlo o ejecuta con terminal para elegir si reutilizarlo",
                source_dir.display()
            ));
        }
        let respuesta = pregunta(&format!("Ya existe el repositorio {}. ¿Usar el existente (u) o borrar y clonar de nuevo (b)? [u/b]: ", source_dir.display()))
            .ok_or_else(|| "no se recibió respuesta; instalación cancelada".to_string())?;
        match respuesta.to_lowercase().as_str() {
            "u" | "usar" | "" => {}
            "b" | "borrar" | "reclonar" => {
                std::fs::remove_dir_all(&source_dir).map_err(|e| {
                    format!(
                        "no se pudo borrar el código fuente {}: {e}",
                        source_dir.display()
                    )
                })?;
                source_exists = false;
            }
            _ => {
                return Err(
                    "respuesta no válida; escribe 'u' o 'b'. Instalación cancelada".to_string(),
                );
            }
        }
    }
    if !source_exists {
        if source_dir.exists() {
            let mut entries = std::fs::read_dir(&source_dir)
                .map_err(|e| format!("no se pudo inspeccionar {}: {e}", source_dir.display()))?;
            if entries.next().is_some() {
                return Err(format!(
                    "el directorio fuente '{}' ya existe y no parece un checkout válido; no se modifica",
                    source_dir.display()
                ));
            }
        } else if let Some(parent) = source_dir.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("no se pudo crear {}: {e}", parent.display()))?;
        }
        let mut clone = Command::new(git);
        clone.args(["clone", "--depth", "1"]);
        if install.recursive {
            clone.args(["--recurse-submodules", "--shallow-submodules"]);
        }
        if let Some(source_ref) = &install.source_ref {
            clone.args(["--branch", source_ref]);
        }
        let output = clone
            .arg(source)
            .arg(&source_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|e| format!("no se pudo ejecutar git clone: {e}"))?;
        if !output.success() {
            return Err(format!("git clone de {source} terminó con {output}"));
        }
    }
    let build_dir = source_dir.join("build-ropero");
    let mut configure = Command::new(&cmake);
    configure
        .arg("-S")
        .arg(&cmake_source)
        .arg("-B")
        .arg(&build_dir);
    configure.args(["-G", "Ninja"]);
    configure.args(&args);
    if manifest.id == ropero_core::EngineId::FastFlowLm {
        crate::flm::configure_xrt_command(&mut configure, &env.vars);
    }
    if matches!(manifest.id, ropero_core::EngineId::FastFlowLm) {
        configure.arg(format!(
            "-DCMAKE_XCLBIN_PREFIX={}",
            source_dir.join("build-ropero/install/share/flm").display()
        ));
    }
    let status = configure
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("no se pudo ejecutar cmake configure: {e}"))?;
    if !status.success() {
        return Err(format!("cmake configure terminó con {status}"));
    }
    let mut build = Command::new(&cmake);
    build.arg("--build").arg(&build_dir);
    if manifest.id == ropero_core::EngineId::FastFlowLm {
        crate::flm::configure_xrt_command(&mut build, &env.vars);
    }
    if !matches!(
        manifest.id,
        ropero_core::EngineId::LlamaServer | ropero_core::EngineId::TurboLlama
    ) && let Some(target) = &install.target
    {
        build.args(["--target", target]);
    }
    let status = build
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("no se pudo ejecutar cmake --build: {e}"))?;
    if !status.success() {
        return Err(format!("cmake --build terminó con {status}"));
    }
    let install_dir = build_dir.join("install");
    let mut install_command = Command::new(&cmake);
    install_command
        .arg("--install")
        .arg(&build_dir)
        .arg("--prefix")
        .arg(&install_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if manifest.id == ropero_core::EngineId::FastFlowLm {
        crate::flm::configure_xrt_command(&mut install_command, &env.vars);
    }
    let status = install_command
        .status()
        .map_err(|e| format!("no se pudo ejecutar cmake --install: {e}"))?;
    if !status.success() {
        return Err(format!("cmake --install terminó con {status}"));
    }
    if matches!(
        manifest.id,
        ropero_core::EngineId::LlamaServer | ropero_core::EngineId::TurboLlama
    ) {
        prepara_python_llama(&source_dir, env)?;
    }
    let binary = install.binary.as_deref().unwrap_or("bin/llama-server");
    let executable = install_dir.join(binary);
    if !es_ejecutable(&executable) {
        return Err(format!(
            "el build terminó pero no existe el ejecutable {}",
            executable.display()
        ));
    }
    Ok((executable, Some(source_dir)))
}

/// Instala dependencias declaradas por el manifiesto usando el gestor del
/// sistema. El propio gestor solicita privilegios y confirma los cambios.
fn prepara_dependencias_sistema(
    install: &ropero_core::EngineInstallSpec,
    env: &Environment,
) -> Result<(), String> {
    if install.system_packages.is_empty() && install.apt_repositories.is_empty() {
        return Ok(());
    }
    let gestor = gestor_paquetes_disponible(install, env).ok_or_else(|| {
            let instrucciones = install
                .system_packages
                .iter()
                .map(|(manager, pkgs)| format!("{manager}: {}", pkgs.join(" ")))
                .collect::<Vec<_>>()
                .join("; ");
            format!(
                "no encuentro un gestor de paquetes compatible (pacman o apt-get); dependencias declaradas: {instrucciones}"
            )
        })?;
    prepara_repositorios_apt(install, gestor, env)?;
    let paquetes = install.system_packages.get(gestor)
        .or_else(|| if gestor == "apt" { install.system_packages.get("apt-get") } else { None })
        .ok_or_else(|| {
        format!(
            "el manifiesto no declara paquetes de sistema para {gestor}; no puedo completar la instalación de FLM"
        )
    })?;
    let faltantes: Vec<_> = paquetes
        .iter()
        .filter(|paquete| !paquete_instalado(gestor, paquete, env))
        .collect();
    if faltantes.is_empty() {
        return Ok(());
    }

    let es_root = Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "0");
    if !es_root && !hay_tty() {
        return Err(format!(
            "faltan dependencias de sistema: {}. Ejecuta `ropero install engines fastflowllm` desde una terminal para instalarlas con sudo",
            faltantes
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    let sudo = if es_root {
        None
    } else {
        Some(which_en(env, "sudo").ok_or_else(|| {
            format!(
                "faltan dependencias de sistema: {}. Instálalas como administrador",
                faltantes
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        })?)
    };
    let ejecutable_gestor = if gestor == "apt" && which_en(env, "apt-get").is_some() {
        "apt-get"
    } else {
        gestor
    };
    let mut command = sudo
        .as_ref()
        .map(Command::new)
        .unwrap_or_else(|| Command::new(ejecutable_gestor));
    if sudo.is_some() {
        command.arg(ejecutable_gestor);
    }
    match gestor {
        "pacman" => {
            command.args(["-S", "--needed"]);
        }
        "apt-get" | "apt" => {
            command.arg("install");
        }
        _ => unreachable!(),
    }
    command.args(faltantes.iter().map(|p| p.as_str()));
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("no se pudo ejecutar el gestor {gestor}: {e}"))?;
    if !status.success() {
        return Err(format!(
            "el gestor {gestor} no pudo instalar las dependencias de FastFlowLM (salida {status})"
        ));
    }
    let que_siguen_faltando: Vec<_> = faltantes
        .into_iter()
        .filter(|paquete| !paquete_instalado(gestor, paquete, env))
        .collect();
    if !que_siguen_faltando.is_empty() {
        return Err(format!(
            "el gestor {gestor} terminó sin instalar estos paquetes: {}",
            que_siguen_faltando
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    Ok(())
}

fn prepara_repositorios_apt(
    install: &ropero_core::EngineInstallSpec,
    gestor: &str,
    env: &Environment,
) -> Result<(), String> {
    if !matches!(gestor, "apt" | "apt-get") {
        return Ok(());
    }
    let distro = id_distribucion_linux().unwrap_or_default();
    let repos = install.apt_repositories.get(&distro);
    let packages = install
        .system_packages
        .get(gestor)
        .or_else(|| install.system_packages.get("apt-get"));

    if repos.is_none() {
        if let Some(paquetes_xrt) = packages.filter(|pkgs| pkgs.iter().any(|p| p == "libxrt-npu2"))
        {
            for paquete in paquetes_xrt
                .iter()
                .filter(|p| p.contains("xrt") || p.contains("amdxdna"))
            {
                if !paquete_instalado("apt-get", paquete, env) && !apt_tiene_candidato(paquete, env)
                {
                    return Err(format!(
                        "no hay paquete apt disponible para `{paquete}` en `{distro}`. FastFlowLM necesita XRT y el plugin XDNA compatibles; la guía oficial configura `ppa:lemonade-team/stable` en Ubuntu. En esta distribución configura el repositorio AMD adecuado y vuelve a ejecutar la instalación"
                    ));
                }
            }
        }
        return Ok(());
    }

    for repo in repos.expect("se comprobó arriba") {
        if repositorio_apt_configurado(repo) {
            continue;
        }
        if !hay_tty() {
            return Err(format!(
                "falta configurar el repositorio apt `{repo}`. Ejecuta `ropero install engines fastflowllm` desde una terminal para añadirlo con confirmación"
            ));
        }
        let paquetes_instalados = packages.is_some_and(|pkgs| {
            pkgs.iter()
                .any(|p| p == "software-properties-common" && paquete_instalado("apt-get", p, env))
        });
        if !paquetes_instalados {
            ejecuta_apt_privilegiado(env, "install", &["software-properties-common"])?;
        }
        ejecuta_admin(
            env,
            "add-apt-repository",
            &["-y", repo],
            "no se pudo añadir el repositorio XRT",
        )?;
        ejecuta_apt_privilegiado(env, "update", &[])?;
    }
    Ok(())
}

fn id_distribucion_linux() -> Option<String> {
    let os_release = std::fs::read_to_string("/etc/os-release").ok()?;
    os_release
        .lines()
        .find_map(|line| line.strip_prefix("ID="))
        .map(|value| value.trim_matches(['"', '\'']).to_ascii_lowercase())
}

fn repositorio_apt_configurado(repo: &str) -> bool {
    let needle = repo
        .strip_prefix("ppa:")
        .unwrap_or(repo)
        .to_ascii_lowercase();
    let mut files = vec![PathBuf::from("/etc/apt/sources.list")];
    if let Ok(entries) = std::fs::read_dir("/etc/apt/sources.list.d") {
        files.extend(entries.flatten().map(|entry| entry.path()));
    }
    files.into_iter().any(|path| {
        std::fs::read_to_string(path)
            .ok()
            .is_some_and(|contents| contents.to_ascii_lowercase().contains(&needle))
    })
}

fn apt_tiene_candidato(paquete: &str, env: &Environment) -> bool {
    let Some(apt_cache) = which_en(env, "apt-cache") else {
        return false;
    };
    Command::new(apt_cache)
        .args(["policy", paquete])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .any(|line| line.trim_start().starts_with("Candidate:") && !line.contains("(none)"))
        })
}

fn ejecuta_apt_privilegiado(
    env: &Environment,
    accion: &str,
    paquetes: &[&str],
) -> Result<(), String> {
    let apt = if which_en(env, "apt-get").is_some() {
        "apt-get"
    } else {
        "apt"
    };
    let args = std::iter::once(accion)
        .chain(paquetes.iter().copied())
        .collect::<Vec<_>>();
    ejecuta_admin(
        env,
        apt,
        &args,
        "no se pudo preparar el repositorio apt de XRT",
    )
}

fn ejecuta_admin(
    env: &Environment,
    programa: &str,
    args: &[&str],
    descripcion: &str,
) -> Result<(), String> {
    let es_root = Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .is_some_and(|output| String::from_utf8_lossy(&output.stdout).trim() == "0");
    let sudo = if es_root {
        None
    } else {
        Some(which_en(env, "sudo").ok_or_else(|| format!("{descripcion}: hace falta sudo"))?)
    };
    let mut command = sudo
        .as_ref()
        .map(Command::new)
        .unwrap_or_else(|| Command::new(programa));
    if sudo.is_some() {
        command.arg(programa);
    }
    let status = command
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("{descripcion}: {error}"))?;
    if !status.success() {
        return Err(format!("{descripcion} (salida {status})"));
    }
    Ok(())
}

/// Instala paquetes AUR sólo mediante un helper explícito del usuario
/// (`paru` o `yay`); pacman por sí solo no ejecuta recetas AUR.
fn prepara_dependencias_aur(
    install: &ropero_core::EngineInstallSpec,
    env: &Environment,
) -> Result<(), String> {
    if install.aur_packages.is_empty() {
        return Ok(());
    }
    if which_en(env, "pacman").is_none() {
        // Las dependencias AUR son la receta específica de Arch. En sistemas
        // apt se usan las dependencias declaradas para ese gestor.
        return Ok(());
    }
    let faltantes: Vec<_> = install
        .aur_packages
        .iter()
        .filter(|paquete| !paquete_instalado("pacman", paquete, env))
        .collect();
    if faltantes.is_empty() {
        return Ok(());
    }
    let helper = ["paru", "yay"]
        .iter()
        .find_map(|nombre| which_en(env, nombre).map(|ruta| (nombre, ruta)))
        .ok_or_else(|| {
            format!(
                "FLM requiere paquetes AUR: {}. Instala `paru` o `yay` y vuelve a ejecutar `ropero install engines fastflowllm`; pacman no instala paquetes AUR",
                faltantes.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(" ")
            )
        })?;
    if Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .is_some_and(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "0"
        })
    {
        return Err("no ejecutes la instalación AUR como root; vuelve a lanzar Ropero desde tu usuario normal".to_string());
    }
    if !hay_tty() {
        return Err(format!(
            "faltan paquetes AUR: {}. Ejecuta `ropero install engines fastflowllm` desde una terminal para revisar e instalar la receta con {}",
            faltantes
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            helper.0
        ));
    }
    let status = Command::new(helper.1)
        .args(["-S", "--needed"])
        .args(faltantes.iter().map(|p| p.as_str()))
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("no se pudo ejecutar {}: {error}", helper.0))?;
    if !status.success() {
        return Err(format!(
            "{} no pudo instalar los paquetes AUR de FastFlowLM (salida {status})",
            helper.0
        ));
    }
    Ok(())
}

/// Gestor declarado por el motor que está disponible en el `PATH` capturado.
/// Devuelve sólo métodos cuya invocación conoce Ropero; no infiere gestores
/// por el sistema operativo ni intenta instalar con una receta ajena.
pub fn gestor_paquetes_disponible(
    install: &ropero_core::EngineInstallSpec,
    env: &Environment,
) -> Option<&'static str> {
    if install.system_packages.contains_key("pacman") && which_en(env, "pacman").is_some() {
        return Some("pacman");
    }
    let declara_apt = install.system_packages.contains_key("apt-get")
        || install.system_packages.contains_key("apt");
    if declara_apt {
        if which_en(env, "apt-get").is_some() {
            return Some("apt-get");
        }
        if which_en(env, "apt").is_some() {
            return Some("apt");
        }
    }
    None
}

fn paquete_instalado(gestor: &str, paquete: &str, env: &Environment) -> bool {
    let (program, args): (&str, Vec<&str>) = match gestor {
        "pacman" => ("pacman", vec!["-Qq", paquete]),
        "apt-get" | "apt" => ("dpkg-query", vec!["-W", "-f=${db:Status-Status}", paquete]),
        _ => return false,
    };
    let Some(program) = which_en(env, program) else {
        return false;
    };
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .is_some_and(|output| {
            output.status.success()
                && (gestor == "pacman"
                    || String::from_utf8_lossy(&output.stdout).trim() == "installed")
        })
}

/// `flm validate` ve el nodo DRM, pero el motor abre la NPU a través de XRT.
/// Exige que el plugin esté cargable y que XRT enumere al menos una NPU antes
/// de gastar tiempo compilando el servidor.
pub fn valida_runtime_flm(env: &Environment) -> Result<(), String> {
    let xrt_smi = which_en(env, "xrt-smi")
        .or_else(|| crate::flm::xrt_root(&env.vars).map(|root| root.join("bin/xrt-smi")))
        .ok_or_else(|| "falta xrt-smi; instala XRT y xrt-plugin-amdxdna".to_string())?;
    let mut command = Command::new(xrt_smi);
    command.arg("examine");
    crate::flm::configure_xrt_command(&mut command, &env.vars);
    let output = command
        .output()
        .map_err(|e| format!("no se pudo ejecutar xrt-smi examine: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let salida = format!("{stdout}\n{stderr}");
    let tabla = salida
        .split_once("Device(s) Present")
        .map(|(_, dispositivos)| dispositivos)
        .unwrap_or_default();
    if !output.status.success()
        || tabla.contains("0 devices found")
        || !tabla.contains("NPU")
        || salida.contains("Failed to open library")
    {
        let sugerencia_arch = if which_en(env, "pacman").is_some() {
            " En Arch/CachyOS, comprueba que XRT y su plugin sean de la misma versión; `paru -S amdxdna-git` compila el stack AMD junto y evita mezclar versiones de repositorios distintos."
        } else {
            " Comprueba que el paquete XRT y el plugin XDNA procedan de la misma versión/repositorio; en Ubuntu, la guía de FastFlowLM requiere el PPA oficial de Lemonade."
        };
        return Err(format!(
            "XRT no puede usar la NPU: `xrt-smi examine` no enumera un dispositivo. Comprueba driver, firmware y que XRT y su plugin XDNA sean compatibles; vuelve a ejecutar `xrt-smi examine`. FLM no puede quedar listo hasta que XRT detecte la NPU.{sugerencia_arch}\n{}",
            salida.trim()
        ));
    }
    Ok(())
}

fn valida_ruta_fuente_segura(home: &Path, source_dir: &Path) -> Result<(), String> {
    let root = home.join(".local/share/ropero/engines");
    let relative = source_dir.strip_prefix(home).map_err(|_| {
        format!(
            "la ruta fuente queda fuera de HOME: {}",
            source_dir.display()
        )
    })?;
    let mut current = home.to_path_buf();
    for component in relative.components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(format!(
                "ruta fuente no normalizada: {}",
                source_dir.display()
            ));
        }
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "se rechaza la ruta fuente porque contiene un enlace simbólico: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
            Err(e) => {
                return Err(format!(
                    "no se pudo inspeccionar {}: {e}",
                    current.display()
                ));
            }
        }
    }
    if !source_dir.starts_with(&root) {
        return Err(format!("la ruta fuente debe estar bajo {}", root.display()));
    }
    Ok(())
}

fn valida_compiladores(manifest: &EngineManifest, env: &Environment) -> Result<(), String> {
    let os = env.get("ROPERO_BUILD_OS").unwrap_or(std::env::consts::OS);
    let requeridos: &[&str] = match os {
        "macos" | "darwin" => &["clang", "clang++"],
        "linux" => &[], // C/C++ se validan con las rutas host seleccionadas abajo.
        other => {
            return Err(format!(
                "no hay receta de compilación de {} para {other}",
                manifest.id.as_str()
            ));
        }
    };
    for nombre in requeridos {
        if which_en(env, nombre).is_none() {
            return Err(format!(
                "falta compilador requerido para la receta: {nombre}"
            ));
        }
    }
    if os == "linux" {
        let host_cxx = env
            .get("ROPERO_HOST_CXX")
            .map(PathBuf::from)
            .or_else(|| {
                ["/usr/bin/g++-14", "/usr/bin/g++-13", "/usr/bin/clang++"]
                    .iter()
                    .map(PathBuf::from)
                    .find(|p| p.is_file())
            })
            .ok_or_else(|| "falta compilador C++ host para Linux".to_string())?;
        if !host_cxx.is_file() {
            return Err(format!(
                "no se encontró compilador C++ host en {}",
                host_cxx.display()
            ));
        }
        let host_cc = host_cxx
            .to_string_lossy()
            .replace("g++", "gcc")
            .replace("clang++", "clang");
        if !Path::new(&host_cc).is_file() {
            return Err(format!("no se encontró compilador C host en {host_cc}"));
        }
    }
    Ok(())
}

/// Si una ruta es un enlace simbólico.
pub fn es_enlace(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

/// Muestra un menú y lee la elección del usuario.
///
/// El menú lo compone el dominio ([`ropero_core::install`]); aquí sólo se
/// enseña y se lee. La respuesta se resuelve con la misma regla del
/// dominio: **números** del menú o **nombres**.
pub fn pregunta_menu(lineas: &[String], opciones: &[String]) -> Result<Vec<String>, String> {
    for linea in lineas {
        ropero_core::out_line(linea);
    }
    ropero_core::out_line("");
    ropero_core::out_line("elige qué instalar (números o ids, separados por espacios o comas):");
    let respuesta = pregunta("> ").ok_or_else(|| "no hay respuesta: se cancela".to_string())?;
    ropero_core::install::resuelve_seleccion(&respuesta, opciones).map_err(|e| e.to_string())
}

/// Pregunta qué vía se usa para un modelo.
pub fn pregunta_via(texto: &str) -> Result<ropero_core::install::Via, String> {
    let respuesta = pregunta(texto).ok_or_else(|| "no hay respuesta: se cancela".to_string())?;
    ropero_core::install::Via::de_respuesta(&respuesta)
        .ok_or_else(|| "respuesta no válida: escribe 'd' (download) o 'b' (build)".to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        MODELO_BASALT, colibri_asset, compila_motor, cpu_avx2_available, cuda13_runtime_disponible,
        directorio_staging_basalt, enlaza_ejecutables, escribe_wrapper_flm,
        gestor_paquetes_disponible, prepara_dependencias_sistema, registra_modelo_basalt,
        valida_hardware_basalt, valida_runtime_flm, version_tuple, wrapper_flm_propio,
    };
    use ropero_core::{
        DetectSpec, EngineColor, EngineId, EngineInstallSpec, EngineManifest, Environment,
        Maturity, PortRange,
    };
    use std::collections::BTreeMap;

    fn executable(path: &std::path::Path, script: &str) {
        std::fs::write(path, script).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(path, permissions).unwrap();
        }
    }

    fn test_dir(name: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "{name}-{}-{timestamp}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn colibri_selecciona_archivos_oficiales_por_plataforma() {
        assert_eq!(
            colibri_asset("linux", "x86_64").unwrap(),
            "colibri-{version}-linux-x86_64.tar.gz"
        );
        assert_eq!(
            colibri_asset("macos", "aarch64").unwrap(),
            "colibri-{version}-macos-arm64.tar.gz"
        );
        assert!(colibri_asset("linux", "aarch64").is_err());
    }

    #[test]
    fn registra_toml_basalt_en_catalogo_de_usuario_sin_pisar_existente() {
        let base = test_dir("ropero-basalt-model");
        let env = Environment::new(BTreeMap::new(), base.join("home"), &base);
        let path = base.join("home/.config/ropero/models/basalt/qwen3.8-flash-next-basalt.toml");

        assert!(registra_modelo_basalt(&env).unwrap());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), MODELO_BASALT);

        std::fs::write(&path, "# configuración personalizada\n").unwrap();
        assert!(!registra_modelo_basalt(&env).unwrap());
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# configuración personalizada\n"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn basalt_instala_desde_staging_en_el_directorio_del_destino() {
        let base = test_dir("ropero-basalt-staging");
        let root = base.join("home/.local/share/ropero/engines/basalt");
        let staging = directorio_staging_basalt(&root).unwrap();
        assert_eq!(staging.parent(), root.parent());

        let release = staging.join("unpacked/basalt-v0.4.1");
        std::fs::create_dir_all(release.join("bin")).unwrap();
        std::fs::write(release.join("bin/basalt"), "test executable").unwrap();
        std::fs::rename(&release, &root).unwrap();
        assert!(root.join("bin/basalt").is_file());

        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn flm_preflight_exige_que_xrt_enumere_una_npu() {
        let base = test_dir("ropero-flm-xrt");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), base.display().to_string());
        let env = Environment::new(vars, &base, &base);
        executable(
            &base.join("xrt-smi"),
            "#!/bin/sh\necho 'Device(s) Present'\necho '|[0000:66:00.1]|NPU Strix|'\n",
        );
        let valid = valida_runtime_flm(&env);
        assert!(valid.is_ok(), "{valid:?}");

        executable(
            &base.join("xrt-smi"),
            "#!/bin/sh\necho 'Device(s) Present'\necho '0 devices found'\n",
        );
        let error = valida_runtime_flm(&env).unwrap_err();
        assert!(error.contains("XRT no puede usar la NPU"), "{error}");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn dependencias_sistema_no_piden_privilegios_si_ya_estan_instaladas() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let marca_tiempo = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let candidata = std::env::temp_dir().join(format!(
                "ropero-pkg-ok-{}-{marca_tiempo}-{id}",
                std::process::id(),
            ));
            match std::fs::create_dir(&candidata) {
                Ok(()) => break candidata,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("crear el temporal {}: {error}", candidata.display()),
            }
        };
        executable(&base.join("pacman"), "#!/bin/sh\nexit 0\n");
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), base.display().to_string());
        let env = Environment::new(vars, &base, &base);
        let install = EngineInstallSpec {
            system_packages: BTreeMap::from([(
                "pacman".to_string(),
                vec!["xrt".to_string(), "boost".to_string()],
            )]),
            ..EngineInstallSpec::default()
        };
        assert_eq!(prepara_dependencias_sistema(&install, &env), Ok(()));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn apt_sin_apt_get_cuenta_como_gestor_debian() {
        let base = test_dir("ropero-apt-only");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        executable(&base.join("apt"), "#!/bin/sh\nexit 0\n");
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), base.display().to_string());
        let env = Environment::new(vars, &base, &base);
        let install = EngineInstallSpec {
            system_packages: BTreeMap::from([("apt-get".to_string(), vec!["cmake".to_string()])]),
            ..EngineInstallSpec::default()
        };
        assert_eq!(gestor_paquetes_disponible(&install, &env), Some("apt"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn basalt_exige_blackwell_y_driver_580_95_05() {
        let base = test_dir("ropero-basalt-preflight");
        let _ = std::fs::remove_dir_all(&base);
        let libs = base.join("cuda/lib64");
        std::fs::create_dir_all(&libs).unwrap();
        std::fs::write(libs.join("libcudart.so.13"), "").unwrap();
        std::fs::write(libs.join("libcublas.so.13"), "").unwrap();
        executable(
            &base.join("nvidia-smi"),
            "#!/bin/sh\nprintf '12.0, 590.48.01\\n'\n",
        );
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), base.display().to_string());
        vars.insert("LD_LIBRARY_PATH".to_string(), libs.display().to_string());
        let env = Environment::new(vars, &base, &base);
        if cpu_avx2_available() {
            let valid = valida_hardware_basalt(&env);
            assert!(valid.is_ok(), "{valid:?}");
            executable(
                &base.join("nvidia-smi"),
                "#!/bin/sh\nprintf '8.6, 590.48.01\\n'\n",
            );
            let error = valida_hardware_basalt(&env).unwrap_err();
            assert!(error.contains("compute capability 12.0"));
        }
        assert_eq!(version_tuple("580.95.05"), Some((580, 95, 5)));
        assert!(version_tuple("580.94.99").unwrap() < (580, 95, 5));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn cuda13_en_el_venv_de_basalt_cuenta_como_runtime_disponible() {
        let base = test_dir("ropero-basalt-cuda");
        let libs = base.join(".venv/lib/python3.12/site-packages/nvidia");
        let runtime = libs.join("cuda_runtime/lib");
        let cublas = libs.join("cublas/lib");
        std::fs::create_dir_all(&runtime).unwrap();
        std::fs::create_dir_all(&cublas).unwrap();
        std::fs::write(runtime.join("libcudart.so.13"), "").unwrap();
        std::fs::write(cublas.join("libcublas.so.13"), "").unwrap();
        let env = Environment::new(BTreeMap::new(), &base, &base);
        assert!(cuda13_runtime_disponible(&env, &base));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn dependencias_sistema_informa_si_hacen_falta_privilegios_y_terminal() {
        let base = test_dir("ropero-pkg-missing");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        executable(&base.join("pacman"), "#!/bin/sh\nexit 1\n");
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), base.display().to_string());
        let env = Environment::new(vars, &base, &base);
        let install = EngineInstallSpec {
            system_packages: BTreeMap::from([("pacman".to_string(), vec!["xrt".to_string()])]),
            ..EngineInstallSpec::default()
        };
        let error = prepara_dependencias_sistema(&install, &env).unwrap_err();
        assert!(error.contains("xrt"));
        assert!(error.contains("terminal"));
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn el_lanzador_flm_es_reconocible_y_prepara_xrt_para_la_shell() {
        let base = test_dir("ropero-flm-wrapper");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let binary = base.join("build/install/bin/flm");
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        let wrapper = base.join("bin/flm");
        std::fs::create_dir_all(wrapper.parent().unwrap()).unwrap();
        escribe_wrapper_flm(&wrapper, &binary).unwrap();
        assert!(wrapper_flm_propio(&wrapper, &binary));
        let text = std::fs::read_to_string(&wrapper).unwrap();
        assert!(text.contains("LD_LIBRARY_PATH=\"$XILINX_XRT/lib"));
        assert!(text.contains("ROPERO_MANAGED_FASTFLOWLLM"));
        assert!(!wrapper_flm_propio(&wrapper, &base.join("otro/flm")));
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn receta_manifestada_clona_configura_compila_y_resuelve_binario() {
        let base = test_dir("ropero-build-engine");
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("path");
        let home = base.join("home");
        let source = home.join(".local/share/ropero/engines/llama-server");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        executable(
            &path.join("git"),
            "#!/bin/sh\nfor last do :; done\nmkdir -p \"$last/requirements\"\nprintf '# project' > \"$last/CMakeLists.txt\"\nprintf '# converter deps' > \"$last/requirements/requirements-convert_hf_to_gguf.txt\"\n",
        );
        executable(
            &path.join("python3"),
            "#!/bin/sh\nif [ \"$1\" = -m ] && [ \"$2\" = venv ]; then mkdir -p \"$3/bin\"; cp \"$0\" \"$3/bin/python\"; fi\nexit 0\n",
        );
        executable(
            &path.join("cmake"),
            "#!/bin/sh\nif [ \"$1\" = \"-S\" ]; then\n  while [ $# -gt 0 ]; do case \"$1\" in -B) shift; build=$1;; esac; shift; done\n  mkdir -p \"$build/bin\"\nelif [ \"$1\" = \"--build\" ]; then\n  build=$2\n  mkdir -p \"$build/bin\"\n  printf '#!/bin/sh\\nexit 0\\n' > \"$build/bin/llama-server\"\n  chmod +x \"$build/bin/llama-server\"\nelse\n  build=$2\n  while [ $# -gt 0 ]; do case \"$1\" in --prefix) shift; prefix=$1;; esac; shift; done\n  mkdir -p \"$prefix/bin\"\n  cp \"$build/bin/llama-server\" \"$prefix/bin/llama-server\"\nfi\n",
        );
        executable(&path.join("ninja"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("clang"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("clang++"), "#!/bin/sh\nexit 0\n");

        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), path.display().to_string());
        vars.insert("ROPERO_BUILD_OS".to_string(), "macos".to_string());
        let env = Environment::new(vars, &home, &base);
        let manifest = EngineManifest {
            id: EngineId::LlamaServer,
            label: "llama-server".to_string(),
            color: EngineColor::default(),
            description: String::new(),
            maturity: Maturity::default(),
            formats: vec!["gguf".to_string()],
            port_range: PortRange {
                min: 8100,
                max: 8199,
            },
            deps: vec!["git".to_string(), "cmake".to_string(), "ninja".to_string()],
            detect: DetectSpec::default(),
            serve: None,
            install: Some(EngineInstallSpec {
                source: Some("https://example.invalid/repo.git".to_string()),
                source_dir: Some("~/.local/share/ropero/engines/llama-server".to_string()),
                target: Some("llama-server".to_string()),
                cmake_args: Vec::new(),
                binary: Some("bin/llama-server".to_string()),
                ..EngineInstallSpec::default()
            }),
            compat: Vec::new(),
        };

        let (binary, compiled_source) =
            compila_motor(&manifest, manifest.install.as_ref().unwrap(), &env, false).unwrap();
        assert_eq!(binary, source.join("build-ropero/install/bin/llama-server"));
        assert_eq!(compiled_source.as_deref(), Some(source.as_path()));
        assert!(binary.is_file());
        assert!(source.join(".venv-ropero/bin/python").is_file());
        let _ = std::fs::remove_dir_all(base);
    }

    fn turbo_manifest() -> EngineManifest {
        EngineManifest {
            id: EngineId::TurboLlama,
            label: "turbo-llama".to_string(),
            color: EngineColor::default(),
            description: String::new(),
            maturity: Maturity::default(),
            formats: vec!["gguf".to_string()],
            port_range: PortRange {
                min: 8200,
                max: 8299,
            },
            deps: Vec::new(),
            detect: DetectSpec::default(),
            serve: None,
            install: None,
            compat: Vec::new(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn turbo_enlaza_todos_los_bins_prefiere_install_y_poda_links_propios() {
        use std::os::unix::fs::symlink;

        let base = test_dir("ropero-link-turbo");
        let _ = std::fs::remove_dir_all(&base);
        let source = base.join("source");
        let build_bin = source.join("build-ropero/bin");
        let install_bin = source.join("build-ropero/install/bin");
        let local_bin = base.join("local-bin");
        std::fs::create_dir_all(&build_bin).unwrap();
        std::fs::create_dir_all(&install_bin).unwrap();
        std::fs::create_dir_all(&local_bin).unwrap();
        executable(&build_bin.join("llama-server"), "#!/bin/sh\nexit 0\n");
        executable(&build_bin.join("llama-cli"), "#!/bin/sh\nexit 0\n");
        executable(&install_bin.join("llama-server"), "#!/bin/sh\nexit 0\n");
        executable(&install_bin.join("llama-quantize"), "#!/bin/sh\nexit 0\n");
        let stale = source.join("old-build/bin/old-tool");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        executable(&stale, "#!/bin/sh\nexit 0\n");
        symlink(&stale, local_bin.join("turbo-old-tool")).unwrap();
        let principal = install_bin.join("llama-server");

        let estado = enlaza_ejecutables(
            &turbo_manifest(),
            &principal,
            Some(&source),
            true,
            &local_bin,
        )
        .unwrap();
        let (mensaje, codigo) = ropero_core::install::mensaje_motor("turbo-llama", &estado);
        assert_eq!(codigo, 0);
        assert!(mensaje.contains("actualizada"), "{mensaje}");
        assert!(mensaje.contains("regenerados"), "{mensaje}");
        assert!(!mensaje.contains("no se toca nada"), "{mensaje}");

        assert_eq!(
            std::fs::read_link(local_bin.join("turbo-llama-server")).unwrap(),
            principal
        );
        assert!(local_bin.join("turbo-llama-cli").is_symlink());
        assert!(local_bin.join("turbo-llama-quantize").is_symlink());
        assert!(!local_bin.join("turbo-old-tool").exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn no_pisa_symlink_ajeno_aunque_destino_tenga_el_mismo_nombre() {
        use std::os::unix::fs::symlink;

        let base = test_dir("ropero-link-owned");
        let _ = std::fs::remove_dir_all(&base);
        let source = base.join("source");
        let build_bin = source.join("build-ropero/bin");
        let install_bin = source.join("build-ropero/install/bin");
        let local_bin = base.join("local-bin");
        let foreign = base.join("foreign/llama-cli");
        std::fs::create_dir_all(&build_bin).unwrap();
        std::fs::create_dir_all(&install_bin).unwrap();
        std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        std::fs::create_dir_all(&local_bin).unwrap();
        executable(&install_bin.join("llama-server"), "#!/bin/sh\nexit 0\n");
        executable(&install_bin.join("llama-cli"), "#!/bin/sh\nexit 0\n");
        executable(&foreign, "#!/bin/sh\nexit 0\n");
        let foreign_link = local_bin.join("turbo-llama-cli");
        symlink(&foreign, &foreign_link).unwrap();

        let error = enlaza_ejecutables(
            &turbo_manifest(),
            &install_bin.join("llama-server"),
            Some(&source),
            true,
            &local_bin,
        )
        .unwrap_err();
        assert!(error.contains("enlace ajeno"), "{error}");
        assert_eq!(std::fs::read_link(&foreign_link).unwrap(), foreign);
        assert!(!local_bin.join("turbo-llama-server").exists());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn force_falla_preflight_cuda_sin_borrar_el_checkout() {
        let base = test_dir("ropero-force-preflight");
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("path");
        let home = base.join("home");
        let source = home.join(".local/share/ropero/engines/llama-server");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("CMakeLists.txt"), "# existing checkout").unwrap();
        std::fs::write(source.join("keep.marker"), "keep").unwrap();
        executable(&path.join("git"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("cmake"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("ninja"), "#!/bin/sh\nexit 0\n");
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), path.display().to_string());
        vars.insert("ROPERO_BUILD_OS".to_string(), "linux".to_string());
        vars.insert(
            "ROPERO_CUDA_PATH".to_string(),
            base.join("cuda-no-existe").display().to_string(),
        );
        let env = Environment::new(vars, &home, &base);
        let manifest = llama_manifest("~/.local/share/ropero/engines/llama-server");

        let error =
            compila_motor(&manifest, manifest.install.as_ref().unwrap(), &env, true).unwrap_err();
        assert!(error.contains("nvcc"), "{error}");
        assert!(source.join("keep.marker").is_file());
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn force_rechaza_source_dir_fuera_de_raiz_sin_borrarlo() {
        let base = test_dir("ropero-force-root");
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("path");
        let home = base.join("home");
        let outside = home.join("source");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("keep.marker"), "keep").unwrap();
        executable(&path.join("git"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("cmake"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("ninja"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("clang"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("clang++"), "#!/bin/sh\nexit 0\n");
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), path.display().to_string());
        vars.insert("ROPERO_BUILD_OS".to_string(), "macos".to_string());
        let env = Environment::new(vars, &home, &base);
        let manifest = llama_manifest("~/source");

        let error =
            compila_motor(&manifest, manifest.install.as_ref().unwrap(), &env, true).unwrap_err();
        assert!(error.contains("ruta fuente no permitida"), "{error}");
        assert!(outside.join("keep.marker").is_file());
        let _ = std::fs::remove_dir_all(base);
    }

    #[cfg(unix)]
    #[test]
    fn force_rechaza_source_dir_symlink_a_fuera_de_raiz() {
        use std::os::unix::fs::symlink;

        let base = test_dir("ropero-force-symlink");
        let _ = std::fs::remove_dir_all(&base);
        let path = base.join("path");
        let home = base.join("home");
        let outside = home.join("outside");
        let expected = home.join(".local/share/ropero/engines/llama-server");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::create_dir_all(expected.parent().unwrap()).unwrap();
        std::fs::write(outside.join("keep.marker"), "keep").unwrap();
        symlink(&outside, &expected).unwrap();
        executable(&path.join("git"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("cmake"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("ninja"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("clang"), "#!/bin/sh\nexit 0\n");
        executable(&path.join("clang++"), "#!/bin/sh\nexit 0\n");
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), path.display().to_string());
        vars.insert("ROPERO_BUILD_OS".to_string(), "macos".to_string());
        let env = Environment::new(vars, &home, &base);
        let manifest = llama_manifest("~/.local/share/ropero/engines/llama-server");

        let error =
            compila_motor(&manifest, manifest.install.as_ref().unwrap(), &env, true).unwrap_err();
        assert!(error.contains("enlace simbólico"), "{error}");
        assert!(outside.join("keep.marker").is_file());
        assert!(expected.is_symlink());
        let _ = std::fs::remove_dir_all(base);
    }

    fn llama_manifest(source_dir: &str) -> EngineManifest {
        EngineManifest {
            id: EngineId::LlamaServer,
            label: "llama-server".to_string(),
            color: EngineColor::default(),
            description: String::new(),
            maturity: Maturity::default(),
            formats: vec!["gguf".to_string()],
            port_range: PortRange {
                min: 8100,
                max: 8199,
            },
            deps: vec!["git".to_string(), "cmake".to_string(), "ninja".to_string()],
            detect: DetectSpec::default(),
            serve: None,
            install: Some(EngineInstallSpec {
                source: Some("https://example.invalid/repo.git".to_string()),
                source_dir: Some(source_dir.to_string()),
                binary: Some("bin/llama-server".to_string()),
                ..EngineInstallSpec::default()
            }),
            compat: Vec::new(),
        }
    }
}
