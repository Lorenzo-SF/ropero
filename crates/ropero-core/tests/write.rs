//! La escritura tolerante a un pipe roto (`ropero-core::write`).

use std::io::{self, Write};

use ropero_core::{WriteOutcome, write_line, write_tolerante};

/// Un escritor que **siempre** falla con `BrokenPipe`.
struct Roto;

impl Write for Roto {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::BrokenPipe))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Un escritor que falla con un error que **no** es de pipe roto.
struct RotoConError;

impl Write for RotoConError {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Un escritor que cuenta lo que se le escribe.
#[derive(Default)]
struct Contador {
    texto: String,
}

impl Write for Contador {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.texto.push_str(&String::from_utf8_lossy(buf));
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Un pipe roto **no** es un error: es el fin normal de la salida.
#[test]
fn un_pipe_roto_no_es_un_error() {
    let mut roto = Roto;
    let outcome = write_tolerante(&mut roto, "hola");
    assert_eq!(outcome, WriteOutcome::BrokenPipe);
}

/// Un error que no es de pipe roto **sí** se reporta.
#[test]
fn otro_error_de_es_si_se_reporta() {
    let mut roto = RotoConError;
    match write_tolerante(&mut roto, "hola") {
        WriteOutcome::Failed(msg) => assert!(!msg.is_empty()),
        otro => panic!("se esperaba un fallo, se obtuvo {otro:?}"),
    }
}

/// Una escritura normal se escribe entera.
#[test]
fn una_escritura_normal_llega_entera() {
    let mut contador = Contador::default();
    assert_eq!(
        write_tolerante(&mut contador, "hola"),
        WriteOutcome::Written
    );
    assert_eq!(contador.texto, "hola");
}

/// `write_line` añade el salto de línea.
#[test]
fn write_line_anade_el_salto_de_linea() {
    let mut contador = Contador::default();
    assert_eq!(write_line(&mut contador, "hola"), WriteOutcome::Written);
    assert_eq!(contador.texto, "hola\n");
}

/// `is_written` distingue el pipe roto (que no es un fallo) del error real.
#[test]
fn is_written_distingue_el_pipe_roto_del_error_real() {
    assert!(WriteOutcome::Written.is_written());
    assert!(WriteOutcome::BrokenPipe.is_written());
    assert!(!WriteOutcome::Failed("permiso denegado".to_string()).is_written());
}
