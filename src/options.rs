/// The only formatting knobs valafmt has, by design.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Preferred maximum line width; groups break when they would exceed it.
    pub width: u16,
    /// Indentation width in columns. With `use_tabs` this is the tab width
    /// used when measuring lines against `width`.
    pub indent: u8,
    /// Indent with tabs instead of spaces.
    pub use_tabs: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            width: 100,
            indent: 4,
            use_tabs: false,
        }
    }
}
