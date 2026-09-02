//! Statement formatting.

use super::expr::expr_doc;
use super::trivia::{blank_line_before, leading_parts, tok, token_text, trailing_docs};
use super::{assign_rhs_doc, type_doc};
use crate::doc::{self, Doc};
use crate::lexer::token::Token;
use crate::syntax::ast::*;

fn sp() -> Doc {
    doc::text(" ")
}

pub fn block_doc(b: &Block) -> Doc {
    let mut inner = Vec::new();
    for (i, s) in b.statements.iter().enumerate() {
        inner.push(doc::hardline());
        if i > 0 && blank_line_before(s.first_token()) {
            inner.push(doc::hardline());
        }
        inner.push(stmt_doc(s));
    }
    for p in leading_parts(&b.r_brace.leading) {
        inner.push(doc::hardline());
        inner.push(p);
    }
    if inner.is_empty() {
        return doc::concat(vec![
            tok(&b.l_brace),
            token_text(&b.r_brace),
            doc::concat(trailing_docs(&b.r_brace.trailing)),
        ]);
    }
    doc::concat(vec![
        tok(&b.l_brace),
        doc::indent(doc::concat(inner)),
        doc::hardline(),
        token_text(&b.r_brace),
        doc::concat(trailing_docs(&b.r_brace.trailing)),
    ])
}

/// `(cond)` with soft breaking inside the parens.
fn paren_cond(l: &Token, e: &Expr, r: &Token) -> Doc {
    doc::group(doc::concat(vec![
        tok(l),
        doc::indent(doc::concat(vec![doc::softline(), expr_doc(e)])),
        doc::softline(),
        tok(r),
    ]))
}

/// The body of if/while/for/foreach: ` { ... }` for blocks, otherwise the
/// statement inline when it fits, indented on its own line when it doesn't.
fn embedded_body(s: &Stmt) -> Doc {
    match s {
        Stmt::Block(b) => doc::concat(vec![sp(), block_doc(b)]),
        other => doc::group(doc::indent(doc::concat(vec![doc::line(), stmt_doc(other)]))),
    }
}

pub fn stmt_doc(s: &Stmt) -> Doc {
    match s {
        Stmt::Block(b) => block_doc(b),
        Stmt::Empty(t) => tok(t),
        Stmt::LocalDecl(d) => local_decl_doc(d),
        Stmt::Expr { expr, semicolon } => doc::concat(vec![expr_doc(expr), tok(semicolon)]),
        Stmt::If(i) => if_doc(i),
        Stmt::Switch(sw) => switch_doc(sw),
        Stmt::While(w) => doc::concat(vec![
            tok(&w.while_kw),
            sp(),
            paren_cond(&w.l_paren, &w.cond, &w.r_paren),
            embedded_body(&w.body),
        ]),
        Stmt::DoWhile(d) => {
            let body = match &*d.body {
                Stmt::Block(b) => doc::concat(vec![sp(), block_doc(b), sp()]),
                other => doc::concat(vec![
                    doc::group(doc::indent(doc::concat(vec![doc::line(), stmt_doc(other)]))),
                    doc::hardline(),
                ]),
            };
            doc::concat(vec![
                tok(&d.do_kw),
                body,
                tok(&d.while_kw),
                sp(),
                paren_cond(&d.l_paren, &d.cond, &d.r_paren),
                tok(&d.semicolon),
            ])
        }
        Stmt::For(f) => for_doc(f),
        Stmt::Foreach(f) => doc::concat(vec![
            tok(&f.foreach_kw),
            sp(),
            doc::group(doc::concat(vec![
                tok(&f.l_paren),
                doc::indent(doc::concat(vec![
                    doc::softline(),
                    local_decl_type_doc(&f.ty),
                    sp(),
                    tok(&f.name),
                    sp(),
                    tok(&f.in_kw),
                    sp(),
                    expr_doc(&f.expr),
                ])),
                doc::softline(),
                tok(&f.r_paren),
            ])),
            embedded_body(&f.body),
        ]),
        Stmt::Break { kw, semicolon } | Stmt::Continue { kw, semicolon } | Stmt::Yield { kw, semicolon } => {
            doc::concat(vec![tok(kw), tok(semicolon)])
        }
        Stmt::Return { kw, expr, semicolon } => {
            let mut out = vec![tok(kw)];
            if let Some(e) = expr {
                out.push(sp());
                out.push(expr_doc(e));
            }
            out.push(tok(semicolon));
            doc::concat(out)
        }
        Stmt::Throw { kw, expr, semicolon } => {
            doc::concat(vec![tok(kw), sp(), expr_doc(expr), tok(semicolon)])
        }
        Stmt::Try(t) => try_doc(t),
        Stmt::Lock(l) => {
            let mut out = vec![tok(&l.lock_kw), sp(), paren_cond(&l.l_paren, &l.expr, &l.r_paren)];
            match &l.body {
                Body::Block(b) => {
                    out.push(sp());
                    out.push(block_doc(b));
                }
                Body::Semicolon(t) => out.push(tok(t)),
            }
            doc::concat(out)
        }
        Stmt::Unlock { kw, l_paren, expr, r_paren, semicolon } => doc::concat(vec![
            tok(kw),
            sp(),
            paren_cond(l_paren, expr, r_paren),
            tok(semicolon),
        ]),
        Stmt::Delete { kw, expr, semicolon } => {
            doc::concat(vec![tok(kw), sp(), expr_doc(expr), tok(semicolon)])
        }
        Stmt::With(w) => {
            let mut inner = vec![tok(&w.with_kw), sp(), tok(&w.l_paren)];
            if let Some((ty, name, eq)) = &w.decl {
                inner.push(local_decl_type_doc(ty));
                inner.push(sp());
                inner.push(tok(name));
                inner.push(sp());
                inner.push(tok(eq));
                inner.push(sp());
            }
            inner.push(expr_doc(&w.expr));
            inner.push(tok(&w.r_paren));
            inner.push(sp());
            inner.push(block_doc(&w.body));
            doc::concat(inner)
        }
    }
}

pub fn local_decl_type_doc(t: &LocalDeclType) -> Doc {
    match t {
        LocalDeclType::Var { prefix, var_kw, nullable } => {
            let mut out = Vec::new();
            for p in prefix {
                out.push(tok(p));
                out.push(sp());
            }
            out.push(tok(var_kw));
            if let Some(q) = nullable {
                out.push(tok(q));
            }
            doc::concat(out)
        }
        LocalDeclType::Type(ty) => type_doc(ty),
    }
}

pub fn local_decl_doc(d: &LocalDecl) -> Doc {
    let mut out = Vec::new();
    if let Some(c) = &d.const_kw {
        out.push(tok(c));
        out.push(sp());
    }
    out.push(local_decl_type_doc(&d.ty));
    out.push(sp());
    for (i, decl) in d.declarators.items.iter().enumerate() {
        if i > 0 {
            out.push(tok(&d.declarators.commas[i - 1]));
            out.push(sp());
        }
        out.push(tok(&decl.name));
        if let Some((eq, init)) = &decl.init {
            out.push(assign_rhs_doc(eq, init));
        }
    }
    out.push(tok(&d.semicolon));
    doc::concat(out)
}

fn if_doc(i: &IfStmt) -> Doc {
    let mut out = vec![tok(&i.if_kw), sp(), paren_cond(&i.l_paren, &i.cond, &i.r_paren)];
    let then_is_block = matches!(&*i.then, Stmt::Block(_));
    out.push(embedded_body(&i.then));
    if let Some(els) = &i.else_branch {
        if then_is_block {
            out.push(sp());
        } else {
            out.push(doc::hardline());
        }
        out.push(tok(&els.else_kw));
        match &*els.body {
            Stmt::If(nested) => {
                out.push(sp());
                out.push(if_doc(nested));
            }
            Stmt::Block(b) => {
                out.push(sp());
                out.push(block_doc(b));
            }
            other => out.push(doc::group(doc::indent(doc::concat(vec![
                doc::line(),
                stmt_doc(other),
            ])))),
        }
    }
    doc::concat(out)
}

fn for_doc(f: &ForStmt) -> Doc {
    let mut header = vec![tok(&f.for_kw), sp(), tok(&f.l_paren)];
    match &f.init {
        ForInit::Decl(d) => header.push(local_decl_doc(d)),
        ForInit::Exprs { exprs, semicolon } => {
            for (i, e) in exprs.items.iter().enumerate() {
                if i > 0 {
                    header.push(tok(&exprs.commas[i - 1]));
                    header.push(sp());
                }
                header.push(expr_doc(e));
            }
            header.push(tok(semicolon));
        }
    }
    if let Some(cond) = &f.cond {
        header.push(sp());
        header.push(expr_doc(cond));
    }
    header.push(tok(&f.semicolon2));
    if !f.update.items.is_empty() {
        header.push(sp());
        for (i, e) in f.update.items.iter().enumerate() {
            if i > 0 {
                header.push(tok(&f.update.commas[i - 1]));
                header.push(sp());
            }
            header.push(expr_doc(e));
        }
    }
    header.push(tok(&f.r_paren));
    header.push(embedded_body(&f.body));
    doc::concat(header)
}

fn switch_doc(sw: &SwitchStmt) -> Doc {
    let mut out = vec![
        tok(&sw.switch_kw),
        sp(),
        paren_cond(&sw.l_paren, &sw.expr, &sw.r_paren),
        sp(),
        tok(&sw.l_brace),
    ];
    let mut inner = Vec::new();
    for (i, section) in sw.sections.iter().enumerate() {
        inner.push(doc::hardline());
        if i > 0 {
            let first = match &section.labels[0] {
                SwitchLabel::Case { case_kw, .. } => case_kw,
                SwitchLabel::Default { default_kw, .. } => default_kw,
            };
            if blank_line_before(first) {
                inner.push(doc::hardline());
            }
        }
        for (j, label) in section.labels.iter().enumerate() {
            if j > 0 {
                inner.push(doc::hardline());
            }
            match label {
                SwitchLabel::Case { case_kw, expr, colon } => {
                    inner.push(tok(case_kw));
                    inner.push(sp());
                    inner.push(expr_doc(expr));
                    inner.push(tok(colon));
                }
                SwitchLabel::Default { default_kw, colon } => {
                    inner.push(tok(default_kw));
                    inner.push(tok(colon));
                }
            }
        }
        let mut stmts = Vec::new();
        for (k, s) in section.statements.iter().enumerate() {
            stmts.push(doc::hardline());
            if k > 0 && blank_line_before(s.first_token()) {
                stmts.push(doc::hardline());
            }
            stmts.push(stmt_doc(s));
        }
        inner.push(doc::indent(doc::concat(stmts)));
    }
    for p in leading_parts(&sw.r_brace.leading) {
        inner.push(doc::hardline());
        inner.push(p);
    }
    out.push(doc::indent(doc::concat(inner)));
    out.push(doc::hardline());
    out.push(token_text(&sw.r_brace));
    out.extend(trailing_docs(&sw.r_brace.trailing));
    doc::concat(out)
}

fn try_doc(t: &TryStmt) -> Doc {
    let mut out = vec![tok(&t.try_kw), sp(), block_doc(&t.body)];
    for c in &t.catches {
        out.push(sp());
        out.push(tok(&c.catch_kw));
        if let Some(p) = &c.param {
            out.push(sp());
            out.push(tok(&p.l_paren));
            out.push(type_doc(&p.ty));
            out.push(sp());
            out.push(tok(&p.name));
            out.push(tok(&p.r_paren));
        }
        out.push(sp());
        out.push(block_doc(&c.body));
    }
    if let Some(f) = &t.finally {
        out.push(sp());
        out.push(tok(&f.finally_kw));
        out.push(sp());
        out.push(block_doc(&f.body));
    }
    doc::concat(out)
}
