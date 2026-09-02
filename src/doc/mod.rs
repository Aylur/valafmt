pub mod printer;

use std::borrow::Cow;

pub type GroupId = u32;

/// Prettier-style document IR. Built by the `format` module, rendered by
/// `printer`. Semantics follow Prettier's printer (break propagation,
/// if_break, line_suffix), not classic Wadler.
#[derive(Debug, Clone)]
pub enum Doc {
    /// Plain text, must not contain newlines.
    Text(Cow<'static, str>),
    /// Byte-exact text that may contain newlines; its internal lines are
    /// never re-indented and it forces enclosing groups to break when it
    /// spans lines.
    Literal(String),
    Concat(Vec<Doc>),
    Group {
        id: Option<GroupId>,
        should_break: bool,
        doc: Box<Doc>,
    },
    Indent(Box<Doc>),
    /// Space when flat, newline when broken.
    Line,
    /// Nothing when flat, newline when broken.
    SoftLine,
    /// Always a newline; forces enclosing groups to break.
    HardLine,
    /// `broken` when the (nearest or referenced) group breaks, `flat` otherwise.
    IfBreak {
        broken: Box<Doc>,
        flat: Box<Doc>,
        group: Option<GroupId>,
    },
    /// Emitted just before the next newline; invisible to `fits` measurement.
    /// Content is rendered flat.
    LineSuffix(Box<Doc>),
    /// Forces enclosing groups to break, no output of its own.
    BreakParent,
}

pub fn text(s: impl Into<Cow<'static, str>>) -> Doc {
    Doc::Text(s.into())
}

pub fn literal(s: impl Into<String>) -> Doc {
    Doc::Literal(s.into())
}

pub fn concat(docs: Vec<Doc>) -> Doc {
    Doc::Concat(docs)
}

pub fn group(doc: Doc) -> Doc {
    Doc::Group { id: None, should_break: false, doc: Box::new(doc) }
}

pub fn group_with_id(id: GroupId, doc: Doc) -> Doc {
    Doc::Group { id: Some(id), should_break: false, doc: Box::new(doc) }
}

pub fn indent(doc: Doc) -> Doc {
    Doc::Indent(Box::new(doc))
}

pub fn line() -> Doc {
    Doc::Line
}

pub fn softline() -> Doc {
    Doc::SoftLine
}

pub fn hardline() -> Doc {
    Doc::HardLine
}

pub fn if_break(broken: Doc, flat: Doc) -> Doc {
    Doc::IfBreak { broken: Box::new(broken), flat: Box::new(flat), group: None }
}

pub fn if_break_in(id: GroupId, broken: Doc, flat: Doc) -> Doc {
    Doc::IfBreak { broken: Box::new(broken), flat: Box::new(flat), group: Some(id) }
}

pub fn line_suffix(doc: Doc) -> Doc {
    Doc::LineSuffix(Box::new(doc))
}

pub fn break_parent() -> Doc {
    Doc::BreakParent
}

pub fn nil() -> Doc {
    Doc::Concat(Vec::new())
}

/// Join items with a separator doc.
pub fn join(sep: Doc, items: Vec<Doc>) -> Doc {
    let mut out = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(sep.clone());
        }
        out.push(item);
    }
    Doc::Concat(out)
}

#[macro_export]
macro_rules! docs {
    ($($d:expr),* $(,)?) => {
        $crate::doc::Doc::Concat(vec![$($d),*])
    };
}
