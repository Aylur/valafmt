/// The only formatting knobs valafmt has, by design.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Preferred maximum line width; groups break when they would exceed it.
    pub line_width: u16,
    /// Indentation width in spaces.
    pub indent: u8,
}

impl Default for Options {
    fn default() -> Self {
        Options { line_width: 100, indent: 4 }
    }
}
