//! La vista `ropero status` y el informe `ropero doctor`.
//!
//! Los dos se prueban con **datos inyectados**: el `status` con una foto de
//! la máquina, el `doctor` con un informe. Ninguno de los dos necesita la
//! máquina que ejecuta el test.

use ropero_core::doctor::{DepCheck, DepStatus, Dependency, DoctorReport, EngineDeps};
use ropero_core::status::{GpuState, MachineState, MemoryState, Reading, status_text};
use ropero_core::{EngineId, EngineManifest, Maturity, Palette};

// ─────────────────────────────────────────────────────────────────────────────
// El status
// ─────────────────────────────────────────────────────────────────────────────

/// Una medida.
fn medida(valor: f64, unit: &'static str) -> Option<Reading> {
    Some(Reading { value: valor, unit })
}

/// La máquina del caso: CPU con reloj y temperatura, una dGPU, RAM y swap
/// con zram.
fn maquina() -> MachineState {
    MachineState {
        cpu_ghz: medida(3.4, "GHz"),
        cpu_temp_c: medida(58.0, "°C"),
        dgpu: Some(GpuState {
            name: "NVIDIA GeForce RTX 5080".to_string(),
            ghz: medida(2.1, "GHz"),
            temp_c: medida(45.0, "°C"),
        }),
        igpu: None,
        memory: MemoryState {
            ram_used_mb: medida(8192.0, "MB"),
            ram_total_mb: medida(32768.0, "MB"),
            swap_used_mb: medida(512.0, "MB"),
            swap_total_mb: medida(8192.0, "MB"),
            zram_mb: medida(31186.0, "MB"),
        },
    }
}

/// El status muestra CPU, dGPU, RAM y swap.
#[test]
fn el_status_muestra_cpu_gpu_ram_y_swap() {
    let texto = status_text(&maquina());
    assert!(texto.contains("CPU"), "texto={texto}");
    assert!(texto.contains("GPU"), "texto={texto}");
    assert!(texto.contains("RAM"), "texto={texto}");
    assert!(texto.contains("SWAP"), "texto={texto}");
    assert!(texto.contains("3.40 GHz"), "texto={texto}");
    assert!(texto.contains("58.0 °C"), "texto={texto}");
    assert!(texto.contains("NVIDIA GeForce RTX 5080"), "texto={texto}");
}

/// La RAM sale **en uso de total**: el número primero es el usado.
///
/// El orden importa: "8192 de 32768" y "32768 de 8192" son las dos
/// cadenas posibles, y sólo una es la verdad.
#[test]
fn la_ram_sale_en_uso_de_total() {
    let texto = status_text(&maquina());
    let linea = texto
        .lines()
        .find(|l| l.contains("RAM"))
        .expect("debe salir la línea de la RAM");
    let usado = linea.find("8192.0").expect("debe salir el uso de la RAM");
    let total = linea
        .find("32768.0")
        .expect("debe salir el total de la RAM");
    assert!(usado < total, "la RAM no sale en uso de total: {linea}");
}

/// El swap también sale en uso de total.
#[test]
fn el_swap_sale_en_uso_de_total() {
    let mut m = maquina();
    m.memory.swap_used_mb = medida(1024.0, "MB");
    m.memory.zram_mb = None;
    let texto = status_text(&m);
    let linea = texto
        .lines()
        .find(|l| l.contains("SWAP"))
        .expect("debe salir la línea del swap");
    let usado = linea.find("1024.0").expect("debe salir el uso del swap");
    let total = linea.find("8192.0").expect("debe salir el total del swap");
    assert!(usado < total, "el swap no sale en uso de total: {linea}");
}

/// Lo que no se puede leer se dice, no se inventa.
#[test]
fn lo_que_no_existe_dice_no_disponible() {
    let texto = status_text(&MachineState::default());
    assert!(texto.contains("no disponible"), "texto={texto}");
    assert!(texto.contains("GPU  no disponible"), "texto={texto}");
}

/// El zram se enseña **aparte**: el swap lo incluye, pero no se confunde con
/// él (CachyOS).
#[test]
fn el_zram_se_enseña_aparte_del_swap() {
    let mut m = maquina();
    // El runtime entrega el swap **con** el zram dentro: 512 + 31186.
    m.memory.swap_used_mb = medida(31698.0, "MB");
    let texto = status_text(&m);
    assert!(texto.contains("zram"), "texto={texto}");
    assert!(texto.contains("31698.0/8192.0 MB"), "texto={texto}");
    // Y el zram se nombra con su tamaño, para que no se confunda.
    assert!(texto.contains("zram 31186.00 MB"), "texto={texto}");
}

/// Sin zram, el swap es el del kernel y no se habla de zram.
#[test]
fn sin_zram_no_se_mentiona_el_zram() {
    let mut m = maquina();
    m.memory.zram_mb = None;
    let texto = status_text(&m);
    assert!(!texto.contains("zram"), "texto={texto}");
    assert!(texto.contains("512.0/8192.0 MB"), "texto={texto}");
}

/// La iGPU sale cuando la hay (sólo Linux).
#[test]
fn la_igpu_sale_cuando_la_hay() {
    let mut m = maquina();
    m.igpu = Some(GpuState {
        name: "AMD Radeon 890M".to_string(),
        ghz: medida(1.2, "GHz"),
        temp_c: medida(48.0, "°C"),
    });
    let texto = status_text(&m);
    assert!(texto.contains("iGPU"), "texto={texto}");
    assert!(texto.contains("AMD Radeon 890M"), "texto={texto}");
}

/// La conversión de kB a MB.
#[test]
fn los_kb_se_convierten_en_mb() {
    assert_eq!(ropero_core::status::kb_a_mb(2048.0), 2.0);
    assert_eq!(ropero_core::status::bytes_a_mb(1024.0 * 1024.0), 1.0);
}

/// El formato compacto conserva recursos y lecturas sin una cabecera ancha.
#[test]
fn el_status_es_compacto_y_muestra_las_lecturas() {
    let texto = status_text(&maquina());
    assert!(
        texto.lines().count() <= 7,
        "status demasiado ancho: {texto}"
    );
    assert!(texto.contains("CPU  3.40 GHz · 58.0 °C"), "texto={texto}");
    assert!(
        texto.contains("RAM  8192.0/32768.0 MB (25%)"),
        "texto={texto}"
    );
    assert!(
        !texto.contains("recurso"),
        "no debe imprimir tabla: {texto}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// El doctor
// ─────────────────────────────────────────────────────────────────────────────

/// Un motor de prueba.
fn motor(id: EngineId) -> EngineManifest {
    EngineManifest {
        id,
        label: id.as_str().to_string(),
        color: ropero_core::EngineColor::Naranja,
        description: String::new(),
        maturity: Maturity::Integrated,
        formats: vec!["gguf".to_string()],
        port_range: ropero_core::PortRange::new(8100, 8199),
        deps: vec!["git".to_string()],
        detect: ropero_core::DetectSpec::default(),
        serve: None,
        install: None,
        compat: Vec::new(),
    }
}

/// Una dependencia satisfied.
fn ok(nombre: &str) -> DepCheck {
    DepCheck {
        dependency: Dependency {
            name: nombre.to_string(),
            remedy: "instálalo con tu gestor de paquetes".to_string(),
        },
        status: DepStatus::Ok,
    }
}

/// Una dependencia ausente.
fn falta(nombre: &str) -> DepCheck {
    DepCheck {
        dependency: Dependency {
            name: nombre.to_string(),
            remedy: "instálalo con `apt install git`".to_string(),
        },
        status: DepStatus::Missing,
    }
}

/// Cuando falta una dependencia, el doctor dice **qué hacer**.
#[test]
fn una_dependencia_ausente_dice_que_hacer() {
    let informe = DoctorReport {
        ropero: vec![ok("git"), falta("hf")],
        engines: vec![EngineDeps {
            manifest: motor(EngineId::LlamaServer),
            installed: true,
            checks: vec![falta("cmake")],
        }],
    };
    let texto = informe.lines(&Palette::plain()).join("\n");
    assert!(texto.contains("falta"), "texto={texto}");
    assert!(texto.contains("hf"), "texto={texto}");
    // Y el remedio: lo que hay que hacer.
    assert!(texto.contains("apt install git"), "texto={texto}");
    assert!(texto.contains("cmake"), "texto={texto}");
}

/// El doctor sabe si está todo bien.
#[test]
fn el_doctor_sabe_si_todo_esta_bien() {
    let bien = DoctorReport {
        ropero: vec![ok("git"), ok("hf")],
        engines: vec![EngineDeps {
            manifest: motor(EngineId::LlamaServer),
            installed: true,
            checks: vec![ok("cmake")],
        }],
    };
    assert!(bien.todo_ok());
    assert_eq!(bien.fallan(), 0);

    let mal = DoctorReport {
        ropero: vec![ok("git"), falta("hf")],
        engines: vec![EngineDeps {
            manifest: motor(EngineId::LlamaServer),
            installed: true,
            checks: vec![ok("cmake")],
        }],
    };
    assert!(!mal.todo_ok());
    assert_eq!(mal.fallan(), 1);
}

/// El doctor comprueba **todos** los motores, instalados o no (doc CLI v2
/// §14).
#[test]
fn el_doctor_comprueba_todos_los_motores() {
    let informe = DoctorReport {
        ropero: vec![ok("git")],
        engines: vec![
            EngineDeps {
                manifest: motor(EngineId::LlamaServer),
                installed: true,
                checks: vec![ok("git")],
            },
            EngineDeps {
                manifest: motor(EngineId::Vllm),
                installed: false,
                checks: vec![falta("uv")],
            },
        ],
    };
    let texto = informe.lines(&Palette::plain()).join("\n");
    assert!(texto.contains("llama-server"), "texto={texto}");
    assert!(texto.contains("vllm"), "texto={texto}");
    assert!(texto.contains("no instalado"), "texto={texto}");
    assert!(!informe.todo_ok());
}

/// El doctor dice primero lo de ropero, luego lo de los motores.
#[test]
fn el_doctor_empieza_por_las_dependencias_de_ropero() {
    let informe = DoctorReport {
        ropero: vec![ok("git")],
        engines: vec![EngineDeps {
            manifest: motor(EngineId::LlamaServer),
            installed: true,
            checks: vec![ok("git")],
        }],
    };
    let lineas = informe.lines(&Palette::plain());
    let ropero = lineas
        .iter()
        .position(|l| l.contains("DEPENDENCIAS DE ROPERO"))
        .expect("debe estar la sección de ropero");
    let motores = lineas
        .iter()
        .position(|l| l.contains("DEPENDENCIAS DE LOS MOTORES"))
        .expect("debe estar la sección de motores");
    assert!(ropero < motores, "lineas={lineas:?}");
}

/// Un motor sin dependencias lo dice.
#[test]
fn un_motor_sin_dependencias_lo_dice() {
    let mut manifiesto = motor(EngineId::LlamaServer);
    manifiesto.deps.clear();
    let informe = DoctorReport {
        ropero: vec![ok("git")],
        engines: vec![EngineDeps {
            manifest: manifiesto,
            installed: true,
            checks: vec![],
        }],
    };
    let texto = informe.lines(&Palette::plain()).join("\n");
    assert!(texto.contains("sin dependencias"), "texto={texto}");
}

/// La tabla de los motores tiene **una sola cabecera**: el motor es una fila
/// de grupo y una regla lo separa del siguiente.
///
/// Antes cada motor repetía la cabecera y su regla: la salida se veía
/// recargada y no se leía como una tabla (doc CLI v2 §2).
#[test]
fn el_doctor_no_repite_la_cabecera_por_motor() {
    let informe = DoctorReport {
        ropero: vec![ok("git")],
        engines: vec![
            EngineDeps {
                manifest: motor(EngineId::LlamaServer),
                installed: true,
                checks: vec![ok("cmake")],
            },
            EngineDeps {
                manifest: motor(EngineId::Vllm),
                installed: false,
                checks: vec![falta("uv")],
            },
        ],
    };
    let lineas = informe.lines(&Palette::plain());
    // Una cabecera por sección: la de ropero y la de los motores.
    let cabeceras: Vec<&String> = lineas
        .iter()
        .filter(|l| l.contains("dependencia"))
        .collect();
    assert_eq!(cabeceras.len(), 2, "lineas={lineas:#?}");
    // Una regla por sección y una entre motores.
    let reglas: Vec<&String> = lineas
        .iter()
        .filter(|l| l.trim_start().starts_with('─'))
        .collect();
    assert_eq!(reglas.len(), 3, "lineas={lineas:#?}");
    // Los motores salen en su propia línea: son filas de grupo.
    for motor in ["llama-server", "vllm"] {
        let fila = lineas
            .iter()
            .find(|l| l.contains(motor))
            .expect("el motor sale en el informe");
        assert!(fila.contains(motor), "fila={fila}");
        assert!(
            !fila.contains("dependencia"),
            "el motor no es una cabecera: {fila}"
        );
    }
    // Y las dependencias de cada motor salen debajo de su nombre.
    let cmake = lineas
        .iter()
        .position(|l| l.contains("cmake"))
        .expect("cmake sale en el informe");
    let llama = lineas
        .iter()
        .position(|l| l.contains("llama-server"))
        .expect("llama-server sale en el informe");
    assert!(llama < cmake, "lineas={lineas:#?}");
}
