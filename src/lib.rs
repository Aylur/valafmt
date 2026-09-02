pub mod doc;
pub mod lexer;
pub mod options;

pub use options::Options;

use std::fmt;

#[derive(Debug)]
pub struct Diagnostic {
    pub offset: usize,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

#[derive(Debug)]
pub enum FormatError {
    /// The input did not lex/parse; nothing was formatted.
    Parse(Vec<Diagnostic>),
    /// The formatter produced output whose token stream differs from the
    /// input's — an internal bug; the input must be left untouched.
    SafetyCheck(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::Parse(diags) => {
                for d in diags {
                    writeln!(f, "{}:{}: error: {}", d.line, d.col, d.message)?;
                }
                Ok(())
            }
            FormatError::SafetyCheck(msg) => {
                write!(f, "internal error (input left unchanged, please report): {msg}")
            }
        }
    }
}

pub(crate) fn diagnostics_from_lex(src: &str, errors: &[lexer::LexError]) -> Vec<Diagnostic> {
    errors
        .iter()
        .map(|e| {
            let (line, col) = lexer::cursor::Cursor::line_col(src, e.offset);
            Diagnostic { offset: e.offset, line, col, message: e.message.clone() }
        })
        .collect()
}

/// Format Vala source text. Returns the formatted text, or an error if the
/// input does not parse or the internal safety check fails.
pub fn format_source(src: &str, _options: &Options) -> Result<String, FormatError> {
    let lexed = lexer::lex(src);
    if !lexed.errors.is_empty() {
        return Err(FormatError::Parse(diagnostics_from_lex(src, &lexed.errors)));
    }
    // Parser + printer land in later milestones; for now this is lex-only.
    unimplemented!("formatting is implemented in milestone M3")
}
