//! Colorizado seguro de la salida del motor.
//!
//! Es un colorizador **común**: resalta errores, avisos, rendimiento y
//! peticiones HTTP. Si no hay un formato específico por motor, se usa este.
//! La cobertura es semántica (línea a línea), no un parser del protocolo de
//! cada motor.

use std::io::{self, BufRead, Write};

use ropero_core::palette::{
    HexColor, LOG_DEFAULT, LOG_ERROR, LOG_INFO, LOG_LIGHT, LOG_LLAMA_WARN_OFFICIAL,
    LOG_LLAMA_WARN_TURBO, LOG_MUTED, LOG_PERFORMANCE, LOG_WARNING,
};

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
/// `engine_color` es el índice heredado del color del motor (si se conoce) para las
/// líneas informativas de arranque/carga; si no, se usa un verde neutro.
pub fn colorize_line(line: &str, engine_color: Option<HexColor>, enabled: bool) -> String {
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
    engine_color: Option<HexColor>,
    enabled: bool,
) -> String {
    if !enabled {
        return line.to_string();
    }
    let lower = line.to_lowercase();
    let rst = "\x1b[0m";
    let bred = LOG_ERROR.foreground();
    let yel = LOG_WARNING.foreground();
    let mag = LOG_PERFORMANCE.foreground();
    let cyn = LOG_INFO.foreground();
    let gry = LOG_MUTED.foreground();
    let light = LOG_LIGHT.foreground();
    let llama_warn_official = LOG_LLAMA_WARN_OFFICIAL.foreground();
    let llama_warn_turbo = LOG_LLAMA_WARN_TURBO.foreground();
    let main = engine_color.unwrap_or(LOG_DEFAULT).foreground();

    if matches!(
        engine,
        Some(ropero_core::EngineId::LlamaServer | ropero_core::EngineId::TurboLlama)
    ) {
        let llama_level = line.split_whitespace().find_map(|token| match token {
            "E" => Some(bred.as_str()),
            "W" => Some(if engine == Some(ropero_core::EngineId::TurboLlama) {
                llama_warn_turbo.as_str()
            } else {
                llama_warn_official.as_str()
            }),
            "I" => Some(main.as_str()),
            "D" => Some(gry.as_str()),
            "T" => Some(light.as_str()),
            _ => None,
        });
        if let Some(ansi) = llama_level {
            return format!("{ansi}{line}{rst}");
        }
    }

    // Ignis uses tracing's timestamp + level + target layout. Preserve its
    // per-line severity. Recent Ignis builds emit OpenTelemetry JSONL, so
    // normalize those records into a compact event summary before the legacy
    // text fallback below.
    if engine == Some(ropero_core::EngineId::Ignis)
        && let Some((summary, ansi)) = ignis_json_line(line, engine_color.unwrap_or(LOG_DEFAULT))
    {
        return format!("{}{summary}{rst}", ansi.foreground());
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
    if es_error || tiene_nivel(line, &["error", "fatal", "critical", "crit"]) {
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
    if tiene_nivel(line, &["warning", "warn", "wrn"]) || lower.contains("deprecated") {
        let ansi = if engine == Some(ropero_core::EngineId::LlamaServer) {
            llama_warn_official.as_str()
        } else if engine == Some(ropero_core::EngineId::TurboLlama) {
            llama_warn_turbo.as_str()
        } else {
            yel.as_str()
        };
        return format!("{ansi}{line}{rst}");
    }
    if tiene_nivel(line, &["debug", "dbg"]) {
        return format!("{gry}{line}{rst}");
    }
    if tiene_nivel(line, &["trace", "trc"]) {
        return format!("{light}{line}{rst}");
    }
    let engine_markers: &[&str] = match engine {
        Some(ropero_core::EngineId::TensorRtLlm) => &["[trt-llm]", "tensorrt_llm", "tensorrt-llm"],
        Some(ropero_core::EngineId::AirLlm) => {
            &["uvicorn", "fastapi", "airllm-server", "application startup"]
        }
        Some(ropero_core::EngineId::MlxServe) => &["mlx", "memory", "tokens/s"],
        Some(ropero_core::EngineId::FastFlowLm) => &["fastflowllm", "flm", "npu"],
        Some(ropero_core::EngineId::Strata) => &["strata", "model loaded", "serving"],
        Some(ropero_core::EngineId::Vllm) => &["vllm", "enginecore", "scheduler"],
        Some(ropero_core::EngineId::Ignis) => &["ignis.", "ignis-server", "model loaded"],
        Some(ropero_core::EngineId::Basalt) => &["[server]", "basalt", "loaded"],
        Some(ropero_core::EngineId::Colibri) => &["colibri", "coli", "model loaded"],
        Some(ropero_core::EngineId::LlamaServer | ropero_core::EngineId::TurboLlama) => {
            &["llama.cpp", "llama_server", "llama_context", "ggml_", "srv"]
        }
        None => &[],
    };
    if engine_markers.iter().any(|marker| lower.contains(marker)) {
        return format!("{main}{line}{rst}");
    }
    if tiene_nivel(line, &["info", "informational", "notice"]) {
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

/// Detecta niveles de log como tokens para evitar que palabras como
/// `information` o `warning_count` disparen un falso positivo.
fn tiene_nivel(line: &str, levels: &[&str]) -> bool {
    line.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| levels.iter().any(|level| token.eq_ignore_ascii_case(level)))
}

/// Convierte un registro JSONL de Ignis en una línea corta y el color de su
/// severidad OpenTelemetry. Los atributos se filtran por evento para no
/// volcar planes completos de memoria ni metadatos de trazas.
fn ignis_json_line(line: &str, info_color: HexColor) -> Option<(String, HexColor)> {
    let record: serde_json::Value = serde_json::from_str(line).ok()?;
    let object = record.as_object()?;
    let level = object.get("severity_text")?.as_str()?.to_ascii_uppercase();
    let number = object.get("severity_number").and_then(|v| v.as_u64());
    let event = object.get("event_name")?.as_str()?;
    let ansi = if event.ends_with("request.done") {
        LOG_PERFORMANCE
    } else {
        match level.as_str() {
            "FATAL" | "CRITICAL" | "ERROR" => LOG_ERROR,
            "WARN" | "WARNING" => LOG_WARNING,
            "INFO" => info_color,
            "DEBUG" => LOG_MUTED,
            "TRACE" => LOG_LIGHT,
            _ => match number? {
                21..=24 => LOG_ERROR,
                17..=20 => LOG_ERROR,
                13..=16 => LOG_WARNING,
                9..=12 => info_color,
                5..=8 => LOG_MUTED,
                1..=4 => LOG_LIGHT,
                _ => LOG_MUTED,
            },
        }
    };
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
        engine_color: Option<HexColor>,
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
    engine_color: Option<HexColor>,
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
    use ropero_core::{EngineColor, EngineId, palette};

    fn red_engine_color() -> Option<palette::HexColor> {
        palette::engine_hex(EngineColor::Rojo)
    }

    #[test]
    fn aplica_el_patron_de_fastflowllm_con_el_tema_del_motor() {
        let salida = colorize_engine_line(
            "FastFlowLM NPU ready",
            Some(EngineId::FastFlowLm),
            red_engine_color(),
            true,
        );
        assert!(salida.starts_with(&palette::engine_hex(EngineColor::Rojo).unwrap().foreground()));
    }

    #[test]
    fn los_errores_tienen_prioridad_sobre_el_color_del_motor() {
        let salida = colorize_engine_line(
            "FastFlowLM ERROR: out of memory",
            Some(EngineId::FastFlowLm),
            red_engine_color(),
            true,
        );
        assert!(salida.starts_with(&palette::LOG_ERROR.foreground()));
    }

    #[test]
    fn sin_color_preserva_la_linea_literal() {
        let linea = "FastFlowLM listening";
        assert_eq!(
            colorize_engine_line(linea, Some(EngineId::FastFlowLm), red_engine_color(), false),
            linea
        );
    }
}
