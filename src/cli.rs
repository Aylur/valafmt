use clap::Parser;
use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use valafmt::lexer;

#[derive(Parser)]
#[command(name = "valafmt", version, about = "An opinionated formatter for Vala")]
pub struct Args {
    /// Files or directories to format (recurses for *.vala and *.vapi).
    /// With no paths, reads from stdin and writes to stdout.
    pub paths: Vec<PathBuf>,

    /// Don't write; report files that would change (exit 1 if any).
    #[arg(long)]
    pub check: bool,

    /// Print the result to stdout instead of writing in place.
    #[arg(long)]
    pub stdout: bool,

    /// Preferred maximum line width.
    #[arg(long, default_value_t = 100)]
    pub line_width: u16,

    /// Indentation width in spaces.
    #[arg(long, default_value_t = 4)]
    pub indent: u8,

    /// Debug: dump the token stream (with trivia) instead of formatting.
    #[arg(long, hide = true)]
    pub dump_tokens: bool,
}

pub fn run() -> ExitCode {
    let args = Args::parse();

    if args.dump_tokens {
        let src = match read_single_input(&args) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("valafmt: {e}");
                return ExitCode::from(2);
            }
        };
        dump_tokens(&src);
        return ExitCode::SUCCESS;
    }

    eprintln!("valafmt: formatting is not implemented yet (milestone M3)");
    ExitCode::from(2)
}

fn read_single_input(args: &Args) -> anyhow::Result<String> {
    match args.paths.as_slice() {
        [] => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            Ok(s)
        }
        [path] => Ok(std::fs::read_to_string(path)?),
        _ => anyhow::bail!("--dump-tokens takes at most one file"),
    }
}

fn dump_tokens(src: &str) {
    let result = lexer::lex(src);
    for err in &result.errors {
        let (line, col) = lexer::cursor::Cursor::line_col(src, err.offset);
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
}
