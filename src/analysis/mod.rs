use language::Language;
use std::ops::Range;
use std::path::Path;

mod highlight;
mod language;
mod outline;
mod tags;

#[derive(Debug, Clone, Default)]
pub struct FileAnalysis {
    pub highlights: Vec<HighlightSpan>,
    pub definitions: Vec<Symbol>,
    pub references: Vec<Symbol>,
    pub outline: Vec<OutlineItem>,
}

#[derive(Debug, Clone)]
pub struct OutlineItem {
    pub depth: usize,
    pub line: usize,
    pub signature: String,
    pub child_count: usize,
    pub is_constant: bool,
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
    let outline = outline::extract_outline(language, source);
    Some(FileAnalysis {
        highlights,
        definitions,
        references,
        outline,
    })
}

pub fn highlight_text(path: &Path, text: &str) -> Vec<HighlightSpan> {
    Language::detect(path)
        .map(|language| highlight::highlight(language, text))
        .unwrap_or_default()
}

pub fn line_comment(path: &Path) -> Option<&'static str> {
    Language::detect(path).map(Language::line_comment)
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
    fn rust_outline_nests_impl_methods() {
        let source = "pub struct Camera {\n    zoom: f64,\n}\n\nimpl Camera {\n    \
            pub fn new(\n        zoom: f64,\n    ) -> Self {\n        Self { zoom }\n    }\n}\n\n\
            const LIMIT: usize = 3;\n";
        let analysis = analyse(Path::new("camera.rs"), source).unwrap();
        let outline: Vec<(usize, &str, usize)> = analysis
            .outline
            .iter()
            .map(|item| (item.depth, item.signature.as_str(), item.child_count))
            .collect();
        assert_eq!(
            outline,
            vec![
                (0, "pub struct Camera", 0),
                (0, "impl Camera", 1),
                (1, "pub fn new(zoom: f64) -> Self", 0),
                (0, "const LIMIT: usize = 3", 0),
            ]
        );
    }

    #[test]
    fn python_outline_unwraps_decorators() {
        let source =
            "class A:\n    @property\n    def b(self):\n        return 1\n\ndef c(x):\n    pass\n";
        let analysis = analyse(Path::new("a.py"), source).unwrap();
        let signatures: Vec<&str> = analysis
            .outline
            .iter()
            .map(|item| item.signature.as_str())
            .collect();
        assert_eq!(signatures, vec!["class A", "def b(self)", "def c(x)"]);
    }

    #[test]
    fn typescript_outline_unwraps_exports() {
        let source = "export class A {\n  run(): void {}\n}\nexport const b = () => 1;\n\
            interface C { d: number }\n";
        let analysis = analyse(Path::new("a.ts"), source).unwrap();
        let signatures: Vec<&str> = analysis
            .outline
            .iter()
            .map(|item| item.signature.as_str())
            .collect();
        assert_eq!(
            signatures,
            vec![
                "class A",
                "run(): void",
                "const b = () => 1",
                "interface C",
                "d: number"
            ]
        );
    }

    fn outline_signatures(path: &str, source: &str) -> Vec<String> {
        analyse(Path::new(path), source)
            .unwrap()
            .outline
            .iter()
            .map(|item| format!("{}{}", "  ".repeat(item.depth), item.signature))
            .collect()
    }

    #[test]
    fn rust_outline_lists_public_fields() {
        let source = "pub struct A {\n    pub b: u8,\n    c: u8,\n    pub(crate) d: u8,\n}\n";
        assert_eq!(
            outline_signatures("a.rs", source),
            vec!["pub struct A", "  pub b: u8", "  pub(crate) d: u8"]
        );
    }

    #[test]
    fn python_outline_lists_public_class_attributes() {
        let source =
            "X = 1\nclass A:\n    b: int = 1\n    _c: int\n    def d(self):\n        pass\n";
        assert_eq!(
            outline_signatures("a.py", source),
            vec!["class A", "  b: int = 1", "  def d(self)"]
        );
    }

    #[test]
    fn typescript_outline_lists_public_fields() {
        let source = "class A {\n  b: number = 1;\n  private c = 2;\n  #d = 3;\n  public e?: string;\n}\n\
            interface I {\n  f: number;\n  g(): void;\n}\n";
        assert_eq!(
            outline_signatures("a.ts", source),
            vec![
                "class A",
                "  b: number = 1",
                "  public e?: string",
                "interface I",
                "  f: number",
                "  g(): void"
            ]
        );
    }

    #[test]
    fn javascript_outline_lists_public_fields() {
        let source = "class A {\n  b = 1;\n  #c = 2;\n}\n";
        assert_eq!(
            outline_signatures("a.js", source),
            vec!["class A", "  b = 1"]
        );
    }

    #[test]
    fn go_outline_lists_exported_fields() {
        let source = "package a\ntype A struct {\n    B int\n    c string\n    D, E int\n}\n";
        assert_eq!(
            outline_signatures("a.go", source),
            vec!["type A struct", "  B int", "  D, E int"]
        );
    }

    #[test]
    fn broken_source_does_not_panic() {
        let analysis = analyse(Path::new("a.rs"), "fn (((( {{ \"unterminated");
        assert!(analysis.is_some());
    }
}
