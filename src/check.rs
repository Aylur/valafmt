//! The token-stream safety check: after formatting, re-lex the output and
//! verify it contains exactly the same significant tokens and the same
//! comments/directives as the input. This turns any formatter bug that would
//! drop or alter code or comments into a loud error instead of silent damage.

use crate::lexer::token::{Token, TokenKind, TriviaKind};
use crate::lexer::{lex, LexResult};

/// Compare input and output; `Err` carries a human-readable description of
/// the first difference.
pub fn verify(input: &str, output: &str) -> Result<(), String> {
    let a = lex(input);
    let b: LexResult = lex(output);
    if !b.errors.is_empty() {
        return Err(format!(
            "formatted output does not lex: {}",
            b.errors[0].message
        ));
    }
    let sa = significant_stream(&a.tokens);
    let sb = significant_stream(&b.tokens);
    for (i, (x, y)) in sa.iter().zip(sb.iter()).enumerate() {
        if x != y {
            return Err(format!(
                "token stream diverges at item {i}: input has {x:?}, output has {y:?}"
            ));
        }
    }
    if sa.len() != sb.len() {
        return Err(format!(
            "token stream length differs: input {} items, output {} items (first extra: {:?})",
            sa.len(),
            sb.len(),
            if sa.len() > sb.len() { &sa[sb.len()] } else { &sb[sa.len()] }
        ));
    }
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
enum Item {
    Token(TokenKind, String),
    Comment(String),
    Directive(String),
}

fn significant_stream(tokens: &[Token]) -> Vec<Item> {
    let mut out = Vec::new();
    for (i, t) in tokens.iter().enumerate() {
        for tr in &t.leading {
            push_trivia(&mut out, tr.kind, &tr.text);
        }
        if t.kind != TokenKind::Eof && !is_inert_trailing_comma(tokens, i) {
            if t.kind == TokenKind::QuestionDot {
                // `? .` in the input fuses to `?.` in the output; compare
                // both spellings as the two-token form.
                out.push(Item::Token(TokenKind::Question, "?".to_string()));
                out.push(Item::Token(TokenKind::Dot, ".".to_string()));
            } else {
                out.push(Item::Token(t.kind, t.text.clone()));
            }
        }
        for tr in &t.trailing {
            push_trivia(&mut out, tr.kind, &tr.text);
        }
    }
    out
}

/// A comma immediately preceding a closing bracket/brace/paren is
/// semantically inert (trailing comma); the formatter may add or drop them.
fn is_inert_trailing_comma(tokens: &[Token], i: usize) -> bool {
    tokens[i].kind == TokenKind::Comma
        && matches!(
            tokens.get(i + 1).map(|t| t.kind),
            Some(TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace)
        )
}

fn push_trivia(out: &mut Vec<Item>, kind: TriviaKind, text: &str) {
    match kind {
        TriviaKind::Whitespace => {}
        TriviaKind::LineComment | TriviaKind::BlockComment | TriviaKind::DocComment => {
            out.push(Item::Comment(normalize_ws(text)));
        }
        TriviaKind::Directive => out.push(Item::Directive(normalize_ws(text))),
    }
}

/// Comments are compared with whitespace runs collapsed, since the formatter
/// may re-indent block-comment continuation lines.
fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            in_ws = true;
        } else {
            if in_ws && !out.is_empty() {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    out
}
