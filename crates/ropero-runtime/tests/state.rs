//! Tests del estado en disco y de la identidad de proceso (doc B §3.3.3-3.3.4).
//!
//! El estado en disco es el **contrato compartido** con la v1 (ADR-002): el
//! formato `CLAVE=valor` no se cambia, y las claves nuevas son adiciones
//! compatibles.

use std::path::{Path, PathBuf};

use ropero_runtime::process::ProcIdentity;
use ropero_runtime::state::{
    PortState, clear_state, managed_ports, read_state, stamp_de, state_dir, state_path, write_state,
};

/// Un directorio temporal que se borra al soltarlo.
struct TempDir(PathBuf);

impl TempDir {
    fn nuevo() -> Self {
        let base = std::env::temp_dir().join(format!(
            "ropero-state-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&base).expect("crear el temporal");
        Self(base)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Un estado de prueba.
fn estado() -> PortState {
    PortState {
        model: "groot".to_string(),
        engine: "llama-server".to_string(),
        pid: Some(42137),
        port: Some(9999),
        level: "high".to_string(),
        ts: Some(1_762_362_312),
        log: "/home/usuario/.local/state/ropero/logs/9999.log".to_string(),
        stamp: "a1b2c3d4".to_string(),
        start_time: Some(812_345_678),
        exe_digest: Some("3f2a9c".to_string()),
    }
}

#[test]
fn el_estado_tiene_las_ocho_claves_de_la_v1() {
    let lineas = estado().to_lines();
    let claves: Vec<&str> = lineas
        .iter()
        .map(|l| l.split_once('=').map(|(c, _)| c).unwrap_or(""))
        .collect();
    for clave in [
        "MODEL", "ENGINE", "PID", "PORT", "LEVEL", "TS", "LOG", "STAMP",
    ] {
        assert!(claves.contains(&clave), "falta {clave} en {claves:?}");
    }
}

#[test]
fn el_estado_anade_las_tres_claves_nuevas() {
    let lineas = estado().to_lines();
    let texto = lineas.join("\n");
    assert!(texto.contains("STARTTIME=812345678"), "estado={texto}");
    assert!(texto.contains("EXEDIGEST=3f2a9c"), "estado={texto}");
    assert!(texto.contains("VERSION=2"), "estado={texto}");
}

#[test]
fn el_estado_se_lee_y_se_escribe() {
    let dir = TempDir::nuevo();
    write_state(dir.path(), 9999, &estado()).expect("escribir el estado");
    let leido = read_state(dir.path(), 9999).expect("leer el estado");
    assert_eq!(leido, estado());
}

#[test]
fn el_estado_se_escribe_en_ports_menor_puerto() {
    let dir = TempDir::nuevo();
    write_state(dir.path(), 9999, &estado()).expect("escribir el estado");
    // El nombre del fichero es el número del puerto, sin extensión: es el
    // nombre que la v1 usa.
    assert!(
        dir.path().join("ports").join("9999").exists(),
        "el fichero del estado tiene que estar en ports/9999"
    );
}

#[test]
fn la_escritura_conserva_las_claves_que_no_vienen() {
    let dir = TempDir::nuevo();
    write_state(dir.path(), 9999, &estado()).expect("escribir el estado");

    // Se escribe un estado con el PID cambiado: el resto se conserva.
    let mut parcial = PortState {
        model: String::new(),
        engine: String::new(),
        pid: Some(999),
        port: None,
        level: String::new(),
        ts: None,
        log: String::new(),
        stamp: String::new(),
        start_time: None,
        exe_digest: None,
    };
    parcial.model.clear();
    write_state(dir.path(), 9999, &parcial).expect("escribir el estado parcial");

    let leido = read_state(dir.path(), 9999).expect("leer el estado");
    // El PID se ha actualizado…
    assert_eq!(leido.pid, Some(999));
    // …y el resto sigue ahí.
    assert_eq!(leido.model, "groot");
    assert_eq!(leido.engine, "llama-server");
    assert_eq!(leido.level, "high");
    assert_eq!(leido.stamp, "a1b2c3d4");
}

#[test]
fn un_estado_de_la_v1_se_lee_sin_las_claves_nuevas() {
    // Un fichero escrito por la v1: no tiene STARTTIME, ni EXEDIGEST, ni
    // VERSION. Se lee igual (y sin quejarse).
    let dir = TempDir::nuevo();
    let ruta = state_path(dir.path(), 9999);
    std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
    std::fs::write(
        &ruta,
        "MODEL=groot\nENGINE=llama-server\nPID=42137\nPORT=9999\nLEVEL=high\nTS=1762362312\nLOG=/tmp/g.log\nSTAMP=abc\n",
    )
    .unwrap();

    let leido = read_state(dir.path(), 9999).expect("leer el estado de la v1");
    assert_eq!(leido.model, "groot");
    assert_eq!(leido.pid, Some(42137));
    assert_eq!(leido.stamp, "abc");
    // Las claves nuevas no están: se quedan vacías, no se inventan.
    assert_eq!(leido.start_time, None);
    assert_eq!(leido.exe_digest, None);
}

#[test]
fn el_estado_se_borra() {
    let dir = TempDir::nuevo();
    write_state(dir.path(), 9999, &estado()).expect("escribir el estado");
    clear_state(dir.path(), 9999);
    assert!(read_state(dir.path(), 9999).is_none());
}

#[test]
fn los_puertos_gestionados_se_listan_ordenados() {
    let dir = TempDir::nuevo();
    for port in [9999u16, 8100, 8200] {
        let mut e = estado();
        e.port = Some(port);
        write_state(dir.path(), port, &e).expect("escribir el estado");
    }
    assert_eq!(managed_ports(dir.path()), vec![8100, 8200, 9999]);
}

#[test]
fn el_directorio_de_estado_por_defecto_es_el_de_local_state() {
    let dir = state_dir(Path::new("/home/usuario"), None);
    assert_eq!(
        dir,
        PathBuf::from("/home/usuario/.local/state/ropero"),
        "el directorio de estado por defecto"
    );
}

#[test]
fn la_variable_de_entorno_manda_en_el_directorio_de_estado() {
    let dir = state_dir(Path::new("/home/usuario"), Some("/tmp/otro-estado"));
    assert_eq!(dir, PathBuf::from("/tmp/otro-estado"));
}

#[test]
fn la_escritura_es_atomica_no_deja_ficheros_temporales() {
    let dir = TempDir::nuevo();
    write_state(dir.path(), 9999, &estado()).expect("escribir el estado");

    // La escritura atómica es fichero temporal + `rename`: si queda un
    // temporal, el estado se escribió **encima** del bueno, y un lector
    // concurrente pudo leer un fichero a medias.
    let entradas: Vec<String> = std::fs::read_dir(dir.path().join("ports"))
        .expect("leer el directorio de puertos")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        entradas,
        vec!["9999".to_string()],
        "no puede quedar ningún fichero temporal: escritura atómica"
    );
}

#[test]
fn la_escritura_reemplaza_el_fichero_y_no_lo_trunca() {
    // La prueba de que la escritura es atómica es **observar el
    // fichero durante la escritura**: con `tmp` + `rename`, el fichero
    // destino **cambia de golpe** (de inode), nunca se trunca en sitio.
    // Un lector concurrente puede ver el estado viejo entero o el
    // nuevo entero, pero nunca uno a medias.
    //
    // Se mide el inode: `rename` instala un inode **nuevo** en el
    // destino. Si la escritura fuese "abrir y escribir encima", el
    // inode **no** cambiaría.
    let dir = TempDir::nuevo();
    write_state(dir.path(), 9999, &estado()).expect("escribir el estado");
    let ruta = state_path(dir.path(), 9999);

    #[cfg(unix)]
    let inode_antes = {
        use std::os::unix::fs::MetadataExt;
        std::fs::metadata(&ruta).expect("metadatos").ino()
    };

    // Se escribe un estado con el PID cambiado: es otro contenido.
    let mut otro = estado();
    otro.pid = Some(555_555);
    write_state(dir.path(), 9999, &otro).expect("escribir el estado");

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let inode_despues = std::fs::metadata(&ruta).expect("metadatos").ino();
        assert_ne!(
            inode_antes, inode_despues,
            "el destino tiene que ser un fichero NUEVO (rename), no el mismo truncado"
        );
    }

    // Y el contenido final está completo.
    let texto = std::fs::read_to_string(&ruta).expect("leer el estado");
    assert!(texto.contains("PID=555555"), "estado={texto}");
    assert!(texto.contains("MODEL=groot"), "estado={texto}");
    assert!(texto.contains("VERSION=2"), "estado={texto}");
}

#[test]
fn el_estado_se_escribe_de_todo_a_una() {
    let dir = TempDir::nuevo();
    write_state(dir.path(), 9999, &estado()).expect("escribir el estado");
    // El fichero existe y está **completo**: se leen todas las claves.
    let texto = std::fs::read_to_string(state_path(dir.path(), 9999)).expect("leer el estado");
    for clave in [
        "MODEL",
        "ENGINE",
        "PID",
        "PORT",
        "LEVEL",
        "TS",
        "LOG",
        "STAMP",
        "STARTTIME",
        "EXEDIGEST",
        "VERSION",
    ] {
        assert!(
            texto.lines().any(|l| l.starts_with(&format!("{clave}="))),
            "falta {clave} en {texto}"
        );
    }
}

#[test]
fn el_sello_es_estable_y_cambia_con_el_contenido() {
    assert_eq!(
        stamp_de(&["groot", "high", "32768"]),
        stamp_de(&["groot", "high", "32768"])
    );
    assert_ne!(
        stamp_de(&["groot", "high", "32768"]),
        stamp_de(&["groot", "high", "8192"])
    );
    assert_ne!(stamp_de(&["groot", "high"]), stamp_de(&["otro", "high"]));
    assert!(!stamp_de(&["groot"]).is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// La identidad de proceso
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn una_identidad_con_el_mismo_pid_y_start_time_es_la_misma() {
    let a = ProcIdentity::with_digest(100, 500, None);
    let b = ProcIdentity::with_digest(100, 500, None);
    assert!(a.es_la_misma(&b));
}

#[test]
fn un_pid_distinto_no_es_la_misma_identidad() {
    let a = ProcIdentity::with_digest(100, 500, None);
    let b = ProcIdentity::with_digest(101, 500, None);
    assert!(!a.es_la_misma(&b));
}

#[test]
fn un_start_time_distinto_no_es_la_misma_identidad() {
    // Es el caso del PID reciclado: el PID coincide, el proceso no.
    let a = ProcIdentity::with_digest(100, 500, None);
    let b = ProcIdentity::with_digest(100, 501, None);
    assert!(!a.es_la_misma(&b), "un PID reciclado no es nuestro");
}

#[test]
fn un_digest_distinto_no_es_la_misma_identidad() {
    let a = ProcIdentity::with_digest(100, 500, Some([1u8; 32]));
    let b = ProcIdentity::with_digest(100, 500, Some([2u8; 32]));
    assert!(!a.es_la_misma(&b));
}

#[test]
fn un_digest_que_falta_no_es_una_discrepancia() {
    // Falta el dato, no discrepa: el `start_time` es el que decide.
    let a = ProcIdentity::with_digest(100, 500, None);
    let b = ProcIdentity::with_digest(100, 500, Some([1u8; 32]));
    assert!(a.es_la_misma(&b));
}

#[test]
fn la_identidad_de_un_proceso_que_no_existe_es_none() {
    // El PID 0 no es un proceso: no hay identidad que inventar.
    let identidad = ProcIdentity::of(0);
    assert!(identidad.is_none(), "identidad={identidad:?}");
}

#[test]
fn la_identidad_de_este_proceso_se_puede_leer() {
    let pid = std::process::id();
    let identidad = ProcIdentity::of(pid).expect("este proceso existe");
    assert_eq!(identidad.pid, pid);
    assert!(
        identidad.start_time > 0,
        "start_time={}",
        identidad.start_time
    );
}
