use super::{Doc, GroupId};
use crate::options::Options;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Flat,
    Break,
}

/// Render a document to text. `doc` is consumed conceptually; break
/// propagation runs first, then the command-stack print loop.
pub fn print(doc: &Doc, options: &Options) -> String {
    let doc = propagate_breaks(doc).0;
    Printer {
        width: options.line_width as isize,
        indent_width: options.indent as usize,
        out: String::new(),
        col: 0,
        group_modes: HashMap::new(),
        line_suffixes: Vec::new(),
    }
    .print(&doc)
}

/// Bottom-up pass: rebuild the tree, marking every group that contains a
/// hard line, break_parent, or multi-line literal as must-break.
/// Returns (new_doc, forces_break).
fn propagate_breaks(doc: &Doc) -> (Doc, bool) {
    match doc {
        Doc::Text(s) => (Doc::Text(s.clone()), false),
        Doc::Literal(s) => (Doc::Literal(s.clone()), s.contains('\n')),
        Doc::Concat(children) => {
            let mut forces = false;
            let mut out = Vec::with_capacity(children.len());
            for c in children {
                let (d, f) = propagate_breaks(c);
                forces |= f;
                out.push(d);
            }
            (Doc::Concat(out), forces)
        }
        Doc::Group { id, should_break, doc } => {
            let (inner, f) = propagate_breaks(doc);
            let sb = *should_break || f;
            // A broken group forces its parents to break too.
            (Doc::Group { id: *id, should_break: sb, doc: Box::new(inner) }, sb)
        }
        Doc::Indent(inner) => {
            let (d, f) = propagate_breaks(inner);
            (Doc::Indent(Box::new(d)), f)
        }
        Doc::Line => (Doc::Line, false),
        Doc::SoftLine => (Doc::SoftLine, false),
        Doc::HardLine => (Doc::HardLine, true),
        Doc::IfBreak { broken, flat, group } => {
            // Contents chosen at print time; measure neither as forcing.
            let (b, _) = propagate_breaks(broken);
            let (f, _) = propagate_breaks(flat);
            (Doc::IfBreak { broken: Box::new(b), flat: Box::new(f), group: *group }, false)
        }
        Doc::LineSuffix(inner) => {
            let (d, _) = propagate_breaks(inner);
            (Doc::LineSuffix(Box::new(d)), false)
        }
        Doc::BreakParent => (Doc::BreakParent, true),
    }
}

/// A print command: (indentation level in spaces, mode, doc).
type Cmd<'a> = (usize, Mode, &'a Doc);

struct Printer<'a> {
    width: isize,
    indent_width: usize,
    out: String,
    col: usize,
    group_modes: HashMap<GroupId, Mode>,
    line_suffixes: Vec<Cmd<'a>>,
}

impl<'a> Printer<'a> {
    fn print(mut self, doc: &'a Doc) -> String {
        let mut cmds: Vec<Cmd<'a>> = vec![(0, Mode::Break, doc)];
        while let Some((ind, mode, doc)) = cmds.pop() {
            match doc {
                Doc::Text(s) => {
                    self.out.push_str(s);
                    self.col += str_width(s);
                }
                Doc::Literal(s) => {
                    self.out.push_str(s);
                    if let Some(last) = s.rfind('\n') {
                        self.col = str_width(&s[last + 1..]);
                    } else {
                        self.col += str_width(s);
                    }
                }
                Doc::Concat(children) => {
                    for c in children.iter().rev() {
                        cmds.push((ind, mode, c));
                    }
                }
                Doc::Group { id, should_break, doc: inner } => {
                    let group_mode = if *should_break {
                        Mode::Break
                    } else if self.fits(&(ind, Mode::Flat, inner), &cmds) {
                        Mode::Flat
                    } else {
                        Mode::Break
                    };
                    if let Some(id) = id {
                        self.group_modes.insert(*id, group_mode);
                    }
                    cmds.push((ind, group_mode, inner));
                }
                Doc::Indent(inner) => {
                    cmds.push((ind + self.indent_width, mode, inner));
                }
                Doc::Line => match mode {
                    Mode::Flat => {
                        self.out.push(' ');
                        self.col += 1;
                    }
                    Mode::Break => self.newline(ind),
                },
                Doc::SoftLine => {
                    if mode == Mode::Break {
                        self.newline(ind);
                    }
                }
                Doc::HardLine => self.newline(ind),
                Doc::IfBreak { broken, flat, group } => {
                    let m = match group {
                        Some(id) => self.group_modes.get(id).copied().unwrap_or(mode),
                        None => mode,
                    };
                    let chosen = if m == Mode::Break { broken } else { flat };
                    cmds.push((ind, mode, chosen));
                }
                Doc::LineSuffix(inner) => {
                    self.line_suffixes.push((ind, Mode::Flat, inner));
                }
                Doc::BreakParent => {}
            }
        }
        // Flush any suffixes that never met a newline (end of document).
        self.flush_suffixes();
        self.out
    }

    /// Emit a newline: first flush pending line suffixes, then trim trailing
    /// whitespace on the current line, then break and indent.
    fn newline(&mut self, ind: usize) {
        self.flush_suffixes();
        trim_trailing_spaces(&mut self.out);
        self.out.push('\n');
        for _ in 0..ind {
            self.out.push(' ');
        }
        self.col = ind;
    }

    /// Render pending line-suffix content inline (flat, no measurement).
    fn flush_suffixes(&mut self) {
        if self.line_suffixes.is_empty() {
            return;
        }
        let suffixes = std::mem::take(&mut self.line_suffixes);
        for (_, _, doc) in suffixes {
            let rendered = render_flat(doc);
            self.out.push_str(&rendered);
            self.col += str_width(&rendered);
        }
    }

    /// Would `next` (in flat mode) plus the remaining commands fit on the
    /// current line?
    fn fits(&self, next: &Cmd<'a>, rest: &[Cmd<'a>]) -> bool {
        let mut remaining = self.width - self.col as isize;
        // Local stack seeded with `next`; fall back to `rest` (in original
        // order from the end) when it empties.
        let mut rest_idx = rest.len();
        let mut cmds: Vec<Cmd<'a>> = vec![*next];
        loop {
            let (ind, mode, doc) = match cmds.pop() {
                Some(c) => c,
                None => {
                    if rest_idx == 0 {
                        return true;
                    }
                    rest_idx -= 1;
                    rest[rest_idx]
                }
            };
            if remaining < 0 {
                return false;
            }
            match doc {
                Doc::Text(s) => remaining -= str_width(s) as isize,
                Doc::Literal(s) => {
                    if s.contains('\n') {
                        return true;
                    }
                    remaining -= str_width(s) as isize;
                }
                Doc::Concat(children) => {
                    for c in children.iter().rev() {
                        cmds.push((ind, mode, c));
                    }
                }
                Doc::Group { should_break, doc: inner, .. } => {
                    let m = if *should_break { Mode::Break } else { mode };
                    cmds.push((ind, m, inner));
                }
                Doc::Indent(inner) => cmds.push((ind + self.indent_width, mode, inner)),
                Doc::Line => match mode {
                    Mode::Flat => remaining -= 1,
                    Mode::Break => return true,
                },
                Doc::SoftLine => {
                    if mode == Mode::Break {
                        return true;
                    }
                }
                Doc::HardLine => return true,
                Doc::IfBreak { broken, flat, group } => {
                    let m = match group {
                        Some(id) => self.group_modes.get(id).copied().unwrap_or(mode),
                        None => mode,
                    };
                    let chosen = if m == Mode::Break { broken } else { flat };
                    cmds.push((ind, mode, chosen));
                }
                Doc::LineSuffix(_) => {}
                Doc::BreakParent => {}
            }
            if remaining < 0 {
                return false;
            }
        }
    }
}

fn render_flat(doc: &Doc) -> String {
    let mut out = String::new();
    let mut stack = vec![doc];
    while let Some(d) = stack.pop() {
        match d {
            Doc::Text(s) => out.push_str(s),
            Doc::Literal(s) => out.push_str(s),
            Doc::Concat(children) => stack.extend(children.iter().rev()),
            Doc::Group { doc, .. } | Doc::Indent(doc) | Doc::LineSuffix(doc) => stack.push(doc),
            Doc::Line => out.push(' '),
            Doc::SoftLine | Doc::HardLine | Doc::BreakParent => {}
            Doc::IfBreak { flat, .. } => stack.push(flat),
        }
    }
    out
}

fn trim_trailing_spaces(out: &mut String) {
    let trimmed = out.trim_end_matches([' ', '\t']).len();
    // Only trim back to the last newline, never across it.
    let line_start = out.rfind('\n').map(|i| i + 1).unwrap_or(0);
    out.truncate(trimmed.max(line_start));
}

/// Display width of a string: counts characters, not bytes. Good enough for
/// code (tabs don't appear in generated output; East Asian width is ignored).
fn str_width(s: &str) -> usize {
    s.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::*;

    fn opts(width: u16) -> Options {
        Options { line_width: width, indent: 4 }
    }

    #[test]
    fn flat_when_fits() {
        let d = group(concat(vec![
            text("foo("),
            indent(concat(vec![softline(), text("a,"), line(), text("b")])),
            softline(),
            text(")"),
        ]));
        assert_eq!(print(&d, &opts(80)), "foo(a, b)");
    }

    #[test]
    fn breaks_when_too_wide() {
        let d = group(concat(vec![
            text("foo("),
            indent(concat(vec![softline(), text("aaaaaaaa,"), line(), text("bbbbbbbb")])),
            softline(),
            text(")"),
        ]));
        assert_eq!(print(&d, &opts(10)), "foo(\n    aaaaaaaa,\n    bbbbbbbb\n)");
    }

    #[test]
    fn hardline_propagates() {
        let d = group(concat(vec![
            text("call("),
            indent(concat(vec![softline(), text("() => {"), indent(hardline()), hardline(), text("}")])),
            softline(),
            text(")"),
        ]));
        // The hardline inside forces the outer group to break even at width 80.
        let out = print(&d, &opts(80));
        assert!(out.starts_with("call(\n"), "got: {out}");
    }

    #[test]
    fn if_break_trailing_comma() {
        let mk = || {
            group(concat(vec![
                text("f("),
                indent(concat(vec![softline(), text("x"), if_break(text(","), nil())])),
                softline(),
                text(")"),
            ]))
        };
        assert_eq!(print(&mk(), &opts(80)), "f(x)");
        let d2 = concat(vec![text("aaaaaaaaaaaaaaaaaaaa = "), mk()]);
        assert_eq!(print(&d2, &opts(10)), "aaaaaaaaaaaaaaaaaaaa = f(\n    x,\n)");
    }

    #[test]
    fn line_suffix_comment() {
        let d = concat(vec![
            text("int a;"),
            line_suffix(text(" // hi")),
            hardline(),
            text("int b;"),
        ]);
        assert_eq!(print(&d, &opts(80)), "int a; // hi\nint b;");
    }

    #[test]
    fn literal_not_reindented() {
        let d = group(concat(vec![
            text("var s = "),
            indent(concat(vec![softline(), literal("\"\"\"a\n  b\"\"\""), text(";")])),
        ]));
        let out = print(&d, &opts(80));
        assert!(out.contains("\n  b\"\"\""), "got: {out}");
    }
}
