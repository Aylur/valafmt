//! Corpus runner: walk directories of real Vala code, report what fraction
//! lexes, parses, formats (with the safety check), and reformats
//! idempotently. Usage: cargo run --release --example corpus -- corpus/

use std::path::Path;

#[derive(Default)]
struct Stats {
    files: usize,
    lex_ok: usize,
    parse_ok: usize,
    format_ok: usize,
    idempotent: usize,
    failures: Vec<(String, String)>,
}

fn main() {
    let mut stats = Stats::default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|a| a == "-v");
    for root in args.iter().filter(|a| *a != "-v") {
        walk(Path::new(root), &mut stats);
    }
    println!(
        "files: {}\nlex ok: {} ({:.1}%)\nparse ok: {} ({:.1}%)\nformat+safety ok: {} ({:.1}%)\nidempotent: {} ({:.1}%)",
        stats.files,
        stats.lex_ok,
        pct(stats.lex_ok, stats.files),
        stats.parse_ok,
        pct(stats.parse_ok, stats.files),
        stats.format_ok,
        pct(stats.format_ok, stats.files),
        stats.idempotent,
        pct(stats.idempotent, stats.files),
    );
    let show = if verbose { stats.failures.len() } else { 20 };
    for (path, err) in stats.failures.iter().take(show) {
        println!("FAIL {path}: {err}");
    }
    if stats.failures.len() > show {
        println!("... and {} more failures", stats.failures.len() - show);
    }
}

fn pct(n: usize, d: usize) -> f64 {
    if d == 0 {
        0.0
    } else {
        n as f64 * 100.0 / d as f64
    }
}

fn walk(path: &Path, stats: &mut Stats) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();
        entries.sort();
        for e in entries {
            walk(&e, stats);
        }
        return;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext != "vala" {
        return;
    }
    let Ok(src) = std::fs::read_to_string(path) else { return };
    stats.files += 1;
    let name = path.display().to_string();

    let lexed = valafmt::lexer::lex(&src);
    if !lexed.errors.is_empty() {
        stats.failures.push((name, format!("lex: {}", lexed.errors[0].message)));
        return;
    }
    stats.lex_ok += 1;

    match valafmt::parser::parse(lexed.tokens) {
        Err(e) => {
            let (line, col) = valafmt::lexer::cursor::Cursor::line_col(&src, e.offset);
            stats.failures.push((name, format!("parse {line}:{col}: {}", e.message)));
            return;
        }
        Ok(_) => stats.parse_ok += 1,
    }

    let opts = valafmt::Options::default();
    let out = match valafmt::format_source(&src, &opts) {
        Ok(o) => o,
        Err(e) => {
            let msg = format!("{e}").lines().next().unwrap_or("").to_string();
            stats.failures.push((name, format!("format: {msg}")));
            return;
        }
    };
    stats.format_ok += 1;

    match valafmt::format_source(&out, &opts) {
        Ok(again) if again == out => stats.idempotent += 1,
        Ok(_) => stats.failures.push((name, "not idempotent".to_string())),
        Err(e) => {
            let msg = format!("{e}").lines().next().unwrap_or("").to_string();
            stats.failures.push((name, format!("reformat: {msg}")));
        }
    }
}
