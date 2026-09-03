use clap::Parser;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use valafmt::{format_source, FormatError, Options};

#[derive(Parser)]
#[command(
    name = "valafmt",
    version,
    about = "An opinionated code formatter for Vala"
)]
pub struct Args {
    /// Files or directories to format in place (directories are searched
    /// recursively for *.vala and *.vapi files).
    pub paths: Vec<PathBuf>,

    /// Don't write; report files that would change (exit 1 if any).
    #[arg(long)]
    pub check: bool,

    /// Print the formatted result to stdout instead of writing in place.
    #[arg(long)]
    pub stdout: bool,

    /// Preferred maximum line width.
    #[arg(long, default_value_t = 100)]
    pub width: u16,

    /// Indentation width in spaces.
    #[arg(long, default_value_t = 4)]
    pub indent: u8,

    /// Debug: dump the token stream (with trivia) instead of formatting.
    #[arg(long, hide = true)]
    pub dump_tokens: bool,
}

pub fn run() -> ExitCode {
    let args = Args::parse();
    let options = Options {
        width: args.width,
        indent: args.indent,
    };

    if args.dump_tokens {
        return dump_tokens_mode(&args);
    }

    if args.paths.is_empty() {
        return stdin_mode(&args, &options);
    }

    let mut files = Vec::new();
    for path in &args.paths {
        if path.is_dir() {
            collect_vala_files(path, &mut files);
        } else {
            files.push(path.clone());
        }
    }
    files.sort();
    files.dedup();

    if files.is_empty() {
        eprintln!("valafmt: no .vala or .vapi files found");
        return ExitCode::from(2);
    }

    let mut would_change = 0usize;
    let mut errors = 0usize;
    let mut internal = false;
    for file in &files {
        let src = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("valafmt: {}: {e}", file.display());
                errors += 1;
                continue;
            }
        };
        match format_source(&src, &options) {
            Ok(out) => {
                if out == src {
                    continue;
                }
                if args.check {
                    println!("{}", file.display());
                    would_change += 1;
                } else if args.stdout {
                    print!("{out}");
                } else {
                    if let Err(e) = std::fs::write(file, &out) {
                        eprintln!("valafmt: {}: {e}", file.display());
                        errors += 1;
                    }
                }
            }
            Err(FormatError::Parse(diags)) => {
                for d in diags {
                    eprintln!(
                        "{}:{}:{}: error: {}",
                        file.display(),
                        d.line,
                        d.col,
                        d.message
                    );
                }
                errors += 1;
            }
            Err(e @ FormatError::SafetyCheck(_)) => {
                eprintln!("valafmt: {}: {e}", file.display());
                internal = true;
            }
        }
    }

    if internal {
        ExitCode::from(3)
    } else if errors > 0 {
        ExitCode::from(2)
    } else if would_change > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn stdin_mode(args: &Args, options: &Options) -> ExitCode {
    let mut src = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut src) {
        eprintln!("valafmt: stdin: {e}");
        return ExitCode::from(2);
    }
    match format_source(&src, options) {
        Ok(out) => {
            if args.check {
                if out == src {
                    ExitCode::SUCCESS
                } else {
                    eprintln!("valafmt: stdin would be reformatted");
                    ExitCode::from(1)
                }
            } else {
                print!("{out}");
                ExitCode::SUCCESS
            }
        }
        Err(FormatError::Parse(diags)) => {
            for d in diags {
                eprintln!("stdin:{}:{}: error: {}", d.line, d.col, d.message);
            }
            ExitCode::from(2)
        }
        Err(e @ FormatError::SafetyCheck(_)) => {
            eprintln!("valafmt: {e}");
            ExitCode::from(3)
        }
    }
}

fn collect_vala_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_vala_files(&path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("vala") | Some("vapi")
        ) {
            out.push(path);
        }
    }
}

fn dump_tokens_mode(args: &Args) -> ExitCode {
    let src = match args.paths.as_slice() {
        [] => {
            let mut s = String::new();
            if std::io::stdin().read_to_string(&mut s).is_err() {
                return ExitCode::from(2);
            }
            s
        }
        [path] => match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("valafmt: {e}");
                return ExitCode::from(2);
            }
        },
        _ => {
            eprintln!("valafmt: --dump-tokens takes at most one file");
            return ExitCode::from(2);
        }
    };
    let result = valafmt::lexer::lex(&src);
    for err in &result.errors {
        let (line, col) = valafmt::lexer::cursor::Cursor::line_col(&src, err.offset);
        eprintln!("{line}:{col}: error: {}", err.message);
    }
    for tok in &result.tokens {
        for t in &tok.leading {
            println!("    leading  {:?} {:?}", t.kind, t.text);
        }
        println!("{:?} {:?}", tok.kind, tok.text);
        for t in &tok.trailing {
            println!("    trailing {:?} {:?}", t.kind, t.text);
        }
    }
    ExitCode::SUCCESS
}
