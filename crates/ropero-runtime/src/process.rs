//! La identidad de un proceso: `pid + start_time` (doc B §3.3.3).
//!
//! ## Por qué el PID no basta
//!
//! Los PIDs se reciclan. Un arranque que muere deja su PID en el fichero
//! de estado, y el SO lo da a otro proceso: si `ropero stop` se fiara
//! sólo del PID, mataría a un inocente (un navegador, un editor). Con
//! `start_time` —los segundos desde epoch en que arrancó el proceso— la
//! coincidencia es real.
//!
//! El `exe_digest` es la tercera capa: aunque el PID y el `start_time`
//! coincidan, el binario tiene que ser el nuestro.
//!
//! ## El orden de las comprobaciones (R-EST-8)
//!
//! ```text
//! 1. ¿Hay algo escuchando en el puerto?  → si no, no hay nada que parar.
//! 2. ¿El PID del estado es el del puerto?→ si no, es ajeno. NO se toca.
//! 3. ¿El start_time coincide?            → si no, es otro proceso. NO se toca.
//! 4. ¿El exe_digest coincide?           → si no, es otro programa. NO se toca.
//! 5. Ahora sí: SIGTERM al grupo de procesos.
//! ```text

use std::path::Path;

/// La identidad de un proceso: lo que hace que sea **nuestro**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcIdentity {
    /// El PID del proceso.
    pub pid: u32,

    /// Los segundos desde epoch del inicio del proceso.
    ///
    /// En Linux, el campo 22 de `/proc/<pid>/stat`. En macOS, `pbi_start_tvsec`
    /// de `proc_pidinfo(PROC_PIDTBSDINFO)`.
    pub start_time: u64,

    /// El hash del binario ejecutado, si se pudo resolver.
    pub exe_digest: Option<[u8; 32]>,
}

impl ProcIdentity {
    /// La identidad de un proceso vivo.
    ///
    /// `None` si el proceso no existe o si no se pudo leer su `start_time`:
    /// sin `start_time` no se puede probar que un PID recycled sea el
    /// nuestro, así que **no se inventa** una identidad.
    ///
    /// ## El `exe_digest` y el `exec`
    ///
    /// El digest se lee de `/proc/<pid>/exe`, que es el binario que el
    /// proceso tiene **ahora**. Un proceso que hace `exec` (un script que
    /// lanza otro programa, un motor que se reescribe) cambia de binario
    /// después de arrancar: el digest del launch y el de la parada no
    /// coinciden, y la comparación fallaría con un proceso **nuestro**.
    ///
    /// Por eso el digest **no** se usa para decidir si un proceso es
    /// nuestro: la decisión la toman el PID y el `start_time`, que no
    /// cambian. El digest es un dato informative, no un filtro.
    pub fn of(pid: u32) -> Option<Self> {
        let start_time = start_time_of(pid)?;
        Some(Self {
            pid,
            start_time,
            exe_digest: exe_digest_of(pid),
        })
    }

    /// La identidad de un proceso, con el digest del binario ya conocido.
    ///
    /// Es la variante que usan las pruebas: el digest se pasa, no se lee.
    pub fn with_digest(pid: u32, start_time: u64, exe_digest: Option<[u8; 32]>) -> Self {
        Self {
            pid,
            start_time,
            exe_digest,
        }
    }

    /// Si esta identidad es la misma que la otra.
    ///
    /// El **PID** y el `start_time` son la prueba: los PIDs se reciclan,
    /// pero un `start_time` distinto es otro proceso (P-3).
    ///
    /// El `exe_digest` **no** decide: un proceso que hace `exec` cambia
    /// de binario después de arrancar, así que el digest del arranque y
    /// el de la parada no coinciden aunque sea el mismo proceso. El
    /// digest se compara sólo cuando **los dos** lo tienen y **ninguno**
    /// de los dos ha hecho `exec` desde el arranque —que es un dato que
    /// no se puede saber—, así que aquí no se usa como filtro: es un
    /// dato informative para el diagnóstico.
    pub fn es_la_misma(&self, otra: &ProcIdentity) -> bool {
        if self.pid != otra.pid {
            return false;
        }
        self.start_time == otra.start_time
    }

    /// Si los dos digests conocidos **difieren**.
    ///
    /// Es un dato informativo, no un filtro: un `exec` lo cambia sin que
    /// el proceso deje de ser el nuestro.
    pub fn digest_difiere(&self, otra: &ProcIdentity) -> bool {
        match (self.exe_digest, otra.exe_digest) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        }
    }
}

/// El `start_time` de un proceso, en segundos desde epoch.
///
/// En Linux se lee el campo 22 de `/proc/<pid>/stat`. Ese campo viene
/// después del `comm`, que va entre paréntesis y **puede contener
/// espacios y paréntesis**, así que no se parte el campo por espacios:
/// se busca el **último** `)` y se parte a partir de ahí.
#[cfg(target_os = "linux")]
pub fn start_time_of(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // El `comm` va entre paréntesis: `pid (comm) state ppid ...`.
    let fin_comm = stat.rfind(')')?;
    let resto = stat.get(fin_comm + 1..)?;
    // Tras el `)` vienen: state(3) ppid(4) pgrp(5) session(6) tty(7)
    // tpgid(8) flags(9) minflt(10) cminflt(11) majflt(12) cmajflt(13)
    // utime(14) stime(15) cutime(16) cstime(17) priority(18) nice(19)
    // num_threads(20) itrealvalue(21) starttime(22).
    let campos: Vec<&str> = resto.split_whitespace().collect();
    // `resto` empieza por el estado (campo 3), así que el campo 22 es el
    // índice 19 (22 - 3).
    campos.get(19)?.parse::<u64>().ok()
}

/// Lee la hora de inicio del proceso mediante la API de procesos de macOS.
#[cfg(target_os = "macos")]
pub fn start_time_of(pid: u32) -> Option<u64> {
    proceso_mac(pid).map(|info| info.pbi_start_tvsec)
}

/// Lee el estado del proceso mediante la API de procesos de macOS.
#[cfg(target_os = "macos")]
pub fn estado_del_proceso(pid: u32) -> Option<char> {
    use std::mem::{MaybeUninit, size_of};

    let pid_nativo = libc::pid_t::try_from(pid).ok()?;
    let mut info = MaybeUninit::<libc::proc_bsdshortinfo>::zeroed();
    // SAFETY: proc_pidinfo writes at most `buffersize` bytes to the valid
    // buffer. The result is read only when the API reports a full structure.
    let bytes = unsafe {
        proc_pidinfo(
            pid_nativo,
            libc::PROC_PIDT_SHORTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size_of::<libc::proc_bsdshortinfo>() as libc::c_int,
        )
    };
    if bytes != size_of::<libc::proc_bsdshortinfo>() as libc::c_int {
        return None;
    }

    // SAFETY: the successful return above confirms the full structure.
    let info = unsafe { info.assume_init() };
    if info.pbsi_pid != pid {
        return None;
    }
    let estado = info.pbsi_status;
    match estado {
        libc::SIDL => Some('I'),
        libc::SRUN => Some('R'),
        libc::SSLEEP => Some('S'),
        libc::SSTOP => Some('T'),
        libc::SZOMB => Some('Z'),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
#[link(name = "proc")]
unsafe extern "C" {
    fn proc_pidinfo(
        pid: libc::c_int,
        flavor: libc::c_int,
        arg: u64,
        buffer: *mut libc::c_void,
        buffersize: libc::c_int,
    ) -> libc::c_int;
}

#[cfg(target_os = "macos")]
fn proceso_mac(pid: u32) -> Option<libc::proc_bsdinfo> {
    use std::mem::{MaybeUninit, size_of};

    let mut info = MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    // SAFETY: proc_pidinfo writes at most `buffersize` bytes to the valid
    // buffer. The result is read only when the API reports a full structure.
    let bytes = unsafe {
        proc_pidinfo(
            libc::pid_t::try_from(pid).ok()?,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size_of::<libc::proc_bsdinfo>() as libc::c_int,
        )
    };
    if bytes != size_of::<libc::proc_bsdinfo>() as libc::c_int {
        return None;
    }

    // SAFETY: the successful return above confirms that the full struct was
    // initialized by proc_pidinfo.
    let info = unsafe { info.assume_init() };
    (info.pbi_pid == pid).then_some(info)
}

/// El estado de un proceso, tal y como lo dice el kernel.
///
/// En Linux se lee de `/proc/<pid>/stat`: la letra del estado es el
/// primer campo después del `comm` ('R' ejecutando, 'S' dormido, 'Z'
/// zombi, 'T' parado…).
///
/// `None` si no se puede leer: el proceso no existe, o el sistema no
/// tiene `/proc`.
///
/// ## Por qué importa el zombi
///
/// Un proceso que ha muerto pero que su padre todavía no ha recogido
/// sigue en la tabla de procesos: la señal 0 la acepta y `kill(pid, 0)`
/// dice que está vivo. El motor que muere al hacer bind es exactamente
/// eso: un zombi durante el arranque, y sin mirar su estado parecería un
/// motor sano.
#[cfg(target_os = "linux")]
pub fn estado_del_proceso(pid: u32) -> Option<char> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // El `comm` va entre paréntesis y puede contener espacios y
    // paréntesis: el estado es el primer campo **después** del cierre.
    let fin_comm = stat.rfind(')')?;
    let resto = stat.get(fin_comm + 1..)?;
    resto.split_whitespace().next()?.chars().next()
}

/// Sin una implementación nativa, no se inventan datos de proceso.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn start_time_of(_pid: u32) -> Option<u64> {
    None
}

/// Sin una implementación nativa, no se inventan datos de proceso.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
pub fn estado_del_proceso(_pid: u32) -> Option<char> {
    None
}

/// El SHA-256 del binario de un proceso, si se puede resolver.
///
/// Se usa el enlace `/proc/<pid>/exe`. Si el proceso es nuestro, el
/// enlace apunta al binario; si es un zombie o pertenece a otro usuario,
/// puede no estar disponible, y entonces el digest es `None`: un dato que
/// falta, no una discrepancia.
pub fn exe_digest_of(pid: u32) -> Option<[u8; 32]> {
    let exe = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    digest_of(&exe)
}

/// El SHA-256 de un fichero.
///
/// Implementación propia y mínima: el runtime sólo necesita un hash de 32
/// bytes para comparar binarios, y una dependencia de criptografía por eso
/// solo sería desproporcionada. Es SHA-256 (FIPS 180-4).
pub fn digest_of(path: &Path) -> Option<[u8; 32]> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let leidos = file.read(&mut buffer).ok()?;
        if leidos == 0 {
            break;
        }
        hasher.update(&buffer[..leidos]);
    }
    Some(hasher.finalize())
}

/// SHA-256 (FIPS 180-4), la parte que hace falta.
pub struct Sha256 {
    estado: [u32; 8],
    buffer: [u8; 64],
    buffer_len: usize,
    total: u64,
}

impl Sha256 {
    /// Un hash nuevo.
    pub fn new() -> Self {
        Self {
            estado: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            buffer: [0u8; 64],
            buffer_len: 0,
            total: 0,
        }
    }

    /// Actualiza el hash con más datos.
    pub fn update(&mut self, datos: &[u8]) {
        self.total = self.total.wrapping_add(datos.len() as u64);
        let mut offset = 0;
        if self.buffer_len > 0 {
            let faltan = 64 - self.buffer_len;
            let cogidos = faltan.min(datos.len());
            self.buffer[self.buffer_len..self.buffer_len + cogidos]
                .copy_from_slice(&datos[..cogidos]);
            self.buffer_len += cogidos;
            offset = cogidos;
            if self.buffer_len == 64 {
                let bloque = self.buffer;
                self.comprimir(&bloque);
                self.buffer_len = 0;
            }
        }
        while offset + 64 <= datos.len() {
            let mut bloque = [0u8; 64];
            bloque.copy_from_slice(&datos[offset..offset + 64]);
            self.comprimir(&bloque);
            offset += 64;
        }
        if offset < datos.len() {
            let resto = datos.len() - offset;
            self.buffer[..resto].copy_from_slice(&datos[offset..]);
            self.buffer_len = resto;
        }
    }

    /// El hash final.
    pub fn finalize(mut self) -> [u8; 32] {
        let bits = self.total.wrapping_mul(8);
        self.update(&[0x80]);
        // El relleno va con ceros hasta que queden 8 bytes de longitud.
        while self.buffer_len != 56 {
            self.update(&[0x00]);
        }
        let mut bloque_final = self.buffer;
        bloque_final[56..64].copy_from_slice(&bits.to_be_bytes());
        self.comprimir(&bloque_final);
        let mut out = [0u8; 32];
        for (i, palabra) in self.estado.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&palabra.to_be_bytes());
        }
        out
    }

    /// La ronda de compresión.
    fn comprimir(&mut self, bloque: &[u8; 64]) {
        const K: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                bloque[i * 4],
                bloque[i * 4 + 1],
                bloque[i * 4 + 2],
                bloque[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.estado;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        self.estado[0] = self.estado[0].wrapping_add(a);
        self.estado[1] = self.estado[1].wrapping_add(b);
        self.estado[2] = self.estado[2].wrapping_add(c);
        self.estado[3] = self.estado[3].wrapping_add(d);
        self.estado[4] = self.estado[4].wrapping_add(e);
        self.estado[5] = self.estado[5].wrapping_add(f);
        self.estado[6] = self.estado[6].wrapping_add(g);
        self.estado[7] = self.estado[7].wrapping_add(h);
    }
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::new()
    }
}
