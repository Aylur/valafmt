pub mod check;
pub mod doc;
pub mod format;
pub mod lexer;
pub mod options;
pub mod parser;
pub mod syntax;

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
pub fn format_source(src: &str, options: &Options) -> Result<String, FormatError> {
    let lexed = lexer::lex(src);
    if !lexed.errors.is_empty() {
        return Err(FormatError::Parse(diagnostics_from_lex(src, &lexed.errors)));
    }
    let file = parser::parse(lexed.tokens).map_err(|e| {
        let (line, col) = lexer::cursor::Cursor::line_col(src, e.offset);
        let mut message = e.message;
        if src.contains("#if") {
            message.push_str(
                "\nnote: this file uses #if preprocessor branches; valafmt can only \
                 format files whose branches each form valid syntax on their own",
            );
        }
        FormatError::Parse(vec![Diagnostic { offset: e.offset, line, col, message }])
    })?;
    let doc = format::format_file(&file, src);
    let output = doc::printer::print(&doc, options);
    check::verify(src, &output).map_err(FormatError::SafetyCheck)?;
    Ok(output)
}
