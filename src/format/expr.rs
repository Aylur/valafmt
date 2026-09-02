//! Expression formatting: binary chains, member chains, call arguments, and
//! the Prettier-style "hug the last lambda argument" layout that keeps
//! `foo.clicked.connect(() => { ... });` on one line.

use super::stmt::block_doc;
use super::trivia::tok;
use super::{type_args_doc, type_doc};
use crate::doc::{self, Doc};
use crate::lexer::token::{Token, TokenKind};
use crate::syntax::ast::*;

fn sp() -> Doc {
    doc::text(" ")
}

/// Values that attach directly after `=` instead of breaking with an indent:
/// anything that can break internally in a useful way (lambdas, object
/// creation, initializer lists, calls and call chains, ternaries) plus plain
/// atoms. Binary chains and other operators break after the `=` instead,
/// matching Prettier's assignment layout.
pub fn hugs_assignment(e: &Expr) -> bool {
    match e {
        Expr::Lambda { .. }
        | Expr::New { .. }
        | Expr::Initializer(_)
        | Expr::Atom(_)
        | Expr::Global { .. }
        | Expr::Ternary { .. } => true,
        Expr::Yield { expr, .. } => hugs_assignment(expr),
        // Postfix chains hug only when they contain a call (otherwise there
        // is nowhere for them to break and the `=` break is the only option).
        Expr::Call { .. } => true,
        Expr::Member { base, .. }
        | Expr::Index { base, .. }
        | Expr::Slice { base, .. }
        | Expr::PostfixIncDec { base, .. } => spine_has_call(base),
        _ => false,
    }
}

fn spine_has_call(e: &Expr) -> bool {
    match e {
        Expr::Call { .. } => true,
        Expr::Member { base, .. }
        | Expr::Index { base, .. }
        | Expr::Slice { base, .. }
        | Expr::PostfixIncDec { base, .. } => spine_has_call(base),
        _ => false,
    }
}

pub fn expr_doc(e: &Expr) -> Doc {
    match e {
        Expr::Atom(t) | Expr::Error(t) => tok(t),
        Expr::Global { global_kw, colon_colon, name } => {
            doc::concat(vec![tok(global_kw), tok(colon_colon), tok(name)])
        }
        Expr::Paren { l_paren, expr, r_paren } => {
            doc::concat(vec![tok(l_paren), expr_doc(expr), tok(r_paren)])
        }
        Expr::Member { .. }
        | Expr::Call { .. }
        | Expr::Index { .. }
        | Expr::Slice { .. }
        | Expr::PostfixIncDec { .. } => chain_doc(e),
        Expr::Unary { op, expr } => {
            let space = matches!(op.kind, TokenKind::Kw(_));
            let mut out = vec![tok(op)];
            if space {
                out.push(sp());
            }
            out.push(expr_doc(expr));
            doc::concat(out)
        }
        Expr::Cast { l_paren, ty, r_paren, expr } => doc::concat(vec![
            tok(l_paren),
            type_doc(ty),
            tok(r_paren),
            sp(),
            expr_doc(expr),
        ]),
        Expr::NonNullCast { l_paren, bang, r_paren, expr } => doc::concat(vec![
            tok(l_paren),
            tok(bang),
            tok(r_paren),
            sp(),
            expr_doc(expr),
        ]),
        Expr::OwnershipTransfer { l_paren, owned_kw, r_paren, expr } => doc::concat(vec![
            tok(l_paren),
            tok(owned_kw),
            tok(r_paren),
            sp(),
            expr_doc(expr),
        ]),
        Expr::Binary { .. } => doc::group(binary_chain_doc(e)),
        Expr::TypeOp { expr, op, ty } => doc::concat(vec![
            expr_doc(expr),
            sp(),
            tok(op),
            sp(),
            type_doc(ty),
        ]),
        Expr::Ternary { cond, question, then, colon, els } => doc::group(doc::concat(vec![
            expr_doc(cond),
            doc::indent(doc::concat(vec![
                doc::line(),
                tok(question),
                sp(),
                expr_doc(then),
                doc::line(),
                tok(colon),
                sp(),
                expr_doc(els),
            ])),
        ])),
        Expr::Lambda { params, arrow, body } => lambda_doc(params, arrow, body),
        Expr::New { new_kw, ty, args, initializer } => {
            let mut out = vec![tok(new_kw), sp(), type_doc(ty)];
            if let Some(a) = args {
                out.push(call_args_doc(&a.l_paren, &a.args, &a.r_paren));
            }
            if let Some(init) = initializer {
                out.push(sp());
                out.push(initializer_doc(init));
            }
            doc::concat(out)
        }
        Expr::Initializer(init) => initializer_doc(init),
        Expr::Typeof { kw, l_paren, ty, r_paren } | Expr::Sizeof { kw, l_paren, ty, r_paren } => {
            doc::concat(vec![tok(kw), tok(l_paren), type_doc(ty), tok(r_paren)])
        }
        Expr::Yield { yield_kw, expr } => doc::concat(vec![tok(yield_kw), sp(), expr_doc(expr)]),
        Expr::Assign { lhs, op, rhs } => {
            let mut out = vec![expr_doc(lhs), sp()];
            for t in op {
                out.push(tok(t));
            }
            if hugs_assignment(rhs) {
                out.push(sp());
                out.push(expr_doc(rhs));
            } else {
                out.push(doc::group(doc::indent(doc::concat(vec![
                    doc::line(),
                    expr_doc(rhs),
                ]))));
            }
            doc::concat(out)
        }
    }
}

fn lambda_doc(params: &LambdaParams, arrow: &Token, body: &LambdaBody) -> Doc {
    let mut out = Vec::new();
    match params {
        LambdaParams::Single(t) => out.push(tok(t)),
        LambdaParams::List { l_paren, params, r_paren } => {
            out.push(tok(l_paren));
            for (i, p) in params.items.iter().enumerate() {
                if i > 0 {
                    out.push(tok(&params.commas[i - 1]));
                    out.push(sp());
                }
                for m in &p.modifiers {
                    out.push(tok(m));
                    out.push(sp());
                }
                out.push(tok(&p.name));
            }
            out.push(tok(r_paren));
        }
    }
    out.push(sp());
    out.push(tok(arrow));
    match body {
        LambdaBody::Block(b) => {
            out.push(sp());
            out.push(block_doc(b));
        }
        LambdaBody::Expr(e) => {
            out.push(doc::group(doc::indent(doc::concat(vec![doc::line(), expr_doc(e)]))));
        }
    }
    doc::concat(out)
}

// ---------------------------------------------------------------------------
// Binary chains
// ---------------------------------------------------------------------------

fn binary_prec(op: &[Token]) -> u8 {
    use TokenKind::*;
    match op[0].kind {
        QuestionQuestion => 1,
        PipePipe => 2,
        AmpAmp => 3,
        Pipe => 4,
        Caret => 5,
        Amp => 6,
        Eq | NotEq | Kw(crate::lexer::token::Keyword::In) => 7,
        Lt | LtEq | GtEq => 8,
        Gt => {
            if op.len() == 2 {
                9 // shift
            } else {
                8
            }
        }
        Shl => 9,
        Plus | Minus => 10,
        Star | Slash | Percent => 11,
        _ => 0,
    }
}

/// Flatten a chain of same-precedence binary operators so `a + b + c` breaks
/// as one unit with the operator kept at the end of each line.
fn binary_chain_doc(e: &Expr) -> Doc {
    let Expr::Binary { lhs, op, rhs } = e else {
        return expr_doc(e);
    };
    let prec = binary_prec(op);
    let mut rest = vec![(op, rhs)];
    let mut cur: &Expr = lhs;
    while let Expr::Binary { lhs, op, rhs } = cur {
        if binary_prec(op) != prec {
            break;
        }
        rest.push((op, rhs));
        cur = lhs;
    }
    rest.reverse();
    // No extra indent on continuations: the enclosing construct (assignment
    // break, condition parens) provides the indentation, as in Prettier.
    let mut out = vec![expr_doc(cur)];
    for (op, rhs) in rest {
        out.push(sp());
        for t in op.iter() {
            out.push(tok(t));
        }
        out.push(doc::line());
        out.push(expr_doc(rhs));
    }
    doc::concat(out)
}

// ---------------------------------------------------------------------------
// Postfix chains and calls
// ---------------------------------------------------------------------------

enum Link<'a> {
    Member { op: &'a Token, name: &'a Token, type_args: &'a Option<TypeArgList> },
    Call {
        type_args: &'a Option<TypeArgList>,
        l_paren: &'a Token,
        args: &'a Punctuated<Arg>,
        r_paren: &'a Token,
        initializer: &'a Option<InitializerList>,
    },
    Index { l_bracket: &'a Token, indices: &'a Punctuated<Expr>, r_bracket: &'a Token },
    Slice {
        l_bracket: &'a Token,
        start: &'a Option<Box<Expr>>,
        colon: &'a Token,
        end: &'a Option<Box<Expr>>,
        r_bracket: &'a Token,
    },
    Postfix { op: &'a Token },
}

fn flatten_chain<'a>(mut e: &'a Expr, links: &mut Vec<Link<'a>>) -> &'a Expr {
    loop {
        match e {
            Expr::Member { base, op, name, type_args } => {
                links.push(Link::Member { op, name, type_args });
                e = base;
            }
            Expr::Call { callee, type_args, l_paren, args, r_paren, initializer } => {
                links.push(Link::Call { type_args, l_paren, args, r_paren, initializer });
                e = callee;
            }
            Expr::Index { base, l_bracket, indices, r_bracket } => {
                links.push(Link::Index { l_bracket, indices, r_bracket });
                e = base;
            }
            Expr::Slice { base, l_bracket, start, colon, end, r_bracket } => {
                links.push(Link::Slice { l_bracket, start, colon, end, r_bracket });
                e = base;
            }
            Expr::PostfixIncDec { base, op } => {
                links.push(Link::Postfix { op });
                e = base;
            }
            _ => return e,
        }
    }
}

/// Does this argument list end with a lambda whose body is a block (or is a
/// lambda at all)? If so the call "hugs" it: other args stay flat and the
/// lambda breaks internally.
fn huggable_last_arg(args: &Punctuated<Arg>) -> bool {
    match args.items.last() {
        Some(Arg { expr: Expr::Lambda { .. }, .. }) => true,
        _ => false,
    }
}

fn chain_doc(e: &Expr) -> Doc {
    let mut links = Vec::new();
    let base = flatten_chain(e, &mut links);
    links.reverse();

    // Count breakable member links to decide whether the chain may break
    // before dots at all (short chains never do; they rely on arg breaking).
    let member_count = links.iter().filter(|l| matches!(l, Link::Member { .. })).count();
    let call_count = links.iter().filter(|l| matches!(l, Link::Call { .. })).count();
    let fluent = member_count >= 3 && call_count >= 2;

    // Hug detection: the final link is a call whose last argument is a lambda.
    let hug_final_call = matches!(links.last(), Some(Link::Call { args, .. }) if huggable_last_arg(args));

    let mut chain_parts = vec![expr_doc(base)];
    let split_at = if hug_final_call { links.len() - 1 } else { links.len() };
    let mut tail_parts = Vec::new();

    for (i, link) in links.iter().enumerate() {
        let target = if i < split_at { &mut chain_parts } else { &mut tail_parts };
        match link {
            Link::Member { op, name, type_args } => {
                if fluent && op.kind == TokenKind::Dot {
                    target.push(doc::softline());
                }
                target.push(tok(op));
                target.push(tok(name));
                if let Some(args) = type_args {
                    target.push(type_args_doc(args));
                }
            }
            Link::Call { type_args, l_paren, args, r_paren, initializer } => {
                if let Some(ta) = type_args {
                    target.push(type_args_doc(ta));
                }
                if huggable_last_arg(args) {
                    target.push(hugged_call_args_doc(l_paren, args, r_paren));
                } else {
                    target.push(call_args_doc(l_paren, args, r_paren));
                }
                if let Some(init) = initializer {
                    target.push(sp());
                    target.push(initializer_doc(init));
                }
            }
            Link::Index { l_bracket, indices, r_bracket } => {
                target.push(tok(l_bracket));
                for (j, idx) in indices.items.iter().enumerate() {
                    if j > 0 {
                        target.push(tok(&indices.commas[j - 1]));
                        target.push(sp());
                    }
                    target.push(expr_doc(idx));
                }
                target.push(tok(r_bracket));
            }
            Link::Slice { l_bracket, start, colon, end, r_bracket } => {
                target.push(tok(l_bracket));
                if let Some(s) = start {
                    target.push(expr_doc(s));
                }
                target.push(tok(colon));
                if let Some(en) = end {
                    target.push(expr_doc(en));
                }
                target.push(tok(r_bracket));
            }
            Link::Postfix { op } => target.push(tok(op)),
        }
    }

    let head = if fluent {
        let first = chain_parts.remove(0);
        doc::group(doc::concat(vec![first, doc::indent(doc::concat(chain_parts))]))
    } else {
        doc::concat(chain_parts)
    };
    if tail_parts.is_empty() {
        head
    } else {
        doc::concat(vec![head, doc::concat(tail_parts)])
    }
}

/// Standard argument list: breaks each argument onto its own line when the
/// whole list does not fit.
pub fn call_args_doc(l_paren: &Token, args: &Punctuated<Arg>, r_paren: &Token) -> Doc {
    if args.is_empty() {
        return doc::concat(vec![tok(l_paren), tok(r_paren)]);
    }
    let mut inner = Vec::new();
    for (i, a) in args.items.iter().enumerate() {
        if i > 0 {
            inner.push(tok(&args.commas[i - 1]));
            inner.push(doc::line());
        }
        inner.push(arg_doc(a));
    }
    doc::group(doc::concat(vec![
        tok(l_paren),
        doc::indent(doc::concat(vec![doc::softline(), doc::concat(inner)])),
        doc::softline(),
        tok(r_paren),
    ]))
}

/// Hugged argument list: `f(a, b, (x) => { ... })` — non-lambda args flat,
/// the final lambda free to break internally without breaking the list.
fn hugged_call_args_doc(l_paren: &Token, args: &Punctuated<Arg>, r_paren: &Token) -> Doc {
    let mut out = vec![tok(l_paren)];
    for (i, a) in args.items.iter().enumerate() {
        if i > 0 {
            out.push(tok(&args.commas[i - 1]));
            out.push(sp());
        }
        out.push(arg_doc(a));
    }
    out.push(tok(r_paren));
    doc::concat(out)
}

fn arg_doc(a: &Arg) -> Doc {
    let mut out = Vec::new();
    if let Some(m) = &a.modifier {
        out.push(tok(m));
        out.push(sp());
    }
    if let Some((name, colon)) = &a.name {
        out.push(tok(name));
        out.push(tok(colon));
        out.push(sp());
    }
    out.push(expr_doc(&a.expr));
    doc::concat(out)
}

/// `{ 1, 2, 3 }` initializer list; breaks one item per line with a trailing
/// comma when too long.
pub fn initializer_doc(init: &InitializerList) -> Doc {
    if init.items.is_empty() {
        return doc::concat(vec![tok(&init.l_brace), tok(&init.r_brace)]);
    }
    let mut inner = Vec::new();
    for (i, item) in init.items.items.iter().enumerate() {
        if i > 0 {
            inner.push(tok(&init.items.commas[i - 1]));
            inner.push(doc::line());
        }
        inner.push(arg_doc(item));
    }
    // A trailing comma in source is dropped flat / kept broken; one is added
    // when the list breaks. The safety check treats trailing commas before a
    // closing bracket as inert.
    let trailing_comma = if init.items.commas.len() == init.items.items.len() {
        // Emit the source's trailing comma token (it may carry comments).
        doc::if_break(tok(&init.items.commas[init.items.items.len() - 1]), doc::nil())
    } else {
        doc::if_break(doc::text(","), doc::nil())
    };
    inner.push(trailing_comma);
    // The closing brace's trailing comments are emitted outside the group so
    // a `} // note` doesn't force the whole list to break (which would
    // re-attach the comment differently on the next run).
    let mut close = Vec::new();
    for p in super::trivia::leading_parts(&init.r_brace.leading) {
        close.push(p);
        close.push(doc::hardline());
    }
    close.push(super::trivia::token_text(&init.r_brace));
    let mut out = vec![doc::group(doc::concat(vec![
        tok(&init.l_brace),
        doc::indent(doc::concat(vec![doc::line(), doc::concat(inner)])),
        doc::line(),
        doc::concat(close),
    ]))];
    out.extend(super::trivia::trailing_docs(&init.r_brace.trailing));
    doc::concat(out)
}
