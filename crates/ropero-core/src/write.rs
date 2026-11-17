//! La salida del programa, tolerante a un pipe roto.
//!
//! `println!` **panica** cuando el destino ya no existe: `ropero command groot |
//! head -1` cierra el pipe antes de que termine la salida y el programa
//! revienta con un panic (`Broken pipe (os error 32)`). Es un crash en un
//! idiom de shell muy común, así que la escritura es **tolerante**:
//!
//! - Un `BrokenPipe` **no es un error**: es la señal de que quien lee ya no
//!   quiere más. Se marca la salida como rota y se deja de escribir.
//! - Cualquier otro error de E/S **sí** es un error, y se reporta.
//!
//! La salida rota **no** cambia el código de salida del comando: un
//! `ropero models | head -1` termina con 0, como si no hubiera pasado nada.
//!
//! ## Por qué vive aquí y no en la CLI
//!
//! La E/S **de datos** (disco, entorno, procesos, red) es de la CLI o llega
//! por parámetro (doc B §3.2.3). Aquí lo único que se abre es la **salida
//! estándar** del propio proceso: no se lee nada, no se decide nada, y el
//! contenido que se imprime lo decide el dominio (`ropero_core::output`).
//! La lógica de *qué* se imprime y la de *cómo* se imprime sin reventar
//! comparten el mismo sitio, que es donde se puede probar sin lanzar el
//! binario.

use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};

/// ¿El pipe de stdout ya está roto?
static STDOUT_BROKEN: AtomicBool = AtomicBool::new(false);
/// ¿El pipe de stderr ya está roto?
static STDERR_BROKEN: AtomicBool = AtomicBool::new(false);

/// El resultado de una escritura tolerante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WriteOutcome {
    /// Se escribió todo (o ya estaba roto y no se escribió nada).
    Written,
    /// El pipe está roto: quien lee ya no quiere más.
    BrokenPipe,
    /// Otro fallo de E/S: el mensaje explica qué pasó.
    Failed(String),
}

impl WriteOutcome {
    /// ¿La escritura se completó (o se omitió porque el pipe ya estaba roto)?
    pub fn is_written(&self) -> bool {
        !matches!(self, Self::Failed(_))
    }
}

/// Escribe en el escritor, tolerando el pipe roto.
///
/// El `BrokenPipe` **no** es un error: se marca el pipe como roto (para que
/// las siguientes escrituras no insisten) y se devuelve
/// [`WriteOutcome::BrokenPipe`]. Cualquier otro error se devuelve como
/// [`WriteOutcome::Failed`].
pub fn write_tolerante(w: &mut impl Write, text: &str) -> WriteOutcome {
    match w.write_all(text.as_bytes()).and_then(|()| w.flush()) {
        Ok(()) => WriteOutcome::Written,
        Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe => {
            STDOUT_BROKEN.store(true, Ordering::Relaxed);
            WriteOutcome::BrokenPipe
        }
        Err(e) => WriteOutcome::Failed(e.to_string()),
    }
}

/// Escribe una línea (texto + salto de línea) en el escritor.
pub fn write_line(w: &mut impl Write, text: &str) -> WriteOutcome {
    write_tolerante(w, &format!("{text}\n"))
}

/// Escribe en stdout tolerando el pipe roto: `true` si se escribió.
///
/// `nueva_linea` añade el salto de línea final. Devuelve `false` si el pipe
/// ya estaba roto: el lector ya no quiere más, así que no se escribe nada.
pub fn out(text: &str, nueva_linea: bool) -> bool {
    if stdout_broken() {
        return false;
    }
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let outcome = if nueva_linea {
        write_line(&mut handle, text)
    } else {
        write_tolerante(&mut handle, text)
    };
    match outcome {
        WriteOutcome::Written => true,
        WriteOutcome::BrokenPipe => {
            mark_stdout_broken();
            false
        }
        WriteOutcome::Failed(e) => {
            err_line(&format!("error: no se pudo escribir en stdout: {e}"));
            true
        }
    }
}

/// Escribe una línea en stdout, tolerando el pipe roto.
pub fn out_line(text: &str) -> bool {
    out(text, true)
}

/// Escribe una línea en stderr, tolerando el pipe roto.
pub fn err_line(text: &str) {
    if stderr_broken() {
        return;
    }
    let stderr = std::io::stderr();
    if let WriteOutcome::BrokenPipe = write_line(&mut stderr.lock(), text) {
        mark_stderr_broken();
    }
}

/// ¿Está roto el pipe de stdout?
pub fn stdout_broken() -> bool {
    STDOUT_BROKEN.load(Ordering::Relaxed)
}

/// ¿Está roto el pipe de stderr?
pub fn stderr_broken() -> bool {
    STDERR_BROKEN.load(Ordering::Relaxed)
}

/// Marca el pipe de stdout como roto.
pub fn mark_stdout_broken() {
    STDOUT_BROKEN.store(true, Ordering::Relaxed);
}

/// Marca el pipe de stderr como roto.
pub fn mark_stderr_broken() {
    STDERR_BROKEN.store(true, Ordering::Relaxed);
}
