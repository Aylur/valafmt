//! Typed concrete syntax tree for Vala. Every token consumed by the parser is
//! stored in the tree (Roslyn-style), so trivia (comments, blank lines,
//! preprocessor directives) is never lost: the formatter emits trivia as part
//! of emitting each token.

use crate::lexer::token::Token;

#[derive(Debug)]
pub struct File {
    pub members: Vec<Member>,
    pub eof: Token,
}

/// A `,`-separated list where separators are kept.
#[derive(Debug)]
pub struct Punctuated<T> {
    pub items: Vec<T>,
    /// Separator tokens; `commas.len()` is `items.len() - 1`, or equal to
    /// `items.len()` when the source had a trailing separator.
    pub commas: Vec<Token>,
}

impl<T> Punctuated<T> {
    pub fn new() -> Self {
        Punctuated { items: Vec::new(), commas: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

impl<T> Default for Punctuated<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// `global::`? ident (`.` ident)* — qualified symbol name; the tokens
/// alternate name/dot. Also used for member types like `Gee.ArrayList`.
#[derive(Debug)]
pub struct QualifiedName {
    /// `global` keyword and `::` token, if present.
    pub global: Option<(Token, Token)>,
    /// name, dot, name, dot, ... (odd positions are `.` tokens)
    pub parts: Vec<Token>,
}

// ---------------------------------------------------------------------------
// Attributes
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct AttributeList {
    pub l_bracket: Token,
    pub attrs: Punctuated<Attribute>,
    pub r_bracket: Token,
}

#[derive(Debug)]
pub struct Attribute {
    pub name: Token,
    pub args: Option<AttributeArgs>,
}

#[derive(Debug)]
pub struct AttributeArgs {
    pub l_paren: Token,
    pub args: Punctuated<AttributeArg>,
    pub r_paren: Token,
}

#[derive(Debug)]
pub struct AttributeArg {
    pub name: Token,
    pub eq: Token,
    pub value: Expr,
}

// ---------------------------------------------------------------------------
// Declarations
// ---------------------------------------------------------------------------

/// A namespace/class/interface/etc. member.
#[derive(Debug)]
pub struct Member {
    pub attributes: Vec<AttributeList>,
    pub kind: MemberKind,
}

#[derive(Debug)]
pub enum MemberKind {
    Using(UsingDirective),
    Namespace(NamespaceDecl),
    Class(TypeDecl),
    Interface(TypeDecl),
    Struct(TypeDecl),
    Enum(EnumDecl),
    Errordomain(EnumDecl),
    Delegate(DelegateDecl),
    Constant(ConstantDecl),
    Field(FieldDecl),
    Method(MethodDecl),
    /// Creation method: `public Foo ()`, `public Foo.variant ()`.
    CreationMethod(CreationMethodDecl),
    Property(PropertyDecl),
    Signal(SignalDecl),
    Constructor(ConstructorDecl),
    Destructor(DestructorDecl),
    /// A stray `;` after a declaration (tolerated, dropped on output).
    Empty(Token),
}

#[derive(Debug)]
pub struct UsingDirective {
    pub using_kw: Token,
    pub names: Punctuated<QualifiedName>,
    pub semicolon: Token,
}

#[derive(Debug)]
pub struct NamespaceDecl {
    pub modifiers: Vec<Token>,
    pub namespace_kw: Token,
    pub name: QualifiedName,
    pub body: DeclBody,
}

/// class / interface / struct declaration.
#[derive(Debug)]
pub struct TypeDecl {
    pub modifiers: Vec<Token>,
    pub keyword: Token,
    pub name: QualifiedName,
    pub type_params: Option<TypeParamList>,
    pub base: Option<BaseClause>,
    pub body: DeclBody,
}

#[derive(Debug)]
pub struct BaseClause {
    pub colon: Token,
    pub types: Punctuated<Type>,
}

#[derive(Debug)]
pub struct DeclBody {
    pub l_brace: Token,
    pub members: Vec<Member>,
    pub r_brace: Token,
}

#[derive(Debug)]
pub struct TypeParamList {
    pub lt: Token,
    pub params: Punctuated<Token>,
    pub gt: Token,
}

#[derive(Debug)]
pub struct EnumDecl {
    pub modifiers: Vec<Token>,
    pub keyword: Token, // `enum` or `errordomain`
    pub name: QualifiedName,
    pub base: Option<BaseClause>,
    pub l_brace: Token,
    pub values: Punctuated<EnumValue>,
    /// `;` separating values from further members, if present.
    pub semicolon: Option<Token>,
    pub members: Vec<Member>,
    pub r_brace: Token,
}

#[derive(Debug)]
pub struct EnumValue {
    pub attributes: Vec<AttributeList>,
    pub name: Token,
    pub value: Option<(Token, Expr)>, // `=` expr
}

#[derive(Debug)]
pub struct DelegateDecl {
    pub modifiers: Vec<Token>,
    pub delegate_kw: Token,
    pub return_type: Type,
    pub name: QualifiedName,
    pub type_params: Option<TypeParamList>,
    pub l_paren: Token,
    pub params: Punctuated<Param>,
    pub r_paren: Token,
    pub throws: Option<ThrowsClause>,
    pub semicolon: Token,
}

#[derive(Debug)]
pub struct ConstantDecl {
    pub modifiers: Vec<Token>,
    pub const_kw: Token,
    pub ty: Type,
    pub name: Token,
    pub init: Option<(Token, Expr)>,
    pub semicolon: Token,
}

#[derive(Debug)]
pub struct FieldDecl {
    pub modifiers: Vec<Token>,
    pub ty: Type,
    pub name: Token,
    pub init: Option<(Token, Expr)>,
    pub semicolon: Token,
}

#[derive(Debug)]
pub struct MethodDecl {
    pub modifiers: Vec<Token>,
    pub return_type: Type,
    /// May be dotted for explicit interface implementations
    /// (`int Iface.method ()`).
    pub name: QualifiedName,
    pub type_params: Option<TypeParamList>,
    pub l_paren: Token,
    pub params: Punctuated<Param>,
    pub r_paren: Token,
    pub throws: Option<ThrowsClause>,
    pub contracts: Vec<Contract>,
    pub body: Body,
}

#[derive(Debug)]
pub struct CreationMethodDecl {
    pub modifiers: Vec<Token>,
    /// `Foo` or `Foo.variant` (dotted).
    pub name: QualifiedName,
    pub l_paren: Token,
    pub params: Punctuated<Param>,
    pub r_paren: Token,
    pub throws: Option<ThrowsClause>,
    pub contracts: Vec<Contract>,
    pub body: Body,
}

#[derive(Debug)]
pub struct ThrowsClause {
    pub throws_kw: Token,
    pub types: Punctuated<Type>,
}

/// `requires (expr)` / `ensures (expr)`.
#[derive(Debug)]
pub struct Contract {
    pub keyword: Token,
    pub l_paren: Token,
    pub expr: Expr,
    pub r_paren: Token,
}

/// Method-like body: a block or a terminating `;`.
#[derive(Debug)]
pub enum Body {
    Block(Block),
    Semicolon(Token),
}

#[derive(Debug)]
pub struct Param {
    pub attributes: Vec<AttributeList>,
    /// `out` / `ref` / `owned` / `params` tokens.
    pub modifiers: Vec<Token>,
    pub kind: ParamKind,
}

#[derive(Debug)]
pub enum ParamKind {
    /// `...`
    Ellipsis(Token),
    Typed {
        ty: Type,
        name: Token,
        default: Option<(Token, Expr)>,
    },
}

#[derive(Debug)]
pub struct PropertyDecl {
    pub modifiers: Vec<Token>,
    pub ty: Type,
    pub name: QualifiedName,
    pub l_brace: Token,
    pub accessors: Vec<PropertyAccessor>,
    pub r_brace: Token,
}

#[derive(Debug)]
pub enum PropertyAccessor {
    /// `[owned] get [;{...}]`, `[private] set [construct] [;{...}]`,
    /// `construct set`, etc.
    Accessor {
        attributes: Vec<AttributeList>,
        modifiers: Vec<Token>, // access modifiers, `owned`
        keywords: Vec<Token>,  // `get`, `set`, `construct` in written order
        body: Body,
    },
    /// `default = expr;`
    Default {
        default_kw: Token,
        eq: Token,
        expr: Expr,
        semicolon: Token,
    },
}

#[derive(Debug)]
pub struct SignalDecl {
    pub modifiers: Vec<Token>, // includes `signal`, `virtual`, access, etc.
    pub signal_kw: Token,
    pub return_type: Type,
    pub name: Token,
    pub l_paren: Token,
    pub params: Punctuated<Param>,
    pub r_paren: Token,
    pub body: Body,
}

#[derive(Debug)]
pub struct ConstructorDecl {
    /// `class` / `static` / access modifiers before `construct`.
    pub modifiers: Vec<Token>,
    pub construct_kw: Token,
    pub body: Block,
}

#[derive(Debug)]
pub struct DestructorDecl {
    pub modifiers: Vec<Token>,
    pub tilde: Token,
    pub name: Token,
    pub l_paren: Token,
    pub r_paren: Token,
    pub body: Block,
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Type {
    /// `owned` / `unowned` / `weak` / `dynamic` / `async` (in delegate
    /// contexts) prefix tokens.
    pub prefix: Vec<Token>,
    pub core: TypeCore,
    /// Suffixes in source order: `?`, `*`, `[...]`.
    pub suffixes: Vec<TypeSuffix>,
}

#[derive(Debug)]
pub enum TypeCore {
    Named {
        name: QualifiedName,
        type_args: Option<TypeArgList>,
    },
    /// `void`
    Void(Token),
}

#[derive(Debug)]
pub struct TypeArgList {
    pub lt: Token,
    pub args: Punctuated<Type>,
    pub gt: Token,
}

#[derive(Debug)]
pub enum TypeSuffix {
    /// `?`
    Nullable(Token),
    /// `*`
    Pointer(Token),
    /// `[]`, `[,]`, `[expr]`, `[expr, expr]`
    Array {
        l_bracket: Token,
        dims: Punctuated<Option<Expr>>,
        r_bracket: Token,
        /// `?` directly after `[]` (nullable array element/etc.)
        nullable: Option<Token>,
    },
}

// ---------------------------------------------------------------------------
// Statements
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct Block {
    pub l_brace: Token,
    pub statements: Vec<Stmt>,
    pub r_brace: Token,
}

#[derive(Debug)]
pub enum Stmt {
    Block(Block),
    Empty(Token),
    /// `var x = ...;` / `Type x = ..., y;` / `const Type X = ...;`
    LocalDecl(LocalDecl),
    Expr {
        expr: Expr,
        semicolon: Token,
    },
    If(IfStmt),
    Switch(SwitchStmt),
    While(WhileStmt),
    DoWhile(DoWhileStmt),
    For(ForStmt),
    Foreach(ForeachStmt),
    Break {
        kw: Token,
        semicolon: Token,
    },
    Continue {
        kw: Token,
        semicolon: Token,
    },
    Return {
        kw: Token,
        expr: Option<Expr>,
        semicolon: Token,
    },
    Throw {
        kw: Token,
        expr: Expr,
        semicolon: Token,
    },
    Try(TryStmt),
    Lock(LockStmt),
    Unlock {
        kw: Token,
        l_paren: Token,
        expr: Expr,
        r_paren: Token,
        semicolon: Token,
    },
    Delete {
        kw: Token,
        expr: Expr,
        semicolon: Token,
    },
    Yield {
        kw: Token,
        semicolon: Token,
    },
    With(WithStmt),
}

#[derive(Debug)]
pub struct LocalDecl {
    pub const_kw: Option<Token>,
    /// `var` / `unowned var` handled as a Type with core name `var`? No —
    /// kept distinct for clarity.
    pub ty: LocalDeclType,
    pub declarators: Punctuated<Declarator>,
    pub semicolon: Token,
}

#[derive(Debug)]
pub enum LocalDeclType {
    /// `var` with optional `unowned`/`owned` prefix and `?` suffix.
    Var {
        prefix: Vec<Token>,
        var_kw: Token,
        nullable: Option<Token>,
    },
    Type(Type),
}

#[derive(Debug)]
pub struct Declarator {
    pub name: Token,
    pub init: Option<(Token, Expr)>, // `=` expr
}

#[derive(Debug)]
pub struct IfStmt {
    pub if_kw: Token,
    pub l_paren: Token,
    pub cond: Expr,
    pub r_paren: Token,
    pub then: Box<Stmt>,
    pub else_branch: Option<ElseBranch>,
}

#[derive(Debug)]
pub struct ElseBranch {
    pub else_kw: Token,
    pub body: Box<Stmt>, // Block or nested IfStmt (for `else if`)
}

#[derive(Debug)]
pub struct SwitchStmt {
    pub switch_kw: Token,
    pub l_paren: Token,
    pub expr: Expr,
    pub r_paren: Token,
    pub l_brace: Token,
    pub sections: Vec<SwitchSection>,
    pub r_brace: Token,
}

#[derive(Debug)]
pub struct SwitchSection {
    pub labels: Vec<SwitchLabel>,
    pub statements: Vec<Stmt>,
}

#[derive(Debug)]
pub enum SwitchLabel {
    Case {
        case_kw: Token,
        expr: Expr,
        colon: Token,
    },
    Default {
        default_kw: Token,
        colon: Token,
    },
}

#[derive(Debug)]
pub struct WhileStmt {
    pub while_kw: Token,
    pub l_paren: Token,
    pub cond: Expr,
    pub r_paren: Token,
    pub body: Box<Stmt>,
}

#[derive(Debug)]
pub struct DoWhileStmt {
    pub do_kw: Token,
    pub body: Box<Stmt>,
    pub while_kw: Token,
    pub l_paren: Token,
    pub cond: Expr,
    pub r_paren: Token,
    pub semicolon: Token,
}

#[derive(Debug)]
pub struct ForStmt {
    pub for_kw: Token,
    pub l_paren: Token,
    /// Either a local declaration (with its `;`) or expression list + `;`.
    pub init: ForInit,
    pub cond: Option<Expr>,
    pub semicolon2: Token,
    pub update: Punctuated<Expr>,
    pub r_paren: Token,
    pub body: Box<Stmt>,
}

#[derive(Debug)]
pub enum ForInit {
    Decl(LocalDecl),
    Exprs {
        exprs: Punctuated<Expr>,
        semicolon: Token,
    },
}

#[derive(Debug)]
pub struct ForeachStmt {
    pub foreach_kw: Token,
    pub l_paren: Token,
    pub ty: LocalDeclType,
    pub name: Token,
    pub in_kw: Token,
    pub expr: Expr,
    pub r_paren: Token,
    pub body: Box<Stmt>,
}

#[derive(Debug)]
pub struct TryStmt {
    pub try_kw: Token,
    pub body: Block,
    pub catches: Vec<CatchClause>,
    pub finally: Option<FinallyClause>,
}

#[derive(Debug)]
pub struct CatchClause {
    pub catch_kw: Token,
    /// `(Type name)`, optional (bare `catch` allowed).
    pub param: Option<CatchParam>,
    pub body: Block,
}

#[derive(Debug)]
pub struct CatchParam {
    pub l_paren: Token,
    pub ty: Type,
    pub name: Token,
    pub r_paren: Token,
}

#[derive(Debug)]
pub struct FinallyClause {
    pub finally_kw: Token,
    pub body: Block,
}

#[derive(Debug)]
pub struct LockStmt {
    pub lock_kw: Token,
    pub l_paren: Token,
    pub expr: Expr,
    pub r_paren: Token,
    /// Block or `;`.
    pub body: Body,
}

#[derive(Debug)]
pub struct WithStmt {
    pub with_kw: Token,
    pub l_paren: Token,
    /// Optional declaration: `with (var x = expr)`.
    pub decl: Option<(LocalDeclType, Token, Token)>, // (type, name, `=`)
    pub expr: Expr,
    pub r_paren: Token,
    pub body: Block,
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum Expr {
    /// Identifier or literal or `this`/`base`/`true`/`false`/`null` — one token.
    /// Multi-line literals (verbatim strings, templates) included.
    Atom(Token),
    /// `global::name`
    Global {
        global_kw: Token,
        colon_colon: Token,
        name: Token,
    },
    Paren {
        l_paren: Token,
        expr: Box<Expr>,
        r_paren: Token,
    },
    Member {
        base: Box<Expr>,
        op: Token, // `.` or `->`
        name: Token,
        /// Explicit generic member reference `foo.bar<int>` (rare).
        type_args: Option<TypeArgList>,
    },
    Call {
        callee: Box<Expr>,
        l_paren: Token,
        args: Punctuated<Arg>,
        r_paren: Token,
    },
    Index {
        base: Box<Expr>,
        l_bracket: Token,
        indices: Punctuated<Expr>,
        r_bracket: Token,
    },
    Slice {
        base: Box<Expr>,
        l_bracket: Token,
        start: Option<Box<Expr>>,
        colon: Token,
        end: Option<Box<Expr>>,
        r_bracket: Token,
    },
    PostfixIncDec {
        base: Box<Expr>,
        op: Token,
    },
    Unary {
        op: Token,
        expr: Box<Expr>,
    },
    /// `(Type) expr`
    Cast {
        l_paren: Token,
        ty: Type,
        r_paren: Token,
        expr: Box<Expr>,
    },
    /// `(!) expr`
    NonNullCast {
        l_paren: Token,
        bang: Token,
        r_paren: Token,
        expr: Box<Expr>,
    },
    /// `(owned) expr`
    OwnershipTransfer {
        l_paren: Token,
        owned_kw: Token,
        r_paren: Token,
        expr: Box<Expr>,
    },
    Binary {
        lhs: Box<Expr>,
        /// One token, or two for `>>` / `>>=` (adjacent `>` `>`).
        op: Vec<Token>,
        rhs: Box<Expr>,
    },
    /// `expr is Type` / `expr as Type`
    TypeOp {
        expr: Box<Expr>,
        op: Token,
        ty: Type,
    },
    Ternary {
        cond: Box<Expr>,
        question: Token,
        then: Box<Expr>,
        colon: Token,
        els: Box<Expr>,
    },
    Lambda {
        params: LambdaParams,
        arrow: Token,
        body: LambdaBody,
    },
    New {
        new_kw: Token,
        ty: Type,
        /// Creation arguments; absent for array creation.
        args: Option<CallArgs>,
        /// `{ field = value, ... }` object initializer, or array initializer.
        initializer: Option<InitializerList>,
    },
    /// `{ e, e, ... }` initializer list.
    Initializer(InitializerList),
    Typeof {
        kw: Token,
        l_paren: Token,
        ty: Type,
        r_paren: Token,
    },
    Sizeof {
        kw: Token,
        l_paren: Token,
        ty: Type,
        r_paren: Token,
    },
    Yield {
        yield_kw: Token,
        expr: Box<Expr>,
    },
    /// Assignment (right-associative): `lhs = rhs`, `lhs += rhs`, ...
    Assign {
        lhs: Box<Expr>,
        /// One token, or `>` + `>=` for `>>=`.
        op: Vec<Token>,
        rhs: Box<Expr>,
    },
    /// Tuple expression `(a, b)`? Not in Vala — placeholder removed.
    /// Statement expression keyword `ref`/`out` argument is handled in `Arg`.
    Error(Token),
}

#[derive(Debug)]
pub struct CallArgs {
    pub l_paren: Token,
    pub args: Punctuated<Arg>,
    pub r_paren: Token,
}

#[derive(Debug)]
pub struct Arg {
    /// `out` / `ref` modifier.
    pub modifier: Option<Token>,
    /// GObject-style named argument: `Object (application: app)`.
    pub name: Option<(Token, Token)>, // (name, `:`)
    pub expr: Expr,
}

#[derive(Debug)]
pub struct InitializerList {
    pub l_brace: Token,
    pub items: Punctuated<Expr>,
    pub r_brace: Token,
}

#[derive(Debug)]
pub enum LambdaParams {
    /// `ident => ...`
    Single(Token),
    /// `(a, b) => ...`
    List {
        l_paren: Token,
        params: Punctuated<Token>,
        r_paren: Token,
    },
}

#[derive(Debug)]
pub enum LambdaBody {
    Block(Block),
    Expr(Box<Expr>),
}

impl Expr {
    /// Whether the assignment `=` in object initializers etc. binds a plain
    /// name — used by the formatter for `name = value` layouts.
    pub fn is_atom(&self) -> bool {
        matches!(self, Expr::Atom(_))
    }
}

// ---------------------------------------------------------------------------
// First-token accessors (used for blank-line preservation and comment checks)
// ---------------------------------------------------------------------------

impl Member {
    pub fn first_token(&self) -> &Token {
        if let Some(attrs) = self.attributes.first() {
            return &attrs.l_bracket;
        }
        self.kind.first_token()
    }
}

impl MemberKind {
    pub fn first_token(&self) -> &Token {
        match self {
            MemberKind::Using(u) => &u.using_kw,
            MemberKind::Namespace(n) => n.modifiers.first().unwrap_or(&n.namespace_kw),
            MemberKind::Class(t) | MemberKind::Interface(t) | MemberKind::Struct(t) => {
                t.modifiers.first().unwrap_or(&t.keyword)
            }
            MemberKind::Enum(e) | MemberKind::Errordomain(e) => {
                e.modifiers.first().unwrap_or(&e.keyword)
            }
            MemberKind::Delegate(d) => d.modifiers.first().unwrap_or(&d.delegate_kw),
            MemberKind::Constant(c) => c.modifiers.first().unwrap_or(&c.const_kw),
            MemberKind::Field(f) => f.modifiers.first().unwrap_or_else(|| f.ty.first_token()),
            MemberKind::Method(m) => m.modifiers.first().unwrap_or_else(|| m.return_type.first_token()),
            MemberKind::CreationMethod(m) => m.modifiers.first().unwrap_or_else(|| m.name.first_token()),
            MemberKind::Property(p) => p.modifiers.first().unwrap_or_else(|| p.ty.first_token()),
            MemberKind::Signal(s) => s.modifiers.first().unwrap_or(&s.signal_kw),
            MemberKind::Constructor(c) => c.modifiers.first().unwrap_or(&c.construct_kw),
            MemberKind::Destructor(d) => d.modifiers.first().unwrap_or(&d.tilde),
            MemberKind::Empty(t) => t,
        }
    }
}

impl QualifiedName {
    pub fn first_token(&self) -> &Token {
        match &self.global {
            Some((g, _)) => g,
            None => &self.parts[0],
        }
    }
}

impl Type {
    pub fn first_token(&self) -> &Token {
        if let Some(t) = self.prefix.first() {
            return t;
        }
        match &self.core {
            TypeCore::Named { name, .. } => name.first_token(),
            TypeCore::Void(t) => t,
        }
    }
}

impl LocalDeclType {
    pub fn first_token(&self) -> &Token {
        match self {
            LocalDeclType::Var { prefix, var_kw, .. } => prefix.first().unwrap_or(var_kw),
            LocalDeclType::Type(t) => t.first_token(),
        }
    }
}

impl Stmt {
    pub fn first_token(&self) -> &Token {
        match self {
            Stmt::Block(b) => &b.l_brace,
            Stmt::Empty(t) => t,
            Stmt::LocalDecl(d) => d.const_kw.as_ref().unwrap_or_else(|| d.ty.first_token()),
            Stmt::Expr { expr, .. } => expr.first_token(),
            Stmt::If(s) => &s.if_kw,
            Stmt::Switch(s) => &s.switch_kw,
            Stmt::While(s) => &s.while_kw,
            Stmt::DoWhile(s) => &s.do_kw,
            Stmt::For(s) => &s.for_kw,
            Stmt::Foreach(s) => &s.foreach_kw,
            Stmt::Break { kw, .. }
            | Stmt::Continue { kw, .. }
            | Stmt::Return { kw, .. }
            | Stmt::Throw { kw, .. }
            | Stmt::Unlock { kw, .. }
            | Stmt::Delete { kw, .. }
            | Stmt::Yield { kw, .. } => kw,
            Stmt::Try(s) => &s.try_kw,
            Stmt::Lock(s) => &s.lock_kw,
            Stmt::With(s) => &s.with_kw,
        }
    }
}

impl Expr {
    pub fn first_token(&self) -> &Token {
        match self {
            Expr::Atom(t) | Expr::Error(t) => t,
            Expr::Global { global_kw, .. } => global_kw,
            Expr::Paren { l_paren, .. } => l_paren,
            Expr::Member { base, .. }
            | Expr::Call { callee: base, .. }
            | Expr::Index { base, .. }
            | Expr::Slice { base, .. }
            | Expr::PostfixIncDec { base, .. } => base.first_token(),
            Expr::Unary { op, .. } => op,
            Expr::Cast { l_paren, .. }
            | Expr::NonNullCast { l_paren, .. }
            | Expr::OwnershipTransfer { l_paren, .. } => l_paren,
            Expr::Binary { lhs, .. } | Expr::Assign { lhs, .. } => lhs.first_token(),
            Expr::TypeOp { expr, .. } => expr.first_token(),
            Expr::Ternary { cond, .. } => cond.first_token(),
            Expr::Lambda { params, .. } => match params {
                LambdaParams::Single(t) => t,
                LambdaParams::List { l_paren, .. } => l_paren,
            },
            Expr::New { new_kw, .. } => new_kw,
            Expr::Initializer(i) => &i.l_brace,
            Expr::Typeof { kw, .. } | Expr::Sizeof { kw, .. } => kw,
            Expr::Yield { yield_kw, .. } => yield_kw,
        }
    }
}
