//! Colorizado seguro de la salida del motor.
//!
//! Es un colorizador **común**: resalta errores, avisos, rendimiento y
//! peticiones HTTP. Si no hay un formato específico por motor, se usa este.
//! La cobertura es semántica (línea a línea), no un parser del protocolo de
//! cada motor.

use std::io::{self, BufRead, Write};

/// Si la salida debe ir coloreada.
///
/// El color se apaga con `NO_COLOR` o si el destino no es un TTY. Se
/// consulta `stderr` porque la salida en vivo del motor se muestra en la
/// terminal (y bash la mandaba a stderr).
pub fn color_enabled() -> bool {
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn isatty(fd: i32) -> i32;
        }
        // SAFETY: `isatty(2)` es una consulta POSIX sin efectos.
        unsafe { isatty(2) != 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

/// Colorea una línea según su semántica.
///
/// `engine_color` es el código ANSI 256 del motor (si se conoce) para las
/// líneas informativas de arranque/carga; si no, se usa un verde neutro.
pub fn colorize_line(line: &str, engine_color: Option<u8>, enabled: bool) -> String {
    colorize_engine_line(line, None, engine_color, enabled)
}

/// Colorea líneas con reglas compartidas y señales características del motor.
///
/// Los patrones por motor son deliberadamente no exhaustivos: errores y
/// rendimiento siempre tienen prioridad y las líneas desconocidas conservan
/// el fallback semántico común.
pub fn colorize_engine_line(
    line: &str,
    engine: Option<ropero_core::EngineId>,
    engine_color: Option<u8>,
    enabled: bool,
) -> String {
    if !enabled {
        return line.to_string();
    }
    let lower = line.to_lowercase();
    let rst = "\x1b[0m";
    let bred = "\x1b[1;31m";
    let yel = "\x1b[1;33m";
    let mag = "\x1b[1;35m";
    let cyn = "\x1b[2;36m";
    let gry = "\x1b[90m";
    let main = engine_color
        .map(|c| format!("\x1b[38;5;{c}m"))
        .unwrap_or_else(|| "\x1b[1;32m".to_string());

    // llama.cpp prefija cada mensaje con su severidad (`I`, `W`, `E`, ...).
    // Respetamos esa estructura antes de aplicar el color común por palabra:
    // así una sesión Turbo no queda como un bloque amarillo uniforme.
    if matches!(
        engine,
        Some(ropero_core::EngineId::LlamaServer | ropero_core::EngineId::TurboLlama)
    ) {
        let ansi = if lower.contains(" e ") || lower.contains(" fatal") {
            Some("\x1b[1;31m")
        } else if lower.contains(" w ") {
            Some(if engine == Some(ropero_core::EngineId::TurboLlama) {
                "\x1b[38;5;214m"
            } else {
                "\x1b[38;5;208m"
            })
        } else if lower.contains(" d ") || lower.contains(" t ") {
            Some("\x1b[2;37m")
        } else if lower.contains(" i ") {
            Some(if engine == Some(ropero_core::EngineId::TurboLlama) {
                "\x1b[38;5;220m"
            } else {
                "\x1b[38;5;208m"
            })
        } else {
            None
        };
        if let Some(ansi) = ansi {
            return format!("{ansi}{line}{rst}");
        }
    }
    // Ignis uses tracing's timestamp + level + target layout. Preserve its
    // per-line severity. Recent Ignis builds emit OpenTelemetry JSONL, so
    // normalize those records into a compact event summary before the legacy
    // text fallback below.
    if engine == Some(ropero_core::EngineId::Ignis) {
        if let Some((summary, ansi)) = ignis_json_line(line) {
            return format!("{ansi}{summary}{rst}");
        }
        let severity = if lower.contains(" error ") {
            Some(bred)
        } else if lower.contains(" warn ") {
            Some("\x1b[38;5;214m")
        } else if lower.contains(" debug ") || lower.contains(" trace ") {
            Some(gry)
        } else if lower.contains(" info ") {
            Some("\x1b[38;5;51m")
        } else {
            None
        };
        if let Some(ansi) = severity {
            return format!("{ansi}{line}{rst}");
        }
    }

    let es_error = [
        "oom",
        "out of memory",
        "cuda error",
        "segfault",
        "abort",
        "fatal",
        "traceback",
        "error",
        "unrecognized argument",
    ]
    .iter()
    .any(|p| lower.contains(p))
        || lower.contains(" 5")
            && lower.split_whitespace().any(|w| {
                w.len() == 3 && w.starts_with('5') && w.chars().all(|c| c.is_ascii_digit())
            });
    if es_error {
        return format!("{bred}{line}{rst}");
    }
    if [
        "tok/s",
        "tokens per second",
        "throughput",
        "eval time",
        "prompt eval",
    ]
    .iter()
    .any(|p| lower.contains(p))
    {
        return format!("{mag}{line}{rst}");
    }
    if ["warning", "deprecated", " wrn", "warn"]
        .iter()
        .any(|p| lower.contains(p))
    {
        return format!("{yel}{line}{rst}");
    }
    let engine_markers: &[&str] = match engine {
        Some(ropero_core::EngineId::TensorRtLlm) => &["[trt-llm]", "tensorrt_llm", "tensorrt-llm"],
        Some(ropero_core::EngineId::AirLlm) => {
            &["uvicorn", "fastapi", "airllm-server", "application startup"]
        }
        Some(ropero_core::EngineId::MlxServe) => &["mlx", "memory", "tokens/s"],
        Some(ropero_core::EngineId::FastFlowLm) => &["fastflowlm", "flm", "npu"],
        Some(ropero_core::EngineId::Strata) => &["strata", "model loaded", "serving"],
        Some(ropero_core::EngineId::Vllm) => &["vllm", "enginecore", "scheduler"],
        Some(ropero_core::EngineId::Ignis) => &["ignis.", "ignis-server", "model loaded"],
        Some(ropero_core::EngineId::LlamaServer | ropero_core::EngineId::TurboLlama) => {
            &["llama.cpp", "llama_server", "llama_context", "ggml_", "srv"]
        }
        None => &[],
    };
    if engine_markers.iter().any(|marker| lower.contains(marker)) {
        return format!("{main}{line}{rst}");
    }
    if [
        "loaded",
        "llm_load",
        "cuda",
        "metal",
        "gpu",
        "server listening",
        "ready",
        "listening",
        "serve",
    ]
    .iter()
    .any(|p| lower.contains(p))
    {
        return format!("{main}{line}{rst}");
    }
    if ["http", "get ", "post ", "chat", "srv"]
        .iter()
        .any(|p| lower.contains(p))
    {
        return format!("{cyn}{line}{rst}");
    }
    format!("{gry}{line}{rst}")
}

/// Convierte un registro JSONL de Ignis en una línea corta y el color de su
/// severidad OpenTelemetry. Los atributos se filtran por evento para no
/// volcar planes completos de memoria ni metadatos de trazas.
fn ignis_json_line(line: &str) -> Option<(String, &'static str)> {
    let record: serde_json::Value = serde_json::from_str(line).ok()?;
    let object = record.as_object()?;
    let level = object.get("severity_text")?.as_str()?.to_ascii_uppercase();
    let number = object.get("severity_number").and_then(|v| v.as_u64());
    let ansi = match level.as_str() {
        "FATAL" | "CRITICAL" => "\x1b[1;31m",
        "ERROR" => "\x1b[1;31m",
        "WARN" | "WARNING" => "\x1b[38;5;214m",
        "INFO" => "\x1b[38;5;51m",
        "DEBUG" => "\x1b[90m",
        "TRACE" => "\x1b[2;37m",
        _ => match number? {
            21..=24 => "\x1b[1;31m",
            17..=20 => "\x1b[1;31m",
            13..=16 => "\x1b[38;5;214m",
            9..=12 => "\x1b[38;5;51m",
            5..=8 => "\x1b[90m",
            1..=4 => "\x1b[2;37m",
            _ => "\x1b[90m",
        },
    };
    let event = object.get("event_name")?.as_str()?;
    let body = object.get("body").and_then(|v| v.as_str()).unwrap_or("");
    let timestamp = object
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|s| s.get(11..19))
        .unwrap_or("--:--:--");
    let mut summary = format!("{timestamp} {level} {event} · {body}");
    let attrs = object.get("attributes").and_then(|v| v.as_object());
    let mut shown = Vec::new();
    let keys: &[&str] = if event.ends_with("request.done") {
        &[
            "request_id",
            "duration_ms",
            "tokens",
            "tok_s",
            "finish_reason",
            "itl_ms_mean",
            "spec.accepted",
            "spec.drafted",
            "reuse_source",
            "reused_prompt_tokens",
            "thinking_forced",
        ]
    } else if event.ends_with("request.ttft") {
        &["request_id", "duration_ms", "tok_s", "class"]
    } else if event.ends_with("request.admitted") {
        &[
            "request_id",
            "duration_ms",
            "prompt_tokens",
            "prefilled_tokens",
            "lane",
        ]
    } else if event.ends_with("vram_plan") {
        &[
            "budget_bytes",
            "allocated_at_load_bytes",
            "weights_bytes",
            "kv_pool_bytes",
            "kv_pool_tokens",
            "headroom_bytes",
            "oversubscribed",
        ]
    } else if event.ends_with("host_plan") {
        &[
            "planned_bytes",
            "available_bytes",
            "margin_bytes",
            "left_bytes",
        ]
    } else if event.ends_with("kv_pool") {
        &[
            "max_context_tokens",
            "token_capacity",
            "kv_format",
            "rope_scaling",
        ]
    } else if event.ends_with("model.loaded") {
        &[
            "decode_lanes",
            "max_context",
            "kv_format",
            "speculation",
            "vision",
        ]
    } else if event.ends_with("prefill_routes") {
        &["gdn_prefill", "prefill_attention", "prefill_linear"]
    } else if event.ends_with("retained_slot_skipped") {
        &["reason", "retained_slots", "retained_slots_in_use"]
    } else if event.ends_with("config.source") {
        &["profile", "profile_by", "card_total_bytes", "source"]
    } else if event.ends_with("server.listening") || event.ends_with("process.started") {
        &["address", "bind", "model", "exposed", "metrics"]
    } else if event.ends_with("response failed") || body == "response failed" {
        &["classification", "latency"]
    } else {
        &["classification", "reason"]
    };
    if let Some(attrs) = attrs {
        for key in keys {
            if let Some(value) = attrs.get(*key) {
                // Secret-bearing fields are never rendered, even if a future
                // event's allowlist accidentally includes them.
                if key.contains("key") || key.contains("token") && *key != "tokens" {
                    continue;
                }
                let value = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => {
                        if key.ends_with("_bytes") && n.as_u64().unwrap_or(0) >= 1_073_741_824 {
                            format!("{:.1}GiB", n.as_u64().unwrap_or(0) as f64 / 1_073_741_824.0)
                        } else if key.ends_with("_ms") && n.as_u64().unwrap_or(0) >= 1000 {
                            format!("{:.1}s", n.as_u64().unwrap_or(0) as f64 / 1000.0)
                        } else if key.ends_with("_s") {
                            format!("{:.1}", n.as_f64().unwrap_or(0.0))
                        } else {
                            n.to_string()
                        }
                    }
                    serde_json::Value::Bool(b) => b.to_string(),
                    _ => continue,
                };
                shown.push(format!("{key}={value}"));
            }
        }
    }
    if !shown.is_empty() {
        summary.push_str(" · ");
        summary.push_str(&shown.join("  "));
    }
    Some((summary, ansi))
}

/// Cuenta las líneas de un fichero (0 si no existe o no se puede leer).
pub fn line_count(path: &std::path::Path) -> usize {
    match std::fs::File::open(path) {
        Ok(f) => io::BufReader::new(f).lines().count(),
        Err(_) => 0,
    }
}

/// Escribe un error de escritura/salida en vivo de forma visible.
///
/// No se traga: el contrato pide no ocultar errores de escritura/log.
pub fn report_write_error(err: &io::Error) {
    let _ = writeln!(
        io::stderr(),
        "error: fallo escribiendo la salida en vivo: {err}"
    );
}

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// Un seguidor del log: imprime en vivo las líneas nuevas, coloreadas.
pub struct LogFollower {
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl LogFollower {
    /// Arranca un seguidor si procede.
    ///
    /// `engine_color` es el código ANSI 256 del motor (tema informativo).
    pub fn start(
        path: std::path::PathBuf,
        from_line: usize,
        engine: ropero_core::EngineId,
        engine_color: Option<u8>,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let enabled = color_enabled();
        let stop_flag = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            follow_loop(&path, from_line, engine, engine_color, enabled, &stop_flag);
        });
        Self {
            stop,
            handle: Some(handle),
        }
    }

    /// No hace nada: un seguidor vacío.
    pub fn none() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// Pide parar y espera a que termine, drenando lo pendiente.
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for LogFollower {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        // No se hace join en drop para no bloquear inesperadamente; `stop()`
        // es el camino explícito. Si queda vivo, es un hilo demoníaco de
        // lectura sin efectos destructivos.
    }
}

/// Sigue el fichero desde `from_line` e imprime las líneas nuevas.
fn follow_loop(
    path: &std::path::Path,
    mut from_line: usize,
    engine: ropero_core::EngineId,
    engine_color: Option<u8>,
    enabled: bool,
    stop: &AtomicBool,
) {
    let mut stderr = io::stderr().lock();
    while !stop.load(Ordering::Relaxed) {
        match read_from(path, from_line) {
            Ok((lineas, leidas)) => {
                for linea in lineas {
                    let out = colorize_engine_line(&linea, Some(engine), engine_color, enabled);
                    if let Err(e) = writeln!(stderr, "{out}") {
                        report_write_error(&e);
                        return;
                    }
                }
                if leidas > 0 {
                    let _ = stderr.flush();
                }
                from_line += leidas;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                report_write_error(&e);
                return;
            }
        }
        thread::sleep(Duration::from_millis(80));
    }
    // Drenaje final.
    if let Ok((lineas, _)) = read_from(path, from_line) {
        for linea in lineas {
            let out = colorize_engine_line(&linea, Some(engine), engine_color, enabled);
            if let Err(e) = writeln!(stderr, "{out}") {
                report_write_error(&e);
                return;
            }
        }
        let _ = stderr.flush();
    }
}

/// Lee las líneas a partir de `from_line`.
fn read_from(path: &std::path::Path, from_line: usize) -> io::Result<(Vec<String>, usize)> {
    let file = std::fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut out = Vec::new();
    let mut count = 0usize;
    for (i, line) in reader.lines().enumerate() {
        let line = line?;
        if i >= from_line {
            out.push(line);
            count += 1;
        }
    }
    Ok((out, count))
}

#[cfg(test)]
mod tests {
    use super::colorize_engine_line;
    use ropero_core::EngineId;

    #[test]
    fn aplica_el_patron_de_fastflowlm_con_el_tema_del_motor() {
        let salida = colorize_engine_line(
            "FastFlowLM NPU ready",
            Some(EngineId::FastFlowLm),
            Some(196),
            true,
        );
        assert!(salida.starts_with("\x1b[38;5;196m"));
    }

    #[test]
    fn los_errores_tienen_prioridad_sobre_el_color_del_motor() {
        let salida = colorize_engine_line(
            "FastFlowLM ERROR: out of memory",
            Some(EngineId::FastFlowLm),
            Some(196),
            true,
        );
        assert!(salida.starts_with("\x1b[1;31m"));
    }

    #[test]
    fn sin_color_preserva_la_linea_literal() {
        let linea = "FastFlowLM listening";
        assert_eq!(
            colorize_engine_line(linea, Some(EngineId::FastFlowLm), Some(196), false),
            linea
        );
    }
}
