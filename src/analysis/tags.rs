use super::language::Language;
use super::{Symbol, SymbolKind};
use std::cell::RefCell;
use tree_sitter_tags::{Tag, TagsConfiguration, TagsContext};

thread_local! {
    static TAGS_CONTEXT: RefCell<TagsContext> = RefCell::new(TagsContext::new());
}

pub fn extract_symbols(language: Language, source: &str) -> (Vec<Symbol>, Vec<Symbol>) {
    let Some(configuration) = language.tags_configuration() else {
        return (Vec::new(), Vec::new());
    };
    let tags = generate_tags(configuration, source);
    let line_starts = compute_line_starts(source);
    let mut definitions = Vec::new();
    let mut references = Vec::new();
    for tag in &tags {
        let symbol = build_symbol(configuration, source, &line_starts, tag);
        match tag.is_definition {
            true => definitions.push(symbol),
            false => references.push(symbol),
        }
    }
    (definitions, references)
}

fn generate_tags(configuration: &TagsConfiguration, source: &str) -> Vec<Tag> {
    TAGS_CONTEXT.with(|context| {
        let mut context = context.borrow_mut();
        match context.generate_tags(configuration, source.as_bytes(), None) {
            Ok((tags, _)) => tags.map_while(Result::ok).collect(),
            Err(_) => Vec::new(),
        }
    })
}

fn build_symbol(
    configuration: &TagsConfiguration,
    source: &str,
    line_starts: &[usize],
    tag: &Tag,
) -> Symbol {
    let name_bytes = source.as_bytes().get(tag.name_range.clone()).unwrap_or(&[]);
    Symbol {
        name: String::from_utf8_lossy(name_bytes).into_owned(),
        kind: map_kind(configuration.syntax_type_name(tag.syntax_type_id)),
        line: line_of(line_starts, tag.name_range.start),
    }
}

fn map_kind(syntax_type_name: &str) -> SymbolKind {
    match syntax_type_name {
        "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "class" | "struct" | "enum" | "union" => SymbolKind::Class,
        "interface" => SymbolKind::Interface,
        "module" | "namespace" => SymbolKind::Module,
        "macro" => SymbolKind::Macro,
        "constant" | "variable" => SymbolKind::Constant,
        "type" | "implementation" => SymbolKind::Type,
        "call" | "reference" => SymbolKind::Call,
        _ => SymbolKind::Other,
    }
}

fn compute_line_starts(source: &str) -> Vec<usize> {
    let newline_ends = source
        .bytes()
        .enumerate()
        .filter(|(_, byte)| *byte == b'\n')
        .map(|(index, _)| index + 1);
    std::iter::once(0).chain(newline_ends).collect()
}

fn line_of(line_starts: &[usize], byte_offset: usize) -> usize {
    line_starts.partition_point(|start| *start <= byte_offset) - 1
}
