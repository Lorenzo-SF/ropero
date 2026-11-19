//! La inferencia contra un motor **falso**: el `POST`, las métricas y lo que
//! pasa cuando el motor no dice todo lo que debería.
//!
//! El motor falso es un `TcpListener` que responde con lo que se le pide:
//! así se prueba el cliente HTTP sin encender un motor de verdad.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::Duration;

use ropero_bateria::informe::Tokens;
use ropero_runtime::inferencia::{motor_de, pregunta};

/// Un motor falso que responde con el cuerpo dado.
///
/// Escucha en un puerto libre y contesta a la primera petición. Devuelve el
/// puerto.
fn motor_falso(cuerpo: String, espera: Duration) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("se puede escuchar");
    let puerto = listener.local_addr().expect("dirección local").port();
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        // Se lee la petición entera (cabeceras + cuerpo) para que el
        // cliente no vea el corte.
        let mut buffer = [0u8; 8192];
        let mut leido = 0usize;
        let mut cabecera = String::new();
        while let Ok(n) = stream.read(&mut buffer) {
            if n == 0 {
                break;
            }
            leido += n;
            cabecera.push_str(&String::from_utf8_lossy(&buffer[..n]));
            if let Some((cabeza, _)) = cabecera.split_once("\r\n\r\n") {
                let largo = cabeza
                    .lines()
                    .find_map(|l| {
                        let (nombre, valor) = l.split_once(':')?;
                        if nombre.trim().eq_ignore_ascii_case("content-length") {
                            valor.trim().parse::<usize>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(0);
                if cabecera.len() >= cabeza.len() + 4 + largo {
                    break;
                }
            }
        }
        let _ = leido;
        // La primera mitad va enseguida; la segunda, un poco después: así
        // el primer token se ve antes que el final.
        let (primera, segunda) = match cuerpo.split_once("\u{1}GUION\u{1}") {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (cuerpo.clone(), String::new()),
        };
        let _ = stream.write_all(primera.as_bytes());
        let _ = stream.flush();
        if !segunda.is_empty() {
            std::thread::sleep(espera);
            let _ = stream.write_all(segunda.as_bytes());
        }
        let _ = stream.flush();
    });
    puerto
}

/// La cabecera y el cuerpo de una respuesta.
fn respuesta(cuerpo: &str) -> String {
    format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{cuerpo}")
}

#[test]
fn el_streaming_se_arma_con_los_deltas_y_dice_los_tokens_del_motor() {
    // El separador parte la respuesta en dos trozos: el primero llega
    // enseguida y el segundo, después (así se ve el primer token).
    let cuerpo = respuesta(concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"391\"}}]}\n\n",
        "\u{1}GUION\u{1}",
        "data: {\"choices\":[{\"delta\":{\"content\":\"\\nUlán Bator\"}}],\"usage\":{\"total_tokens\":42}}\n\n",
        "data: [DONE]\n\n"
    ));
    let puerto = motor_falso(cuerpo, Duration::from_millis(80));
    let r = pregunta(puerto, "modelo", "pregunta", None).expect("el motor falso responde");
    assert_eq!(
        r.texto, "391\nUlán Bator",
        "el texto se arma con los deltas"
    );
    assert_eq!(
        r.metrica.tokens,
        Tokens::Uso(42),
        "los tokens son los del motor"
    );
    assert!(
        r.metrica.latencia_total_ms >= 80,
        "la latencia total espera al motor: {}",
        r.metrica.latencia_total_ms
    );
    let primer = r.metrica.primer_token_ms.expect("el motor dio streaming");
    assert!(
        primer < r.metrica.latencia_total_ms,
        "el primer token llega antes que el final: {primer} vs {}",
        r.metrica.latencia_total_ms
    );
}

#[test]
fn sin_usage_los_tokens_se_cuentan_en_palabras_y_se_dice_que_es_aproximado() {
    let cuerpo = respuesta(concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"391\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"Ulán Bator\"}}]}\n\n",
        "data: [DONE]\n\n"
    ));
    let puerto = motor_falso(cuerpo, Duration::from_millis(10));
    let r = pregunta(puerto, "modelo", "pregunta", None).expect("el motor falso responde");
    assert_eq!(r.texto, "391Ulán Bator");
    assert_eq!(
        r.metrica.tokens,
        Tokens::Aproximado(2),
        "dos palabras, contadas a mano"
    );
}

#[test]
fn una_respuesta_sin_streaming_tambien_se_lee() {
    let cuerpo = respuesta(
        "{\"choices\":[{\"message\":{\"content\":\"391\\nNO SÉ\"}}],\"usage\":{\"total_tokens\":7}}",
    );
    let puerto = motor_falso(cuerpo, Duration::from_millis(5));
    let r = pregunta(puerto, "modelo", "pregunta", None).expect("el motor falso responde");
    assert_eq!(r.texto, "391\nNO SÉ");
    assert_eq!(r.metrica.tokens, Tokens::Uso(7));
    assert!(
        r.metrica.primer_token_ms.is_none(),
        "sin streaming no hay primer token: se dice que no está"
    );
}

#[test]
fn el_motor_que_pide_clave_dice_donde_ponerla() {
    let cuerpo = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n";
    let puerto = motor_falso(cuerpo.to_string(), Duration::from_millis(5));
    let error = pregunta(puerto, "modelo", "pregunta", None).expect_err("el motor pide clave");
    assert_eq!(error.codigo, 1);
    assert!(
        error.mensaje.contains("ropero config keys"),
        "dice dónde poner la clave: {}",
        error.mensaje
    );
    assert!(
        !error.mensaje.contains("Bearer"),
        "la clave no se imprime: {}",
        error.mensaje
    );
}

#[test]
fn un_motor_que_no_responde_es_un_error_que_dice_que_hay_que_hacer() {
    // Un puerto donde no hay nadie: la conexión falla.
    let listener = TcpListener::bind("127.0.0.1:0").expect("se puede escuchar");
    let puerto = listener.local_addr().expect("dirección local").port();
    drop(listener);
    let error = pregunta(puerto, "modelo", "pregunta", None).expect_err("no hay motor");
    assert_eq!(error.codigo, 1);
    assert!(
        error.mensaje.contains("puerto"),
        "dice el puerto: {}",
        error.mensaje
    );
}

#[test]
fn el_motor_en_pie_se_busca_en_el_estado_en_disco() {
    let dir = std::env::temp_dir().join(format!("ropero-inferencia-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("ports")).expect("se puede crear el estado");

    // Un estado con el PID de este mismo proceso: el motor está en pie.
    let puerto = 45999;
    let estado = format!(
        "MODEL=modelo-alpha\nENGINE=turbo-llama\nPID={}\nPORT={puerto}\nLEVEL=low\n",
        std::process::id()
    );
    std::fs::write(dir.join("ports").join(puerto.to_string()), estado)
        .expect("se escribe el estado");
    let motor = motor_de("modelo-alpha", &dir).expect("el motor está en pie");
    assert_eq!(motor.puerto, puerto);
    assert_eq!(motor.nivel, "low");
    assert!(!motor.lanzado, "ropero no lo arrancó: ya estaba");

    // Un estado con un PID que no existe: el motor no está en pie.
    let puerto_muerto = 45998;
    let estado = format!(
        "MODEL=modelo-beta\nENGINE=turbo-llama\nPID=999999\nPORT={puerto_muerto}\nLEVEL=low\n"
    );
    std::fs::write(dir.join("ports").join(puerto_muerto.to_string()), estado)
        .expect("se escribe el estado");
    assert!(
        motor_de("modelo-beta", &dir).is_none(),
        "un motor muerto no está en pie"
    );

    let _ = std::fs::remove_dir_all(&dir);
}
