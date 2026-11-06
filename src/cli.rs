use clap::Parser;
use std::io::{IsTerminal, Read};
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
    /// Files or directories to format recursively.
    pub paths: Vec<PathBuf>,

    /// Don't respect .gitignore / .ignore files when searching directories.
    #[arg(long)]
    pub no_ignore: bool,

    /// Check if the given files are formatted (exit 1 if any are not).
    #[arg(short, long, conflicts_with = "write")]
    pub check: bool,

    /// Edit files in-place. Without this, formatted output goes to stdout.
    #[arg(short, long)]
    pub write: bool,

    /// Preferred maximum line width.
    #[arg(long)]
    pub width: Option<u16>,

    /// Indentation width in spaces.
    #[arg(long)]
    pub indent: Option<u8>,

    /// Indent with tabs instead of spaces.
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = "true", value_name = "BOOL")]
    pub use_tabs: Option<bool>,

    /// Debug: dump the token stream (with trivia) instead of formatting.
    #[arg(long, hide = true)]
    pub dump_tokens: bool,
}

pub fn run() -> ExitCode {
    let args = Args::parse();
    if args.dump_tokens {
        return dump_tokens_mode(&args);
    }

    if args.paths.is_empty() {
        if args.write {
            eprintln!("valafmt: --write requires at least one path");
            return ExitCode::from(2);
        }
        // Resolve .editorconfig as if stdin were a .vala file in the cwd.
        let options = cli_options(&args, editorconfig_options(Path::new("stdin.vala")));
        return stdin_mode(&args, &options);
    }

    let mut files = Vec::new();
    for path in &args.paths {
        if path.is_dir() {
            collect_vala_files(path, !args.no_ignore, &mut files);
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

    let mut unformatted = 0usize;
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
        let options = cli_options(&args, editorconfig_options(file));
        let out = match format_source(&src, &options) {
            Ok(out) => out,
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
                continue;
            }
            Err(e @ FormatError::SafetyCheck(_)) => {
                eprintln!("valafmt: {}: {e}", file.display());
                internal = true;
                continue;
            }
        };

        let name = file.display().to_string();
        if args.check {
            if out != src {
                warn(&name);
                unformatted += 1;
            }
        } else if args.write {
            if out != src {
                if let Err(e) = std::fs::write(file, &out) {
                    eprintln!("valafmt: {}: {e}", file.display());
                    errors += 1;
                    continue;
                }
                println!("{name}");
            }
        } else {
            print!("{out}");
        }
    }

    if internal {
        ExitCode::from(3)
    } else if errors > 0 {
        ExitCode::from(2)
    } else if unformatted > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// Prints `[warn] msg` to stderr, like Prettier does.
fn warn(msg: &str) {
    let color = std::io::stderr().is_terminal();
    eprintln!("[{}] {msg}", paint(color, "33", "warn"));
}

/// Wraps `text` in the ANSI color `code` when `color` is set.
fn paint(color: bool, code: &str, text: &str) -> String {
    if color {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
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
                    warn("(stdin)");
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

/// Applies the options given on the command line on top of `base`.
fn cli_options(args: &Args, base: Options) -> Options {
    Options {
        width: args.width.unwrap_or(base.width),
        indent: args.indent.unwrap_or(base.indent),
        use_tabs: args.use_tabs.unwrap_or(base.use_tabs),
    }
}

/// Reads `max_line_length`, `indent_style` and `indent_size` from the
/// .editorconfig files that apply to `file`, falling back to the defaults for
/// anything missing.
fn editorconfig_options(file: &Path) -> Options {
    use ec4rs::property::{IndentSize, IndentStyle, MaxLineLen, TabWidth};

    let mut options = Options::default();
    let Ok(path) = std::path::absolute(file) else {
        return options;
    };
    let Ok(mut props) = ec4rs::properties_of(path) else {
        return options;
    };
    // Derives indent_size from indent_style = tab and tab_width, per the spec.
    props.use_fallbacks();
    if let Ok(MaxLineLen::Value(n)) = props.get::<MaxLineLen>() {
        options.width = u16::try_from(n).unwrap_or(options.width);
    }
    let indent = match props.get::<IndentSize>() {
        Ok(IndentSize::Value(n)) => Some(n),
        Ok(IndentSize::UseTabWidth) => match props.get::<TabWidth>() {
            Ok(TabWidth::Value(n)) => Some(n),
            _ => None,
        },
        Err(_) => None,
    };
    if let Some(n) = indent {
        options.indent = u8::try_from(n).unwrap_or(options.indent);
    }
    match props.get::<IndentStyle>() {
        Ok(IndentStyle::Tabs) => options.use_tabs = true,
        Ok(IndentStyle::Spaces) => options.use_tabs = false,
        Err(_) => {}
    }
    options
}

/// Version control directories that are always skipped, like Prettier does.
const VCS_DIRS: &[&str] = &[".git", ".hg", ".jj", ".sl", ".svn"];

fn collect_vala_files(dir: &Path, respect_ignore: bool, out: &mut Vec<PathBuf>) {
    let mut builder = ignore::WalkBuilder::new(dir);
    builder
        .hidden(false)
        .git_ignore(respect_ignore)
        .git_global(respect_ignore)
        .git_exclude(respect_ignore)
        .ignore(respect_ignore)
        .parents(respect_ignore)
        .require_git(false)
        .follow_links(true)
        .filter_entry(|entry| {
            entry.depth() == 0
                || !entry.file_type().is_some_and(|t| t.is_dir())
                || !VCS_DIRS.contains(&entry.file_name().to_string_lossy().as_ref())
        });
    let walker = builder.build();
    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let path = entry.into_path();
        if matches!(
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
