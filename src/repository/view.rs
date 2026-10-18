use crate::analysis::{HighlightKind, HighlightSpan, OutlineItem, highlight_text, line_comment};
use crate::repository::{LineSpan, Node};
use std::path::Path;
use std::sync::Arc;

const OUTLINE_INDENT: &str = "    ";
const MAX_LISTING_ENTRIES: usize = 24;

pub struct TextView {
    pub text: Arc<str>,
    pub line_spans: Vec<LineSpan>,
    pub highlights: Vec<HighlightSpan>,
    pub source_lines: Option<Vec<usize>>,
}

#[derive(Clone, Copy)]
pub struct BodyView<'a> {
    pub text: &'a str,
    pub line_spans: &'a [LineSpan],
    pub highlights: &'a [HighlightSpan],
    pub line_numbers: LineNumbers<'a>,
}

#[derive(Clone, Copy)]
pub enum LineNumbers<'a> {
    Sequential,
    Mapped(&'a [usize]),
    Hidden,
}

impl LineNumbers<'_> {
    pub fn collect(&self, row_count: usize) -> Option<Vec<usize>> {
        match self {
            Self::Sequential => Some((1..=row_count).collect()),
            Self::Mapped(lines) => Some(lines.to_vec()),
            Self::Hidden => None,
        }
    }

    pub fn max(&self, row_count: usize) -> Option<usize> {
        match self {
            Self::Sequential => Some(row_count.max(1)),
            Self::Mapped(lines) => lines.iter().copied().max(),
            Self::Hidden => None,
        }
    }
}

impl TextView {
    pub fn new(text: String, highlights: Vec<HighlightSpan>) -> Self {
        Self {
            line_spans: measure_lines(&text),
            text: Arc::from(text),
            highlights,
            source_lines: None,
        }
    }

    fn with_source_lines(mut self, source_lines: Vec<usize>) -> Self {
        if source_lines.len() == self.line_spans.len() {
            self.source_lines = Some(source_lines);
        }
        self
    }

    pub fn as_body(&self) -> BodyView<'_> {
        BodyView {
            text: &self.text,
            line_spans: &self.line_spans,
            highlights: &self.highlights,
            line_numbers: match &self.source_lines {
                Some(lines) => LineNumbers::Mapped(lines),
                None => LineNumbers::Hidden,
            },
        }
    }
}

pub fn build_outline_view(path: &Path, outline: &[OutlineItem]) -> Option<TextView> {
    if outline.is_empty() {
        return None;
    }
    let text = outline
        .iter()
        .map(|item| format!("{}{}", OUTLINE_INDENT.repeat(item.depth), item.signature))
        .collect::<Vec<_>>()
        .join("\n");
    let highlights = highlight_text(path, &text);
    let source_lines = outline.iter().map(|item| item.line + 1).collect();
    Some(TextView::new(text, highlights).with_source_lines(source_lines))
}

pub fn build_summary_view(path: &Path, outline: &[OutlineItem]) -> Option<TextView> {
    let comment = line_comment(path)?;
    let items: Vec<&OutlineItem> = outline
        .iter()
        .filter(|item| item.depth == 0 && !item.is_constant)
        .collect();
    let text = items
        .iter()
        .map(|item| summarise_item(item, comment))
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        return None;
    }
    let highlights = highlight_text(path, &text);
    let source_lines = items.iter().map(|item| item.line + 1).collect();
    Some(TextView::new(text, highlights).with_source_lines(source_lines))
}

pub fn build_listing_view(children: &[&Node]) -> TextView {
    let mut text = String::new();
    let mut highlights = Vec::new();
    for child in children.iter().take(MAX_LISTING_ENTRIES) {
        let start = text.len();
        text.push_str(&child.name);
        if child.get_is_directory() {
            text.push('/');
            highlights.push(HighlightSpan {
                byte_range: start..text.len(),
                kind: HighlightKind::Type,
            });
        }
        text.push('\n');
    }
    let hidden_count = children.len().saturating_sub(MAX_LISTING_ENTRIES);
    if hidden_count > 0 {
        let start = text.len();
        text.push_str(&format!("… {hidden_count} more"));
        highlights.push(HighlightSpan {
            byte_range: start..text.len(),
            kind: HighlightKind::Comment,
        });
    }
    TextView::new(text.trim_end().to_string(), highlights)
}

pub fn measure_lines(text: &str) -> Vec<LineSpan> {
    text.lines().map(measure_line).collect()
}

fn summarise_item(item: &OutlineItem, comment: &str) -> String {
    let name = match item.signature.split_once('(') {
        Some((head, _)) => head.trim_end(),
        None => &item.signature,
    };
    match item.child_count {
        0 => name.to_string(),
        1 => format!("{name}  {comment} 1 item"),
        count => format!("{name}  {comment} {count} items"),
    }
}

fn measure_line(line: &str) -> LineSpan {
    let length = line.chars().count() as u32;
    let trimmed_length = line.trim_start().chars().count() as u32;
    LineSpan {
        indent: length - trimmed_length,
        length,
    }
}
