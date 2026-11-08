use valafmt::lexer::{self, token::TokenKind};

fn reassemble(src: &str) -> String {
    let result = lexer::lex(src);
    assert!(
        result.errors.is_empty(),
        "lex errors on {src:?}: {:?}",
        result.errors
    );
    let mut out = String::new();
    for tok in &result.tokens {
        for t in &tok.leading {
            out.push_str(&t.text);
        }
        out.push_str(&tok.text);
        for t in &tok.trailing {
            out.push_str(&t.text);
        }
    }
    out
}

fn assert_roundtrip(src: &str) {
    assert_eq!(reassemble(src), src, "lexer did not round-trip");
}

fn kinds(src: &str) -> Vec<TokenKind> {
    let result = lexer::lex(src);
    assert!(
        result.errors.is_empty(),
        "lex errors on {src:?}: {:?}",
        result.errors
    );
    result
        .tokens
        .iter()
        .map(|t| t.kind)
        .filter(|k| *k != TokenKind::Eof)
        .collect()
}

#[test]
fn roundtrip_basic() {
    assert_roundtrip("int main() {\n    return 0;\n}\n");
    assert_roundtrip("");
    assert_roundtrip("\n\n\n");
    assert_roundtrip("// only a comment\n");
    assert_roundtrip("/* block */ /** doc */\n");
}

#[test]
fn roundtrip_comments_and_blanks() {
    assert_roundtrip("int a; // trailing\n\n// leading\nint b;\n");
    assert_roundtrip("foo(); /* same\nline */ bar();\n");
}

#[test]
fn roundtrip_directives() {
    assert_roundtrip("#if X\nint a;\n#else\nint b;\n#endif\n");
    assert_roundtrip("  #if INDENTED\n#endif\n");
}

#[test]
fn roundtrip_strings() {
    assert_roundtrip(r#"var s = "hi \"there\"";"#);
    assert_roundtrip("var s = \"\"\"verbatim \" with quote\nand newline\"\"\";\n");
    assert_roundtrip(r#"var s = @"x = $(foo(bar, "nested (paren")) and $name";"#);
    assert_roundtrip("var s = @\"\"\"verbatim $(template)\"\"\";\n");
    assert_roundtrip(r"var c = '\n'; var d = '\''; var e = 'x';");
}

#[test]
fn roundtrip_numbers() {
    assert_roundtrip("var a = 0x1Fu; var b = 10UL; var c = 1.5e-3f; var d = 2.0; var e = 3f;");
    assert_roundtrip("var f = 1.to_string();");
}

#[test]
fn number_member_access_is_not_a_real() {
    use TokenKind::*;
    assert_eq!(kinds("1.to_string"), vec![IntLit, Dot, Ident]);
}

#[test]
fn regex_vs_division() {
    use TokenKind::*;
    assert_eq!(kinds("a / b"), vec![Ident, Slash, Ident]);
    assert_eq!(
        kinds("x = /ab+c/i;"),
        vec![Ident, Assign, RegexLit, Semicolon]
    );
    assert_eq!(kinds("foo(/a\\/b/)"), vec![Ident, LParen, RegexLit, RParen]);
    assert_eq!(kinds("(a) / b"), vec![LParen, Ident, RParen, Slash, Ident]);
    assert_eq!(
        kinds("a[0] / b"),
        vec![Ident, LBracket, IntLit, RBracket, Slash, Ident]
    );
    assert_eq!(
        kinds("return /x/;"),
        vec![Kw(lexer::token::Keyword::Return), RegexLit, Semicolon]
    );
}

#[test]
fn gt_gt_tokens() {
    use TokenKind::*;
    assert_eq!(kinds("a >> b"), vec![Ident, Gt, Gt, Ident]);
    assert_eq!(kinds("a >>= b"), vec![Ident, Gt, GtEq, Ident]);
    assert_eq!(kinds("a << b"), vec![Ident, Shl, Ident]);
    assert_eq!(kinds("a <<= b"), vec![Ident, ShlEq, Ident]);
}

#[test]
fn operators() {
    use TokenKind::*;
    assert_eq!(kinds("a ?? b"), vec![Ident, QuestionQuestion, Ident]);
    assert_eq!(kinds("a?"), vec![Ident, Question]);
    assert_eq!(
        kinds("() => {}"),
        vec![LParen, RParen, FatArrow, LBrace, RBrace]
    );
    assert_eq!(kinds("p->x"), vec![Ident, Arrow, Ident]);
    assert_eq!(
        kinds("global::Posix"),
        vec![Kw(lexer::token::Keyword::Global), ColonColon, Ident]
    );
    assert_eq!(kinds("f(...)"), vec![Ident, LParen, DotDotDot, RParen]);
}

#[test]
fn escaped_identifier() {
    let result = lexer::lex("var @foreach = 1;");
    assert!(result.errors.is_empty());
    assert_eq!(result.tokens[1].kind, TokenKind::Ident);
    assert_eq!(result.tokens[1].text, "@foreach");
}

#[test]
fn build_placeholder() {
    let result = lexer::lex("var v = @VERSION@;");
    assert!(result.errors.is_empty());
    assert_eq!(result.tokens[3].kind, TokenKind::Ident);
    assert_eq!(result.tokens[3].text, "@VERSION@");
}

#[test]
fn blank_line_counting() {
    let result = lexer::lex("int a;\n\n\n\nint b;\n");
    let b_tok = result
        .tokens
        .iter()
        .find(|t| t.text == "int" && t.offset > 0)
        .unwrap();
    assert!(b_tok.leading_newlines() >= 2);
}

#[test]
fn trailing_comment_attachment() {
    let result = lexer::lex("int a; // hello\nint b;\n");
    let semi = result
        .tokens
        .iter()
        .find(|t| t.kind == TokenKind::Semicolon)
        .unwrap();
    assert!(semi.has_trailing_comment());
}
