//! A deliberately badly-behaved stand-in for the `vst-meta` scanner worker.
//!
//! The supervisor in `scan::plugins::spawner` exists to survive plugins that crash or
//! hang the scanner. Those are exactly the conditions a healthy machine will not
//! reproduce on demand, so the recovery paths would otherwise go untested until they
//! matter. This stub speaks the same NDJSON protocol and misbehaves on request,
//! keyed off marker substrings in the paths it is given:
//!
//! - `CRASH` -- announce the path, then die without reporting a result
//! - `HANG`  -- announce the path, then block forever
//! - `SILENT`-- exit immediately, before announcing anything
//! - `GLUE`  -- print a fragment with no newline to stdout before the result, the way
//!   Hadron does, so the result line is glued to it and lost (`plugin-stdout-desyncs-scan`).
//!   The stub does not redirect stdout as the real worker does (ADR-0083): this is the
//!   supervisor's backstop under test
//! - `LINGER`-- scan normally, then never exit after `done`, the way a plugin DLL that
//!   hangs in its exit handlers leaves the real worker (`plugin-scan-hangs-after-last-plugin`)
//! - anything else -- report a plausible success
//!
//! Driven by `tests/plugin_scanner.rs` via `Spawner::with_worker`.

use std::io::{BufRead, Write};

fn main() {
    // Mirrors the real worker's interface: `scan --stdin`.
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "scan") {
        eprintln!("stub worker expects the `scan` subcommand");
        std::process::exit(2);
    }

    let paths: Vec<String> = std::io::stdin()
        .lock()
        .lines()
        .map_while(Result::ok)
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let mut scanned = 0usize;
    let linger = paths.iter().any(|p| p.contains("LINGER"));

    for path in &paths {
        if path.contains("SILENT") {
            // Die before announcing anything, the way a worker that fails during
            // startup would.
            std::process::exit(3);
        }

        writeln!(out, r#"{{"event":"begin","path":{}}}"#, quote(path)).unwrap();
        out.flush().unwrap();

        if path.contains("CRASH") {
            // Abnormal termination with the path still in flight.
            std::process::exit(101);
        }

        if path.contains("HANG") {
            // Never report, never exit: the supervisor's timeout must break this.
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }

        if path.contains("GLUE") {
            // What Hadron's last `ftable` line ends on.
            write!(out, "ft").unwrap();
        }

        writeln!(
            out,
            r#"{{"event":"result","path":{},"status":"success","plugins":[{}]}}"#,
            quote(path),
            plugin_json(path)
        )
        .unwrap();
        out.flush().unwrap();
        scanned += 1;
    }

    writeln!(out, r#"{{"event":"done","scanned":{}}}"#, scanned).unwrap();
    out.flush().unwrap();

    if linger {
        // Every result is out; now refuse to go. Only killing ends this.
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        }
    }
}

/// A minimal `PluginMeta`, matching the real record's required fields.
fn plugin_json(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();

    format!(
        r#"{{"path":{},"format":"VST3","name":{},"vendor":"Stub Audio","version":"1.0.0",
"uid":"00000000000000000000000000000001","category":"Fx","is_instrument":false,
"audio_in_channels":2,"audio_out_channels":2,"audio_in_buses":1,"audio_out_buses":1,
"has_midi_input":false,"has_midi_output":false,"presets":null,"parameters":null,
"latency_samples":null,"has_gui":true,"vendor_url":null,"vendor_email":null,
"is_shell":false,"shell_parent_uid":null,
"extra":{{"format":"VST3","factory_flags":16,"classes":[],"buses":[]}}}}"#,
        quote(path),
        quote(&name)
    )
    .replace('\n', "")
}

/// JSON-quote a string. Paths on Windows are full of backslashes, so this matters.
fn quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
