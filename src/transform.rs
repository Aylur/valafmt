//! Source-level canonicalizations applied before formatting.
//!
//! A namespace that contains exactly one class collapses into the class name:
//!
//! ```vala
//! namespace NS {          =>      class NS.Klass {
//!     class Klass { ... }             ...
//! }                               }
//! ```
//!
//! This changes the token stream, so it runs as a text edit on the source and
//! the edited source is re-parsed; the token-stream safety check then guards
//! everything downstream of the (tiny) edit.

use crate::lexer;
use crate::parser;
use crate::syntax::ast::*;

struct Edit {
    start: usize,
    end: usize,
    replacement: String,
}

/// Apply canonicalizations until a fixpoint (bounded); returns the possibly
/// rewritten source. On any lex/parse hiccup the input is returned unchanged.
pub fn apply(src: &str) -> String {
    let mut cur = src.to_string();
    // Nested namespaces collapse one level per pass.
    for _ in 0..16 {
        let lexed = lexer::lex(&cur);
        if !lexed.errors.is_empty() {
            return cur;
        }
        let Ok(file) = parser::parse(lexed.tokens) else {
            return cur;
        };
        let mut edits = Vec::new();
        collect_namespace_collapses(&file.members, &mut edits);
        if edits.is_empty() {
            return cur;
        }
        edits.sort_by_key(|e| std::cmp::Reverse(e.start));
        for e in edits {
            cur.replace_range(e.start..e.end, &e.replacement);
        }
    }
    cur
}

fn collect_namespace_collapses(members: &[Member], edits: &mut Vec<Edit>) {
    for m in members {
        let MemberKind::Namespace(n) = &m.kind else { continue };
        if m.attributes.is_empty()
            && n.modifiers.is_empty()
            && n.name.global.is_none()
            && n.body.members.len() == 1
        {
            if let MemberKind::Class(c) = &n.body.members[0].kind {
                if c.name.global.is_none() && collapse_is_comment_safe(n) {
                    // Delete `namespace NS ... {` (through the open brace).
                    edits.push(Edit {
                        start: n.namespace_kw.offset,
                        end: n.body.l_brace.offset + n.body.l_brace.text.len(),
                        replacement: String::new(),
                    });
                    // Prefix the class name with the namespace name.
                    let ns_name: String =
                        n.name.parts.iter().map(|t| t.text.as_str()).collect();
                    let name_tok = c.name.first_token();
                    edits.push(Edit {
                        start: name_tok.offset,
                        end: name_tok.offset,
                        replacement: format!("{ns_name}."),
                    });
                    // Delete the namespace's closing `}`.
                    edits.push(Edit {
                        start: n.body.r_brace.offset,
                        end: n.body.r_brace.offset + n.body.r_brace.text.len(),
                        replacement: String::new(),
                    });
                    // The collapsed body is revisited on the next pass.
                    continue;
                }
            }
        }
        collect_namespace_collapses(&n.body.members, edits);
    }
}

/// The deletion range `namespace NS ... {` must not swallow comments (the
/// safety check can't see across this edit). Comments before `namespace` and
/// after `{` survive on their own.
fn collapse_is_comment_safe(n: &NamespaceDecl) -> bool {
    let mut toks: Vec<&crate::lexer::token::Token> = vec![&n.namespace_kw];
    toks.extend(n.name.parts.iter());
    toks.push(&n.body.l_brace);
    for (i, t) in toks.iter().enumerate() {
        // Leading pieces of the first token are outside the deleted range.
        if i > 0 && crate::format::trivia::has_leading_pieces(t) {
            return false;
        }
        // Trailing comments of `{` are outside the range and survive.
        if t.kind != crate::lexer::token::TokenKind::LBrace && t.has_trailing_comment() {
            return false;
        }
    }
    true
}
