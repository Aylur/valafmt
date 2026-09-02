//! AST → Doc conversion: the actual formatting decisions live here.
//! Layout style follows how Prettier formats TypeScript: same-line braces, no
//! space before call parentheses, width-driven wrapping.

pub mod expr;
pub mod stmt;
pub mod trivia;

use crate::doc::{self, Doc};
use crate::lexer::token::Token;
use crate::syntax::ast::*;
use trivia::{blank_line_before, leading_parts, tok, token_text, trailing_docs};

pub fn format_file(file: &File) -> Doc {
    let mut out = Vec::new();
    for (i, member) in file.members.iter().enumerate() {
        if i > 0 {
            out.push(doc::hardline());
            if blank_line_before(member.first_token()) {
                out.push(doc::hardline());
            }
        }
        out.push(member_doc(member));
    }
    // End-of-file comments live on the EOF token.
    for p in leading_parts(&file.eof.leading) {
        out.push(doc::hardline());
        out.push(p);
    }
    if !file.members.is_empty() || trivia::has_leading_pieces(&file.eof) {
        out.push(doc::hardline()); // final newline
    }
    doc::concat(out)
}

fn sp() -> Doc {
    doc::text(" ")
}

/// Emit `tok` for each of a list of modifier tokens, space-separated, with a
/// trailing space when non-empty.
fn modifiers_doc(mods: &[Token]) -> Doc {
    let mut out = Vec::new();
    for m in mods {
        out.push(tok(m));
        out.push(sp());
    }
    doc::concat(out)
}

pub fn qualified_name_doc(name: &QualifiedName) -> Doc {
    let mut out = Vec::new();
    if let Some((g, cc)) = &name.global {
        out.push(tok(g));
        out.push(tok(cc));
    }
    for part in &name.parts {
        out.push(tok(part));
    }
    doc::concat(out)
}

pub fn type_doc(ty: &Type) -> Doc {
    let mut out = Vec::new();
    for p in &ty.prefix {
        out.push(tok(p));
        out.push(sp());
    }
    match &ty.core {
        TypeCore::Void(t) => out.push(tok(t)),
        TypeCore::Named { name, type_args } => {
            out.push(qualified_name_doc(name));
            if let Some(args) = type_args {
                out.push(type_args_doc(args));
            }
        }
        TypeCore::Paren { l_paren, inner, r_paren } => {
            out.push(tok(l_paren));
            out.push(type_doc(inner));
            out.push(tok(r_paren));
        }
    }
    for s in &ty.suffixes {
        match s {
            TypeSuffix::Nullable(t) | TypeSuffix::Pointer(t) => out.push(tok(t)),
            TypeSuffix::Array { l_bracket, dims, r_bracket, nullable } => {
                out.push(tok(l_bracket));
                for (i, dim) in dims.items.iter().enumerate() {
                    if let Some(e) = dim {
                        out.push(expr::expr_doc(e));
                    }
                    if i < dims.commas.len() {
                        out.push(tok(&dims.commas[i]));
                        if dims.items.get(i + 1).map(|d| d.is_some()).unwrap_or(false) {
                            out.push(sp());
                        }
                    }
                }
                out.push(tok(r_bracket));
                if let Some(q) = nullable {
                    out.push(tok(q));
                }
            }
        }
    }
    doc::concat(out)
}

pub fn type_args_doc(args: &TypeArgList) -> Doc {
    let mut out = vec![tok(&args.lt)];
    for (i, a) in args.args.items.iter().enumerate() {
        if i > 0 {
            out.push(tok(&args.args.commas[i - 1]));
            out.push(sp());
        }
        out.push(type_doc(a));
    }
    out.push(tok(&args.gt));
    doc::concat(out)
}

// ---------------------------------------------------------------------------
// Members
// ---------------------------------------------------------------------------

pub fn member_doc(member: &Member) -> Doc {
    let mut out = Vec::new();
    for attrs in &member.attributes {
        out.push(attribute_list_doc(attrs));
        out.push(doc::hardline());
    }
    out.push(member_kind_doc(&member.kind));
    doc::concat(out)
}

fn attribute_list_doc(attrs: &AttributeList) -> Doc {
    let mut out = vec![tok(&attrs.l_bracket)];
    for (i, attr) in attrs.attrs.items.iter().enumerate() {
        if i > 0 {
            out.push(tok(&attrs.attrs.commas[i - 1]));
            out.push(sp());
        }
        out.push(tok(&attr.name));
        if let Some(args) = &attr.args {
            out.push(sp());
            out.push(tok(&args.l_paren));
            let mut inner = Vec::new();
            for (j, arg) in args.args.items.iter().enumerate() {
                if j > 0 {
                    inner.push(tok(&args.args.commas[j - 1]));
                    inner.push(doc::line());
                }
                inner.push(tok(&arg.name));
                inner.push(sp());
                inner.push(tok(&arg.eq));
                inner.push(sp());
                inner.push(expr::expr_doc(&arg.value));
            }
            out.push(doc::group(doc::concat(vec![
                doc::indent(doc::concat(vec![doc::softline(), doc::concat(inner)])),
                doc::softline(),
            ])));
            out.push(tok(&args.r_paren));
        }
    }
    out.push(tok(&attrs.r_bracket));
    doc::concat(out)
}

fn member_kind_doc(kind: &MemberKind) -> Doc {
    match kind {
        MemberKind::Using(u) => {
            let mut out = vec![tok(&u.using_kw), sp()];
            for (i, n) in u.names.items.iter().enumerate() {
                if i > 0 {
                    out.push(tok(&u.names.commas[i - 1]));
                    out.push(sp());
                }
                out.push(qualified_name_doc(n));
            }
            out.push(tok(&u.semicolon));
            doc::concat(out)
        }
        MemberKind::Namespace(n) => doc::concat(vec![
            modifiers_doc(&n.modifiers),
            tok(&n.namespace_kw),
            sp(),
            qualified_name_doc(&n.name),
            sp(),
            decl_body_doc(&n.body),
        ]),
        MemberKind::Class(t) | MemberKind::Interface(t) | MemberKind::Struct(t) => {
            type_decl_doc(t)
        }
        MemberKind::Enum(e) | MemberKind::Errordomain(e) => enum_decl_doc(e),
        MemberKind::Delegate(d) => {
            let mut out = vec![
                modifiers_doc(&d.modifiers),
                tok(&d.delegate_kw),
                sp(),
                type_doc(&d.return_type),
                sp(),
                qualified_name_doc(&d.name),
            ];
            if let Some(tp) = &d.type_params {
                out.push(type_params_doc(tp));
            }
            out.push(params_doc(&d.l_paren, &d.params, &d.r_paren));
            if let Some(t) = &d.throws {
                out.push(throws_doc(t));
            }
            out.push(tok(&d.semicolon));
            doc::concat(out)
        }
        MemberKind::Constant(c) => {
            let mut out = vec![
                modifiers_doc(&c.modifiers),
                tok(&c.const_kw),
                sp(),
                type_doc(&c.ty),
                sp(),
                tok(&c.name),
            ];
            out.push(c_array_doc(&c.array));
            if let Some((eq, init)) = &c.init {
                out.push(assign_rhs_doc(eq, init));
            }
            out.push(tok(&c.semicolon));
            doc::concat(out)
        }
        MemberKind::Field(f) => {
            let mut out = vec![modifiers_doc(&f.modifiers), type_doc(&f.ty), sp(), tok(&f.name)];
            out.push(c_array_doc(&f.array));
            if let Some((eq, init)) = &f.init {
                out.push(assign_rhs_doc(eq, init));
            }
            out.push(tok(&f.semicolon));
            doc::concat(out)
        }
        MemberKind::Method(m) => method_decl_doc(m),
        MemberKind::CreationMethod(m) => {
            let mut out = vec![modifiers_doc(&m.modifiers), qualified_name_doc(&m.name)];
            out.push(params_doc(&m.l_paren, &m.params, &m.r_paren));
            if let Some(t) = &m.throws {
                out.push(throws_doc(t));
            }
            out.push(contracts_doc(&m.contracts));
            out.push(body_doc(&m.body));
            doc::concat(out)
        }
        MemberKind::Property(p) => property_doc(p),
        MemberKind::Signal(s) => {
            let mut out = vec![
                modifiers_doc(&s.modifiers),
                tok(&s.signal_kw),
                sp(),
                type_doc(&s.return_type),
                sp(),
                tok(&s.name),
            ];
            out.push(params_doc(&s.l_paren, &s.params, &s.r_paren));
            out.push(body_doc(&s.body));
            doc::concat(out)
        }
        MemberKind::Constructor(c) => doc::concat(vec![
            modifiers_doc(&c.modifiers),
            tok(&c.construct_kw),
            sp(),
            stmt::block_doc(&c.body),
        ]),
        MemberKind::Destructor(d) => doc::concat(vec![
            modifiers_doc(&d.modifiers),
            tok(&d.tilde),
            tok(&d.name),
            tok(&d.l_paren),
            tok(&d.r_paren),
            sp(),
            stmt::block_doc(&d.body),
        ]),
        // Stray semicolons are kept (they may carry comments) but the token
        // itself is harmless to reprint.
        MemberKind::Empty(t) => tok(t),
    }
}

pub fn method_decl_doc(m: &MethodDecl) -> Doc {
    let mut out = vec![
        modifiers_doc(&m.modifiers),
        type_doc(&m.return_type),
        sp(),
        qualified_name_doc(&m.name),
    ];
    if let Some(tp) = &m.type_params {
        out.push(type_params_doc(tp));
    }
    out.push(params_doc(&m.l_paren, &m.params, &m.r_paren));
    if let Some(t) = &m.throws {
        out.push(throws_doc(t));
    }
    out.push(contracts_doc(&m.contracts));
    out.push(body_doc(&m.body));
    doc::concat(out)
}

fn type_decl_doc(t: &TypeDecl) -> Doc {
    let mut out = vec![modifiers_doc(&t.modifiers), tok(&t.keyword), sp(), qualified_name_doc(&t.name)];
    if let Some(tp) = &t.type_params {
        out.push(type_params_doc(tp));
    }
    if let Some(base) = &t.base {
        // `class Foo : Base, Iface {`; the type list breaks with an indent
        // when it does not fit.
        let mut list = Vec::new();
        for (i, ty) in base.types.items.iter().enumerate() {
            if i > 0 {
                list.push(tok(&base.types.commas[i - 1]));
                list.push(doc::line());
            }
            list.push(type_doc(ty));
        }
        out.push(sp());
        out.push(tok(&base.colon));
        out.push(doc::group(doc::indent(doc::concat(vec![doc::line(), doc::concat(list)]))));
    }
    out.push(sp());
    out.push(decl_body_doc(&t.body));
    doc::concat(out)
}

fn type_params_doc(tp: &TypeParamList) -> Doc {
    let mut out = vec![tok(&tp.lt)];
    for (i, p) in tp.params.items.iter().enumerate() {
        if i > 0 {
            out.push(tok(&tp.params.commas[i - 1]));
            out.push(sp());
        }
        out.push(tok(p));
    }
    out.push(tok(&tp.gt));
    doc::concat(out)
}

fn enum_decl_doc(e: &EnumDecl) -> Doc {
    let mut out = vec![modifiers_doc(&e.modifiers), tok(&e.keyword), sp(), qualified_name_doc(&e.name)];
    if let Some(base) = &e.base {
        out.push(sp());
        out.push(tok(&base.colon));
        out.push(sp());
        out.push(type_doc(&base.types.items[0]));
    }
    out.push(sp());
    out.push(tok(&e.l_brace));
    let mut inner = Vec::new();
    for (i, v) in e.values.items.iter().enumerate() {
        inner.push(doc::hardline());
        if i > 0 && blank_line_before(enum_value_first_token(v)) {
            inner.push(doc::hardline());
        }
        for attrs in &v.attributes {
            inner.push(attribute_list_doc(attrs));
            inner.push(doc::hardline());
        }
        inner.push(tok(&v.name));
        if let Some((eq, val)) = &v.value {
            inner.push(sp());
            inner.push(tok(eq));
            inner.push(sp());
            inner.push(expr::expr_doc(val));
        }
        if i < e.values.commas.len() {
            inner.push(tok(&e.values.commas[i]));
        } else if i + 1 == e.values.items.len() && e.semicolon.is_none() {
            // Normalize: add a trailing comma when the enum has plain values
            // and none in source (broken layout always gets one)... enums in
            // Vala accept a trailing comma; keep output stable by adding one
            // only if the source had one (handled above) — otherwise nothing.
        }
    }
    if let Some(semi) = &e.semicolon {
        inner.push(tok(semi));
    }
    for (i, m) in e.members.iter().enumerate() {
        inner.push(doc::hardline());
        if i > 0 || !e.values.items.is_empty() {
            if blank_line_before(m.first_token()) {
                inner.push(doc::hardline());
            }
        }
        inner.push(member_doc(m));
    }
    // Dangling comments before `}`.
    for p in leading_parts(&e.r_brace.leading) {
        inner.push(doc::hardline());
        inner.push(p);
    }
    out.push(doc::indent(doc::concat(inner)));
    out.push(doc::hardline());
    out.push(token_text(&e.r_brace));
    out.extend(trailing_docs(&e.r_brace.trailing));
    doc::concat(out)
}

fn enum_value_first_token(v: &EnumValue) -> &Token {
    v.attributes.first().map(|a| &a.l_bracket).unwrap_or(&v.name)
}

fn property_doc(p: &PropertyDecl) -> Doc {
    let mut out = vec![
        modifiers_doc(&p.modifiers),
        type_doc(&p.ty),
        sp(),
        qualified_name_doc(&p.name),
        sp(),
    ];
    // `{ get; set; }` stays inline when all accessors are bodyless and fit;
    // any block body forces one accessor per line.
    let mut inner = Vec::new();
    for acc in &p.accessors {
        inner.push(doc::line());
        inner.push(property_accessor_doc(acc));
    }
    for piece in leading_parts(&p.r_brace.leading) {
        inner.push(doc::hardline());
        inner.push(piece);
    }
    out.push(doc::group(doc::concat(vec![
        tok(&p.l_brace),
        doc::indent(doc::concat(inner)),
        doc::line(),
        token_text(&p.r_brace),
        doc::concat(trailing_docs(&p.r_brace.trailing)),
    ])));
    doc::concat(out)
}

fn property_accessor_doc(acc: &PropertyAccessor) -> Doc {
    match acc {
        PropertyAccessor::Accessor { attributes, modifiers, keywords, body } => {
            let mut out = Vec::new();
            for attrs in attributes {
                out.push(attribute_list_doc(attrs));
                out.push(sp());
            }
            out.push(modifiers_doc(modifiers));
            for (i, kw) in keywords.iter().enumerate() {
                if i > 0 {
                    out.push(sp());
                }
                out.push(tok(kw));
            }
            out.push(body_doc(body));
            doc::concat(out)
        }
        PropertyAccessor::Default { default_kw, eq, expr: e, semicolon } => doc::concat(vec![
            tok(default_kw),
            sp(),
            tok(eq),
            sp(),
            expr::expr_doc(e),
            tok(semicolon),
        ]),
    }
}

fn params_doc(l_paren: &Token, params: &Punctuated<Param>, r_paren: &Token) -> Doc {
    if params.is_empty() {
        return doc::concat(vec![tok(l_paren), tok(r_paren)]);
    }
    let mut inner = Vec::new();
    for (i, p) in params.items.iter().enumerate() {
        if i > 0 {
            inner.push(tok(&params.commas[i - 1]));
            inner.push(doc::line());
        }
        inner.push(param_doc(p));
    }
    doc::group(doc::concat(vec![
        tok(l_paren),
        doc::indent(doc::concat(vec![doc::softline(), doc::concat(inner)])),
        doc::softline(),
        tok(r_paren),
    ]))
}

fn param_doc(p: &Param) -> Doc {
    let mut out = Vec::new();
    for attrs in &p.attributes {
        out.push(attribute_list_doc(attrs));
        out.push(sp());
    }
    out.push(modifiers_doc(&p.modifiers));
    match &p.kind {
        ParamKind::Ellipsis(t) => out.push(tok(t)),
        ParamKind::Typed { ty, name, default } => {
            out.push(type_doc(ty));
            out.push(sp());
            out.push(tok(name));
            if let Some((eq, e)) = default {
                out.push(sp());
                out.push(tok(eq));
                out.push(sp());
                out.push(expr::expr_doc(e));
            }
        }
    }
    doc::concat(out)
}

fn throws_doc(t: &ThrowsClause) -> Doc {
    let mut out = vec![sp(), tok(&t.throws_kw), sp()];
    for (i, ty) in t.types.items.iter().enumerate() {
        if i > 0 {
            out.push(tok(&t.types.commas[i - 1]));
            out.push(sp());
        }
        out.push(type_doc(ty));
    }
    doc::concat(out)
}

fn contracts_doc(contracts: &[Contract]) -> Doc {
    let mut out = Vec::new();
    for c in contracts {
        // Contracts go on their own lines, indented, before the body.
        out.push(doc::indent(doc::concat(vec![
            doc::hardline(),
            tok(&c.keyword),
            sp(),
            tok(&c.l_paren),
            expr::expr_doc(&c.expr),
            tok(&c.r_paren),
        ])));
    }
    doc::concat(out)
}

/// ` {` block `}` or `;`.
fn body_doc(body: &Body) -> Doc {
    match body {
        Body::Block(b) => doc::concat(vec![sp(), stmt::block_doc(b)]),
        Body::Semicolon(t) => tok(t),
    }
}

fn decl_body_doc(body: &DeclBody) -> Doc {
    let mut inner = Vec::new();
    for (i, m) in body.members.iter().enumerate() {
        inner.push(doc::hardline());
        if i > 0 && blank_line_before(m.first_token()) {
            inner.push(doc::hardline());
        }
        inner.push(member_doc(m));
    }
    for p in leading_parts(&body.r_brace.leading) {
        inner.push(doc::hardline());
        inner.push(p);
    }
    if inner.is_empty() {
        return doc::concat(vec![tok(&body.l_brace), token_text(&body.r_brace), doc::concat(trailing_docs(&body.r_brace.trailing))]);
    }
    doc::concat(vec![
        tok(&body.l_brace),
        doc::indent(doc::concat(inner)),
        doc::hardline(),
        token_text(&body.r_brace),
        doc::concat(trailing_docs(&body.r_brace.trailing)),
    ])
}

/// `[LEN]` on a declarator name.
pub fn c_array_doc(array: &Option<CArraySuffix>) -> Doc {
    match array {
        None => doc::nil(),
        Some(a) => {
            let mut out = vec![tok(&a.l_bracket)];
            if let Some(size) = &a.size {
                out.push(expr::expr_doc(size));
            }
            out.push(tok(&a.r_bracket));
            doc::concat(out)
        }
    }
}

/// ` = rhs` with Prettier assignment layout: hugging values attach directly,
/// everything else may break after the `=` with an indent.
pub fn assign_rhs_doc(eq: &Token, rhs: &Expr) -> Doc {
    let rhs_doc = expr::expr_doc(rhs);
    if expr::hugs_assignment(rhs) {
        doc::concat(vec![sp(), tok(eq), sp(), rhs_doc])
    } else {
        doc::concat(vec![
            sp(),
            tok(eq),
            doc::group(doc::indent(doc::concat(vec![doc::line(), rhs_doc]))),
        ])
    }
}
