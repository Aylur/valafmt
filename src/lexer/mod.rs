pub mod cursor;
pub mod token;

use cursor::Cursor;
use token::{Keyword, Token, TokenKind, Trivia, TriviaKind};

#[derive(Debug, Clone)]
pub struct LexError {
    pub offset: usize,
    pub message: String,
}

pub struct LexResult {
    /// Always ends with an `Eof` token (which may carry leading trivia).
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

pub fn lex(src: &str) -> LexResult {
    Lexer::new(src).run()
}

struct Lexer<'a> {
    cur: Cursor<'a>,
    src: &'a str,
    tokens: Vec<Token>,
    errors: Vec<LexError>,
    /// Trivia scanned past during trailing-trivia lookahead that actually
    /// belongs to the next token's leading trivia (multi-line block comments).
    pending_leading: Vec<Trivia>,
}

impl<'a> Lexer<'a> {
    fn new(src: &'a str) -> Self {
        Lexer {
            cur: Cursor::new(src),
            src,
            tokens: Vec::new(),
            errors: Vec::new(),
            pending_leading: Vec::new(),
        }
    }

    fn error(&mut self, offset: usize, message: impl Into<String>) {
        self.errors.push(LexError { offset, message: message.into() });
    }

    fn run(mut self) -> LexResult {
        let mut leading = self.scan_trivia(true);
        loop {
            let offset = self.cur.pos();
            if self.cur.is_eof() {
                self.tokens.push(Token {
                    kind: TokenKind::Eof,
                    text: String::new(),
                    offset,
                    leading,
                    trailing: Vec::new(),
                });
                break;
            }
            let kind = self.scan_token();
            let text = self.cur.slice_from(offset).to_string();
            let mut trailing = self.scan_trailing_trivia();
            // A trailing line comment on a token that doesn't end a
            // syntactic unit (`;`, braces, `,`, `:`) would be reordered by
            // the formatter's line-suffix machinery; relocate it to the next
            // token's leading trivia so it prints on its own line instead.
            if !matches!(
                kind,
                TokenKind::Semicolon
                    | TokenKind::LBrace
                    | TokenKind::RBrace
                    | TokenKind::Comma
                    | TokenKind::Colon
            ) {
                if let Some(idx) =
                    trailing.iter().position(|t| t.kind == TriviaKind::LineComment)
                {
                    let moved: Vec<Trivia> = trailing.split_off(idx);
                    let mut new_pending = moved;
                    new_pending.append(&mut self.pending_leading);
                    self.pending_leading = new_pending;
                }
            }
            self.tokens.push(Token { kind, text, offset, leading, trailing });
            leading = std::mem::take(&mut self.pending_leading);
            leading.extend(self.scan_trivia(false));
        }
        LexResult { tokens: self.tokens, errors: self.errors }
    }

    /// Scan trivia in leading position: whitespace runs, comments on their own
    /// lines, and preprocessor directives. `at_file_start` seeds line-start
    /// tracking for directive detection.
    fn scan_trivia(&mut self, at_file_start: bool) -> Vec<Trivia> {
        let mut out = Vec::new();
        // `#` starts a directive iff only whitespace precedes it on its line.
        // In leading position that holds whenever the last piece of whitespace
        // contained a newline (or we are at the very start of the file).
        let mut at_line_start = at_file_start;
        loop {
            let start = self.cur.pos();
            match self.cur.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    let mut saw_newline = false;
                    while matches!(self.cur.peek(), b' ' | b'\t' | b'\r' | b'\n') {
                        if self.cur.peek() == b'\n' {
                            saw_newline = true;
                        }
                        self.cur.bump();
                    }
                    if saw_newline {
                        at_line_start = true;
                    }
                    out.push(Trivia {
                        kind: TriviaKind::Whitespace,
                        text: self.cur.slice_from(start).to_string(),
                    });
                }
                0xEF if self.cur.starts_with("\u{FEFF}") => {
                    // Byte-order mark: fold into whitespace trivia.
                    self.cur.bump_char();
                    out.push(Trivia {
                        kind: TriviaKind::Whitespace,
                        text: self.cur.slice_from(start).to_string(),
                    });
                }
                b'/' if self.cur.peek_at(1) == b'/' => {
                    self.scan_line_comment();
                    out.push(Trivia {
                        kind: TriviaKind::LineComment,
                        text: self.cur.slice_from(start).to_string(),
                    });
                }
                b'/' if self.cur.peek_at(1) == b'*' => {
                    let kind = self.scan_block_comment();
                    out.push(Trivia { kind, text: self.cur.slice_from(start).to_string() });
                }
                b'#' if at_line_start => {
                    while !self.cur.is_eof() && self.cur.peek() != b'\n' {
                        self.cur.bump_char();
                    }
                    let mut text = self.cur.slice_from(start).to_string();
                    // Keep the directive text free of the line's trailing '\r'.
                    if text.ends_with('\r') {
                        text.pop();
                        // Give the '\r' back so it joins the following
                        // whitespace run and round-tripping stays byte-exact.
                        // (Cursor cannot rewind; emit it as its own run.)
                        out.push(Trivia { kind: TriviaKind::Directive, text });
                        out.push(Trivia { kind: TriviaKind::Whitespace, text: "\r".into() });
                        continue;
                    }
                    out.push(Trivia { kind: TriviaKind::Directive, text });
                }
                _ => break,
            }
        }
        out
    }

    /// Scan trailing trivia after a token: pieces on the same line only.
    /// A whitespace run containing a newline (and everything after it) belongs
    /// to the next token's leading trivia, so we stop before consuming it.
    fn scan_trailing_trivia(&mut self) -> Vec<Trivia> {
        let mut out = Vec::new();
        loop {
            let start = self.cur.pos();
            match self.cur.peek() {
                b' ' | b'\t' | b'\r' => {
                    // Only consume if the run does not reach a newline; peek ahead.
                    let mut n = 0;
                    while matches!(self.cur.peek_at(n), b' ' | b'\t' | b'\r') {
                        n += 1;
                    }
                    if self.cur.peek_at(n) == b'\n' {
                        break;
                    }
                    for _ in 0..n {
                        self.cur.bump();
                    }
                    out.push(Trivia {
                        kind: TriviaKind::Whitespace,
                        text: self.cur.slice_from(start).to_string(),
                    });
                }
                b'/' if self.cur.peek_at(1) == b'/' => {
                    self.scan_line_comment();
                    out.push(Trivia {
                        kind: TriviaKind::LineComment,
                        text: self.cur.slice_from(start).to_string(),
                    });
                }
                b'/' if self.cur.peek_at(1) == b'*' => {
                    // A block comment that spans lines belongs to the next
                    // token; scan ahead without committing.
                    let save = start;
                    let kind = self.scan_block_comment();
                    let text = self.cur.slice_from(save).to_string();
                    if text.contains('\n') {
                        self.pending_leading.push(Trivia { kind, text });
                        break;
                    }
                    out.push(Trivia { kind, text });
                }
                _ => break,
            }
        }
        out
    }

    fn scan_line_comment(&mut self) {
        while !self.cur.is_eof() && self.cur.peek() != b'\n' {
            self.cur.bump_char();
        }
        // Do not include a trailing '\r' in the comment text.
        // (Handled implicitly: '\r' before '\n' is part of the comment run
        // here; comments are printed verbatim minus trailing whitespace by
        // the formatter, so this is harmless.)
    }

    fn scan_block_comment(&mut self) -> TriviaKind {
        let start = self.cur.pos();
        self.cur.bump(); // '/'
        self.cur.bump(); // '*'
        let is_doc = self.cur.peek() == b'*' && self.cur.peek_at(1) != b'/';
        loop {
            if self.cur.is_eof() {
                self.error(start, "unterminated block comment");
                break;
            }
            if self.cur.peek() == b'*' && self.cur.peek_at(1) == b'/' {
                self.cur.bump();
                self.cur.bump();
                break;
            }
            self.cur.bump_char();
        }
        if is_doc {
            TriviaKind::DocComment
        } else {
            TriviaKind::BlockComment
        }
    }

    fn prev_significant_kind(&self) -> Option<TokenKind> {
        self.tokens.last().map(|t| t.kind)
    }

    fn scan_token(&mut self) -> TokenKind {
        use TokenKind::*;
        let start = self.cur.pos();
        let b = self.cur.peek();
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.scan_ident(false),
            b'0'..=b'9' => self.scan_number(),
            b'@' => {
                self.cur.bump();
                match self.cur.peek() {
                    b'"' => self.scan_template_string(),
                    // Escaped identifiers may start with a digit: `@2BUTTON_PRESS`.
                    b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9' => self.scan_ident(true),
                    _ => {
                        self.error(start, "stray '@'");
                        self.cur.bump_char();
                        Error
                    }
                }
            }
            b'"' => {
                if self.cur.starts_with("\"\"\"") {
                    self.scan_verbatim_string()
                } else {
                    self.scan_string()
                }
            }
            b'\'' => self.scan_char(),
            b'/' => {
                let prev = self.prev_significant_kind();
                let is_division = prev.map(|k| k.can_end_expression()).unwrap_or(false);
                if is_division {
                    self.cur.bump();
                    if self.cur.eat(b'=') {
                        SlashEq
                    } else {
                        Slash
                    }
                } else {
                    self.scan_regex()
                }
            }
            b'{' => {
                self.cur.bump();
                LBrace
            }
            b'}' => {
                self.cur.bump();
                RBrace
            }
            b'(' => {
                self.cur.bump();
                LParen
            }
            b')' => {
                self.cur.bump();
                RParen
            }
            b'[' => {
                self.cur.bump();
                LBracket
            }
            b']' => {
                self.cur.bump();
                RBracket
            }
            b'.' => {
                self.cur.bump();
                if self.cur.starts_with("..") {
                    self.cur.bump();
                    self.cur.bump();
                    DotDotDot
                } else {
                    Dot
                }
            }
            b',' => {
                self.cur.bump();
                Comma
            }
            b':' => {
                self.cur.bump();
                if self.cur.eat(b':') {
                    ColonColon
                } else {
                    Colon
                }
            }
            b';' => {
                self.cur.bump();
                Semicolon
            }
            b'?' => {
                self.cur.bump();
                if self.cur.eat(b'?') {
                    QuestionQuestion
                } else if self.cur.eat(b'.') {
                    // Null-safe member access `a?.b`. A `?` directly followed
                    // by `.` can't be anything else (`.5` literals and
                    // leading-dot expressions don't exist in Vala).
                    QuestionDot
                } else {
                    Question
                }
            }
            b'=' => {
                self.cur.bump();
                if self.cur.eat(b'=') {
                    Eq
                } else if self.cur.eat(b'>') {
                    FatArrow
                } else {
                    Assign
                }
            }
            b'!' => {
                self.cur.bump();
                if self.cur.eat(b'=') {
                    NotEq
                } else {
                    Bang
                }
            }
            b'<' => {
                self.cur.bump();
                if self.cur.eat(b'=') {
                    LtEq
                } else if self.cur.eat(b'<') {
                    if self.cur.eat(b'=') {
                        ShlEq
                    } else {
                        Shl
                    }
                } else {
                    Lt
                }
            }
            b'>' => {
                self.cur.bump();
                if self.cur.eat(b'=') {
                    GtEq
                } else {
                    Gt
                }
            }
            b'+' => {
                self.cur.bump();
                if self.cur.eat(b'+') {
                    PlusPlus
                } else if self.cur.eat(b'=') {
                    PlusEq
                } else {
                    Plus
                }
            }
            b'-' => {
                self.cur.bump();
                if self.cur.eat(b'-') {
                    MinusMinus
                } else if self.cur.eat(b'=') {
                    MinusEq
                } else if self.cur.eat(b'>') {
                    Arrow
                } else {
                    Minus
                }
            }
            b'*' => {
                self.cur.bump();
                if self.cur.eat(b'=') {
                    StarEq
                } else {
                    Star
                }
            }
            b'%' => {
                self.cur.bump();
                if self.cur.eat(b'=') {
                    PercentEq
                } else {
                    Percent
                }
            }
            b'&' => {
                self.cur.bump();
                if self.cur.eat(b'&') {
                    AmpAmp
                } else if self.cur.eat(b'=') {
                    AmpEq
                } else {
                    Amp
                }
            }
            b'|' => {
                self.cur.bump();
                if self.cur.eat(b'|') {
                    PipePipe
                } else if self.cur.eat(b'=') {
                    PipeEq
                } else {
                    Pipe
                }
            }
            b'^' => {
                self.cur.bump();
                if self.cur.eat(b'=') {
                    CaretEq
                } else {
                    Caret
                }
            }
            b'~' => {
                self.cur.bump();
                Tilde
            }
            b'#' => {
                self.cur.bump();
                self.error(start, "'#' directive must start at the beginning of a line");
                Hash
            }
            _ => {
                self.error(start, format!("unexpected character {:?}", &self.src[start..].chars().next().unwrap_or('\u{FFFD}')));
                self.cur.bump_char();
                Error
            }
        }
    }

    fn scan_ident(&mut self, escaped: bool) -> TokenKind {
        let start = self.cur.pos();
        while matches!(self.cur.peek(), b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_') {
            self.cur.bump();
        }
        if escaped {
            return TokenKind::Ident;
        }
        match Keyword::from_str(self.cur.slice_from(start)) {
            Some(kw) => TokenKind::Kw(kw),
            None => TokenKind::Ident,
        }
    }

    fn scan_number(&mut self) -> TokenKind {
        // Identifier-like names that start with digits appear in real code
        // (`GDesktop.ClockFormat.24H`, enum value `24HR`): if the digits run
        // into identifier characters that don't form a numeric literal, lex
        // the whole run as an identifier.
        {
            let mut n = 0;
            while self.cur.peek_at(n).is_ascii_digit() {
                n += 1;
            }
            let next = self.cur.peek_at(n);
            let is_identish = matches!(next, b'a'..=b'z' | b'A'..=b'Z' | b'_')
                && !matches!(next, b'x' | b'X' | b'e' | b'E' | b'u' | b'U' | b'l' | b'L' | b'f' | b'F' | b'd' | b'D');
            if is_identish {
                while matches!(
                    self.cur.peek(),
                    b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_'
                ) {
                    self.cur.bump();
                }
                return TokenKind::Ident;
            }
        }
        if self.cur.peek() == b'0' && matches!(self.cur.peek_at(1), b'x' | b'X') {
            self.cur.bump();
            self.cur.bump();
            while self.cur.peek().is_ascii_hexdigit() {
                self.cur.bump();
            }
            self.scan_int_suffix();
            return TokenKind::IntLit;
        }
        while self.cur.peek().is_ascii_digit() {
            self.cur.bump();
        }
        let mut real = false;
        if self.cur.peek() == b'.' && self.cur.peek_at(1).is_ascii_digit() {
            real = true;
            self.cur.bump();
            while self.cur.peek().is_ascii_digit() {
                self.cur.bump();
            }
        }
        if matches!(self.cur.peek(), b'e' | b'E') {
            let mut n = 1;
            if matches!(self.cur.peek_at(1), b'+' | b'-') {
                n = 2;
            }
            if self.cur.peek_at(n).is_ascii_digit() {
                real = true;
                for _ in 0..n {
                    self.cur.bump();
                }
                while self.cur.peek().is_ascii_digit() {
                    self.cur.bump();
                }
            }
        }
        if matches!(self.cur.peek(), b'f' | b'F' | b'd' | b'D') {
            self.cur.bump();
            return TokenKind::RealLit;
        }
        if real {
            TokenKind::RealLit
        } else {
            self.scan_int_suffix();
            TokenKind::IntLit
        }
    }

    fn scan_int_suffix(&mut self) {
        while matches!(self.cur.peek(), b'u' | b'U' | b'l' | b'L') {
            self.cur.bump();
        }
    }

    fn scan_char(&mut self) -> TokenKind {
        let start = self.cur.pos();
        self.cur.bump(); // '
        loop {
            match self.cur.peek() {
                0 if self.cur.is_eof() => {
                    self.error(start, "unterminated character literal");
                    return TokenKind::Error;
                }
                b'\\' => {
                    self.cur.bump();
                    self.cur.bump_char();
                }
                b'\'' => {
                    self.cur.bump();
                    return TokenKind::CharLit;
                }
                b'\n' => {
                    self.error(start, "unterminated character literal");
                    return TokenKind::Error;
                }
                _ => self.cur.bump_char(),
            }
        }
    }

    fn scan_string(&mut self) -> TokenKind {
        let start = self.cur.pos();
        self.cur.bump(); // "
        loop {
            match self.cur.peek() {
                0 if self.cur.is_eof() => {
                    self.error(start, "unterminated string literal");
                    return TokenKind::Error;
                }
                b'\\' => {
                    self.cur.bump();
                    self.cur.bump_char();
                }
                b'"' => {
                    self.cur.bump();
                    return TokenKind::StringLit;
                }
                b'\n' => {
                    self.error(start, "unterminated string literal");
                    return TokenKind::Error;
                }
                _ => self.cur.bump_char(),
            }
        }
    }

    fn scan_verbatim_string(&mut self) -> TokenKind {
        let start = self.cur.pos();
        self.cur.bump();
        self.cur.bump();
        self.cur.bump(); // """
        loop {
            if self.cur.is_eof() {
                self.error(start, "unterminated verbatim string literal");
                return TokenKind::Error;
            }
            if self.cur.starts_with("\"\"\"") {
                // In a quote run longer than 3, the extra quotes belong to
                // the content and the *last* three close the literal
                // (`""""""" ` ends content `"` with closing `"""`).
                while self.cur.peek_at(3) == b'"' {
                    self.cur.bump();
                }
                self.cur.bump();
                self.cur.bump();
                self.cur.bump();
                return TokenKind::VerbatimStringLit;
            }
            self.cur.bump_char();
        }
    }

    /// `@"..."` or `@"""..."""`; the leading `@` has already been consumed.
    /// Interpolations `$identifier` and `$(expression)` are consumed raw, with
    /// balanced-paren tracking that skips over nested string/char literals.
    fn scan_template_string(&mut self) -> TokenKind {
        let start = self.cur.pos();
        let verbatim = self.cur.starts_with("\"\"\"");
        if verbatim {
            self.cur.bump();
            self.cur.bump();
            self.cur.bump();
        } else {
            self.cur.bump(); // "
        }
        loop {
            if self.cur.is_eof() {
                self.error(start, "unterminated string template");
                return TokenKind::Error;
            }
            if verbatim {
                if self.cur.starts_with("\"\"\"") {
                    self.cur.bump();
                    self.cur.bump();
                    self.cur.bump();
                    return TokenKind::TemplateStringLit;
                }
            } else {
                match self.cur.peek() {
                    b'"' => {
                        self.cur.bump();
                        return TokenKind::TemplateStringLit;
                    }
                    // valac tolerates raw newlines inside templates.
                    b'\\' => {
                        self.cur.bump();
                        self.cur.bump_char();
                        continue;
                    }
                    _ => {}
                }
            }
            if self.cur.peek() == b'$' && self.cur.peek_at(1) == b'(' {
                self.cur.bump();
                self.cur.bump();
                self.consume_balanced_parens(start);
                continue;
            }
            self.cur.bump_char();
        }
    }

    /// Consume up to and including the ')' matching an already-consumed '(',
    /// skipping nested parens and string/char literals.
    fn consume_balanced_parens(&mut self, err_offset: usize) {
        let mut depth = 1usize;
        while depth > 0 {
            if self.cur.is_eof() {
                self.error(err_offset, "unterminated interpolation in string template");
                return;
            }
            match self.cur.peek() {
                b'(' => {
                    depth += 1;
                    self.cur.bump();
                }
                b')' => {
                    depth -= 1;
                    self.cur.bump();
                }
                b'"' => {
                    if self.cur.starts_with("\"\"\"") {
                        self.scan_verbatim_string();
                    } else {
                        self.scan_string();
                    }
                }
                b'\'' => {
                    self.scan_char();
                }
                _ => self.cur.bump_char(),
            }
        }
    }

    fn scan_regex(&mut self) -> TokenKind {
        let start = self.cur.pos();
        self.cur.bump(); // /
        loop {
            match self.cur.peek() {
                0 if self.cur.is_eof() => {
                    self.error(start, "unterminated regex literal");
                    return TokenKind::Error;
                }
                b'\\' => {
                    self.cur.bump();
                    self.cur.bump_char();
                }
                b'/' => {
                    self.cur.bump();
                    while matches!(self.cur.peek(), b'i' | b's' | b'm' | b'x') {
                        self.cur.bump();
                    }
                    return TokenKind::RegexLit;
                }
                b'\n' => {
                    self.error(start, "unterminated regex literal");
                    return TokenKind::Error;
                }
                _ => self.cur.bump_char(),
            }
        }
    }
}
