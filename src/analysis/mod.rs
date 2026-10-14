use std::ops::Range;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct FileAnalysis {
    pub highlights: Vec<HighlightSpan>,
    pub definitions: Vec<Symbol>,
    pub references: Vec<Symbol>,
}

#[derive(Debug, Clone)]
pub struct HighlightSpan {
    pub byte_range: Range<usize>,
    pub kind: HighlightKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HighlightKind {
    Keyword,
    String,
    Comment,
    Function,
    Type,
    Constant,
    Number,
    Attribute,
    Macro,
    Property,
    Operator,
    Punctuation,
    Variable,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Interface,
    Module,
    Macro,
    Constant,
    Type,
    Call,
    Other,
}

pub fn analyse(_path: &Path, _source: &str) -> Option<FileAnalysis> {
    None
}
