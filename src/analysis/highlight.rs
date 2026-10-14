use super::language::{Language, RECOGNIZED_HIGHLIGHT_NAMES};
use super::{HighlightKind, HighlightSpan};
use std::cell::RefCell;
use std::ops::Range;
use tree_sitter_highlight::{Highlight, HighlightEvent, Highlighter};

thread_local! {
    static HIGHLIGHTER: RefCell<Highlighter> = RefCell::new(Highlighter::new());
}

pub fn highlight(language: Language, source: &str) -> Vec<HighlightSpan> {
    let Some(configuration) = language.highlight_configuration() else {
        return Vec::new();
    };
    HIGHLIGHTER.with(|highlighter| {
        let mut highlighter = highlighter.borrow_mut();
        let Ok(events) = highlighter.highlight(configuration, source.as_bytes(), None, |_| None)
        else {
            return Vec::new();
        };
        flatten_events(events.map_while(Result::ok))
    })
}

fn flatten_events(events: impl Iterator<Item = HighlightEvent>) -> Vec<HighlightSpan> {
    let mut spans: Vec<HighlightSpan> = Vec::new();
    let mut stack: Vec<Option<HighlightKind>> = Vec::new();
    for event in events {
        match event {
            HighlightEvent::HighlightStart(highlight) => stack.push(kind_of(highlight)),
            HighlightEvent::HighlightEnd => {
                stack.pop();
            }
            HighlightEvent::Source { start, end } => {
                let kind = stack.iter().rev().find_map(|kind| *kind);
                push_span(&mut spans, start..end, kind);
            }
        }
    }
    spans
}

fn push_span(
    spans: &mut Vec<HighlightSpan>,
    byte_range: Range<usize>,
    kind: Option<HighlightKind>,
) {
    let Some(kind) = kind else {
        return;
    };
    if byte_range.is_empty() {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.kind == kind
        && last.byte_range.end == byte_range.start
    {
        last.byte_range.end = byte_range.end;
        return;
    }
    spans.push(HighlightSpan { byte_range, kind });
}

fn kind_of(highlight: Highlight) -> Option<HighlightKind> {
    let name = RECOGNIZED_HIGHLIGHT_NAMES.get(highlight.0)?;
    match *name {
        "keyword" => Some(HighlightKind::Keyword),
        "string" | "escape" => Some(HighlightKind::String),
        "comment" => Some(HighlightKind::Comment),
        "function" => Some(HighlightKind::Function),
        "function.macro" | "macro" => Some(HighlightKind::Macro),
        "type" | "constructor" | "tag" | "module" | "namespace" => Some(HighlightKind::Type),
        "constant" | "boolean" | "label" => Some(HighlightKind::Constant),
        "number" => Some(HighlightKind::Number),
        "attribute" => Some(HighlightKind::Attribute),
        "property" => Some(HighlightKind::Property),
        "operator" => Some(HighlightKind::Operator),
        "punctuation" => Some(HighlightKind::Punctuation),
        "variable" => Some(HighlightKind::Variable),
        _ => None,
    }
}
