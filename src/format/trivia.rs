//! Comment and blank-line emission. Comments and preprocessor directives ride
//! on tokens as trivia; this module turns a token into a `Doc` that includes
//! its leading comments/directives (each on its own line) and its trailing
//! comments (as line suffixes). Because every token the parser consumed goes
//! through `tok()`, no comment can be silently dropped — and the safety check
//! verifies that end to end.

use crate::doc::{self, Doc};
use crate::lexer::token::{Token, Trivia, TriviaKind};

/// Full token emission: leading trivia, the token text, trailing trivia.
pub fn tok(t: &Token) -> Doc {
    let mut parts = leading_parts(&t.leading);
    let has_leading = !parts.is_empty();
    if has_leading {
        // Each leading piece sits on its own line before the token.
        let mut seq = Vec::new();
        for p in parts.drain(..) {
            seq.push(p);
            seq.push(doc::hardline());
        }
        seq.push(token_text(t));
        seq.extend(trailing_docs(&t.trailing));
        doc::concat(seq)
    } else {
        let mut seq = vec![token_text(t)];
        seq.extend(trailing_docs(&t.trailing));
        doc::concat(seq)
    }
}

/// Just the token text (no trivia), as Text or byte-exact Literal.
pub fn token_text(t: &Token) -> Doc {
    if t.text.contains('\n') {
        doc::literal(t.text.clone())
    } else {
        doc::text(t.text.clone())
    }
}

/// Leading comment/directive pieces for a token, one `Doc` per piece. The
/// caller emits a hardline after every piece; a blank line in the source
/// (between pieces, or between the last piece and the token) is preserved as
/// an empty piece, which renders as one extra hardline.
pub fn leading_parts(leading: &[Trivia]) -> Vec<Doc> {
    let mut parts = Vec::new();
    let mut pending_newlines = 0usize;
    for tr in leading {
        match tr.kind {
            TriviaKind::Whitespace => pending_newlines += tr.newlines(),
            TriviaKind::LineComment | TriviaKind::BlockComment | TriviaKind::DocComment => {
                push_gap(&mut parts, pending_newlines);
                pending_newlines = 0;
                parts.push(comment_doc(tr));
            }
            TriviaKind::Directive => {
                push_gap(&mut parts, pending_newlines);
                pending_newlines = 0;
                parts.push(doc::directive(tr.text.trim_end().to_string()));
            }
        }
    }
    if !parts.is_empty() && pending_newlines >= 2 {
        // Blank line between the last comment and the token itself.
        parts.push(doc::nil());
    }
    parts
}

/// Between two leading pieces, preserve at most one blank line.
fn push_gap(parts: &mut Vec<Doc>, pending_newlines: usize) {
    if !parts.is_empty() && pending_newlines >= 2 {
        parts.push(doc::nil());
    }
}

/// Trailing trivia docs: line comments become line suffixes that force a
/// break; single-line block comments print inline.
pub fn trailing_docs(trailing: &[Trivia]) -> Vec<Doc> {
    let mut out = Vec::new();
    for tr in trailing {
        match tr.kind {
            TriviaKind::Whitespace => {}
            TriviaKind::LineComment => {
                out.push(doc::line_suffix(doc::text(format!(" {}", tr.text.trim_end()))));
                out.push(doc::break_parent());
            }
            TriviaKind::BlockComment | TriviaKind::DocComment => {
                out.push(doc::text(format!(" {}", tr.text.trim_end())));
            }
            TriviaKind::Directive => {
                // Cannot occur in trailing position (directives start a line).
                out.push(doc::directive(tr.text.trim_end().to_string()));
            }
        }
    }
    out
}

/// A single comment as a Doc. Multi-line block comments whose continuation
/// lines all start with `*` are re-indented so the stars align under the
/// opening `/*`; anything else is kept byte-exact.
pub fn comment_doc(tr: &Trivia) -> Doc {
    let text = tr.text.trim_end();
    if !text.contains('\n') {
        return doc::text(text.to_string());
    }
    let mut lines = text.lines();
    let first = lines.next().unwrap_or_default().trim_end();
    let rest: Vec<&str> = lines.collect();
    if rest.iter().all(|l| l.trim_start().starts_with('*')) {
        let mut seq = vec![doc::text(first.to_string())];
        for l in rest {
            seq.push(doc::hardline());
            seq.push(doc::text(format!(" {}", l.trim_start().trim_end())));
        }
        doc::concat(seq)
    } else {
        doc::literal(text.to_string())
    }
}

/// Does this token's leading trivia indicate a blank line in the source
/// before the construct it begins (i.e. before its first comment, or before
/// the token itself when it has no comments)?
pub fn blank_line_before(t: &Token) -> bool {
    let mut newlines = 0usize;
    for tr in &t.leading {
        match tr.kind {
            TriviaKind::Whitespace => newlines += tr.newlines(),
            _ => break,
        }
    }
    newlines >= 2
}

/// True if the token has any leading comments or directives.
pub fn has_leading_pieces(t: &Token) -> bool {
    t.leading.iter().any(|tr| tr.is_comment() || tr.kind == TriviaKind::Directive)
}
