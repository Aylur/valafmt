/// A simple byte-oriented cursor over source text. Vala source is expected to
/// be UTF-8; all syntactically significant characters are ASCII, so byte-wise
/// scanning is safe as long as multi-byte sequences are only ever copied
/// through verbatim (identifier bodies, strings, comments).
pub struct Cursor<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(src: &'a str) -> Self {
        Cursor { src, pos: 0 }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn is_eof(&self) -> bool {
        self.pos >= self.src.len()
    }

    pub fn peek(&self) -> u8 {
        self.src.as_bytes().get(self.pos).copied().unwrap_or(0)
    }

    pub fn peek_at(&self, n: usize) -> u8 {
        self.src.as_bytes().get(self.pos + n).copied().unwrap_or(0)
    }

    pub fn bump(&mut self) -> u8 {
        let b = self.peek();
        if !self.is_eof() {
            self.pos += 1;
        }
        b
    }

    /// Advance past one full UTF-8 character (used inside identifiers,
    /// comments, and string bodies where non-ASCII may appear).
    pub fn bump_char(&mut self) {
        if let Some(c) = self.src[self.pos..].chars().next() {
            self.pos += c.len_utf8();
        }
    }

    pub fn eat(&mut self, b: u8) -> bool {
        if self.peek() == b {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub fn starts_with(&self, s: &str) -> bool {
        self.src[self.pos..].starts_with(s)
    }

    pub fn slice_from(&self, start: usize) -> &'a str {
        &self.src[start..self.pos]
    }

    /// Line and column (1-based) of a byte offset, for diagnostics.
    pub fn line_col(src: &str, offset: usize) -> (usize, usize) {
        let mut line = 1;
        let mut col = 1;
        for (i, c) in src.char_indices() {
            if i >= offset {
                break;
            }
            if c == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }
}
