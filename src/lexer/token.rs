use std::fmt;

/// A keyword recognized by the Vala scanner. Some of these are contextual and
/// may be accepted as identifiers by the parser (e.g. `get`, `set`, `value`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Keyword {
    Abstract,
    As,
    Async,
    Base,
    Break,
    Case,
    Catch,
    Class,
    Const,
    Construct,
    Continue,
    Default,
    Delegate,
    Delete,
    Do,
    Dynamic,
    Else,
    Ensures,
    Enum,
    Errordomain,
    Extern,
    False,
    Finally,
    For,
    Foreach,
    Get,
    Global,
    If,
    In,
    Inline,
    Interface,
    Internal,
    Is,
    Lock,
    Namespace,
    New,
    Null,
    Out,
    Override,
    Owned,
    Params,
    Partial,
    Private,
    Protected,
    Public,
    Ref,
    Requires,
    Return,
    Sealed,
    Set,
    Signal,
    Sizeof,
    Static,
    Struct,
    Switch,
    This,
    Throw,
    Throws,
    True,
    Try,
    Typeof,
    Unlock,
    Unowned,
    Using,
    Value,
    Var,
    Virtual,
    Void,
    Volatile,
    Weak,
    While,
    With,
    Yield,
}

impl Keyword {
    pub fn from_str(s: &str) -> Option<Keyword> {
        use Keyword::*;
        Some(match s {
            "abstract" => Abstract,
            "as" => As,
            "async" => Async,
            "base" => Base,
            "break" => Break,
            "case" => Case,
            "catch" => Catch,
            "class" => Class,
            "const" => Const,
            "construct" => Construct,
            "continue" => Continue,
            "default" => Default,
            "delegate" => Delegate,
            "delete" => Delete,
            "do" => Do,
            "dynamic" => Dynamic,
            "else" => Else,
            "ensures" => Ensures,
            "enum" => Enum,
            "errordomain" => Errordomain,
            "extern" => Extern,
            "false" => False,
            "finally" => Finally,
            "for" => For,
            "foreach" => Foreach,
            "get" => Get,
            "global" => Global,
            "if" => If,
            "in" => In,
            "inline" => Inline,
            "interface" => Interface,
            "internal" => Internal,
            "is" => Is,
            "lock" => Lock,
            "namespace" => Namespace,
            "new" => New,
            "null" => Null,
            "out" => Out,
            "override" => Override,
            "owned" => Owned,
            "params" => Params,
            "partial" => Partial,
            "private" => Private,
            "protected" => Protected,
            "public" => Public,
            "ref" => Ref,
            "requires" => Requires,
            "return" => Return,
            "sealed" => Sealed,
            "set" => Set,
            "signal" => Signal,
            "sizeof" => Sizeof,
            "static" => Static,
            "struct" => Struct,
            "switch" => Switch,
            "this" => This,
            "throw" => Throw,
            "throws" => Throws,
            "true" => True,
            "try" => Try,
            "typeof" => Typeof,
            "unlock" => Unlock,
            "unowned" => Unowned,
            "using" => Using,
            "value" => Value,
            "var" => Var,
            "virtual" => Virtual,
            "void" => Void,
            "volatile" => Volatile,
            "weak" => Weak,
            "while" => While,
            "with" => With,
            "yield" => Yield,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Ident,
    IntLit,
    RealLit,
    CharLit,
    StringLit,
    /// `"""..."""` — printed byte-exact, may span lines.
    VerbatimStringLit,
    /// `@"..."` or `@"""..."""` — kept as one raw token, printed verbatim.
    TemplateStringLit,
    /// `/pattern/flags` — context-dependent, kept raw.
    RegexLit,
    Kw(Keyword),

    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Dot,
    DotDotDot,
    Comma,
    Colon,
    ColonColon,
    Semicolon,
    Question,
    QuestionQuestion,
    /// `?.` null-safe member access.
    QuestionDot,
    Arrow,    // ->
    FatArrow, // =>
    Assign,
    Eq,
    NotEq,
    Lt,
    LtEq,
    // NB: `>>` is deliberately lexed as two `Gt` tokens (as valac does) so that
    // nested generic argument lists like `List<List<int>>` close correctly; the
    // parser recombines adjacent `Gt Gt` into a shift where appropriate.
    Gt,
    GtEq,
    Shl, // <<
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    PlusPlus,
    MinusMinus,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    AmpEq,
    PipeEq,
    CaretEq,
    ShlEq,
    Amp,
    Pipe,
    Caret,
    Tilde,
    Bang,
    AmpAmp,
    PipePipe,
    Hash, // only reachable on malformed input; directives are trivia

    Eof,
    Error,
}

impl TokenKind {
    /// True if a `/` following a token of this kind is a division operator
    /// rather than the start of a regex literal.
    pub fn can_end_expression(self) -> bool {
        use TokenKind::*;
        matches!(
            self,
            Ident
                | IntLit
                | RealLit
                | CharLit
                | StringLit
                | VerbatimStringLit
                | TemplateStringLit
                | RegexLit
                | RParen
                | RBracket
                | PlusPlus
                | MinusMinus
                | Kw(Keyword::This)
                | Kw(Keyword::Base)
                | Kw(Keyword::True)
                | Kw(Keyword::False)
                | Kw(Keyword::Null)
                | Kw(Keyword::Value)
                | Kw(Keyword::Get)
                | Kw(Keyword::Set)
                | Kw(Keyword::Default)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaKind {
    /// A run of whitespace, stored byte-exact. `newlines()` on the trivia
    /// reports how many line breaks it contains.
    Whitespace,
    LineComment,
    BlockComment,
    /// `/** ... */` valadoc comment.
    DocComment,
    /// A whole `#if` / `#elif` / `#else` / `#endif` line, kept raw
    /// (without its trailing newline).
    Directive,
}

#[derive(Debug, Clone)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub text: String,
    /// Byte offset of this trivia in the original source.
    pub offset: usize,
}

impl Trivia {
    pub fn newlines(&self) -> usize {
        self.text.bytes().filter(|&b| b == b'\n').count()
    }

    pub fn is_comment(&self) -> bool {
        matches!(
            self.kind,
            TriviaKind::LineComment | TriviaKind::BlockComment | TriviaKind::DocComment
        )
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub text: String,
    /// Byte offset of the token text in the original source.
    pub offset: usize,
    /// Everything between the previous token's trailing trivia and this token:
    /// whitespace (with newline info), comments on their own lines, directives.
    pub leading: Vec<Trivia>,
    /// Same-line trivia after the token, up to but not including the newline.
    pub trailing: Vec<Trivia>,
}

impl Token {
    pub fn is(&self, kind: TokenKind) -> bool {
        self.kind == kind
    }

    pub fn is_kw(&self, kw: Keyword) -> bool {
        self.kind == TokenKind::Kw(kw)
    }

    /// Number of line breaks in the leading trivia (used for blank-line
    /// preservation: >= 2 means the source had at least one blank line).
    pub fn leading_newlines(&self) -> usize {
        self.leading.iter().map(|t| t.newlines()).sum()
    }

    pub fn has_leading_comment(&self) -> bool {
        self.leading
            .iter()
            .any(|t| t.is_comment() || t.kind == TriviaKind::Directive)
    }

    pub fn has_trailing_comment(&self) -> bool {
        self.trailing.iter().any(|t| t.is_comment())
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}
