use language::Language;
use std::ops::Range;
use std::path::Path;

mod highlight;
mod language;
mod tags;

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

pub fn analyse(path: &Path, source: &str) -> Option<FileAnalysis> {
    let language = Language::detect(path)?;
    let highlights = highlight::highlight(language, source);
    let (definitions, references) = tags::extract_symbols(language, source);
    Some(FileAnalysis {
        highlights,
        definitions,
        references,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_SOURCE: &str = "fn helper() {}\n\nfn main() {\n    helper();\n}\n";

    #[test]
    fn unknown_extension_is_none() {
        assert!(analyse(Path::new("notes.txt"), "hello").is_none());
        assert!(analyse(Path::new("Makefile"), "all:").is_none());
    }

    #[test]
    fn rust_keyword_is_highlighted() {
        let analysis = analyse(Path::new("main.rs"), RUST_SOURCE).unwrap();
        let has_fn_keyword = analysis
            .highlights
            .iter()
            .any(|span| span.byte_range == (0..2) && span.kind == HighlightKind::Keyword);
        assert!(has_fn_keyword, "{:?}", analysis.highlights);
    }

    #[test]
    fn rust_highlights_are_sorted_and_disjoint() {
        let analysis = analyse(Path::new("main.rs"), RUST_SOURCE).unwrap();
        let are_disjoint = analysis
            .highlights
            .windows(2)
            .all(|pair| pair[0].byte_range.end <= pair[1].byte_range.start);
        assert!(are_disjoint);
        assert!(!analysis.highlights.is_empty());
    }

    #[test]
    fn rust_definition_and_reference() {
        let analysis = analyse(Path::new("main.rs"), RUST_SOURCE).unwrap();
        let main = analysis.definitions.iter().find(|s| s.name == "main");
        assert_eq!(
            main.map(|s| (s.kind, s.line)),
            Some((SymbolKind::Function, 2))
        );
        let call = analysis.references.iter().find(|s| s.name == "helper");
        assert_eq!(call.map(|s| (s.kind, s.line)), Some((SymbolKind::Call, 3)));
    }

    #[test]
    fn python_definition_and_highlight() {
        let source = "def greet(name):\n    return name\n\ngreet('x')\n";
        let analysis = analyse(Path::new("a.py"), source).unwrap();
        assert!(analysis.definitions.iter().any(|s| s.name == "greet"));
        assert!(analysis.references.iter().any(|s| s.name == "greet"));
        assert!(
            analysis
                .highlights
                .iter()
                .any(|s| s.kind == HighlightKind::Keyword)
        );
    }

    #[test]
    fn every_language_analyses_without_panic() {
        let cases = [
            ("a.js", "function f() { return g(1); }\nclass A {}\n"),
            (
                "a.jsx",
                "function C() { return <div className=\"a\">hi</div>; }\n",
            ),
            (
                "a.ts",
                "interface I { a: number }\nfunction f(x: I): void {}\n",
            ),
            ("a.tsx", "const x = <div>{f(1)}</div>;\nclass B {}\n"),
            ("a.go", "package main\nfunc f() { g() }\n"),
        ];
        for (name, source) in cases {
            let analysis = analyse(Path::new(name), source).unwrap();
            assert!(!analysis.highlights.is_empty(), "{name}");
            assert!(!analysis.definitions.is_empty(), "{name}");
        }
    }

    #[test]
    fn broken_source_does_not_panic() {
        let analysis = analyse(Path::new("a.rs"), "fn (((( {{ \"unterminated");
        assert!(analysis.is_some());
    }
}
