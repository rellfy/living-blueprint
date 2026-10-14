use crate::resources::text::ColorBrush;
use parley::Layout;

pub const ENTRY_LIFETIME_SECONDS: f64 = 60.0;
const WRAP_COLUMNS: usize = 80;
const MAX_LINES: usize = 10;
const MAX_CHARACTERS: usize = WRAP_COLUMNS * MAX_LINES;
const ELLIPSIS: char = '…';

#[derive(Default)]
pub struct AgentFeed {
    pub entries: Vec<FeedEntry>,
}

pub struct FeedEntry {
    pub text: String,
    pub kind: FeedKind,
    pub age_seconds: f64,
    pub offset_y: Option<f64>,
    pub layout: Option<(f64, Layout<ColorBrush>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedKind {
    Prompt,
    Message,
    ToolUse,
    Success,
    Failure,
}

impl AgentFeed {
    pub fn push(&mut self, kind: FeedKind, text: &str) {
        let text = format_entry_text(text);
        if text.is_empty() {
            return;
        }
        self.entries.push(FeedEntry {
            text,
            kind,
            age_seconds: 0.0,
            offset_y: None,
            layout: None,
        });
    }

    pub fn update(&mut self, delta_time: f64) {
        for entry in &mut self.entries {
            entry.age_seconds += delta_time;
        }
        self.entries
            .retain(|entry| entry.age_seconds < ENTRY_LIFETIME_SECONDS);
    }
}

fn format_entry_text(text: &str) -> String {
    let wrapped = wrap_text(text.trim(), WRAP_COLUMNS);
    let lines: Vec<&str> = wrapped.lines().collect();
    let is_too_many_lines = lines.len() > MAX_LINES;
    let joined = lines[..lines.len().min(MAX_LINES)].join("\n");
    let is_too_long = joined.chars().count() > MAX_CHARACTERS;
    match is_too_many_lines || is_too_long {
        true => truncate_with_ellipsis(&joined, MAX_CHARACTERS),
        false => joined,
    }
}

fn wrap_text(text: &str, columns: usize) -> String {
    text.lines()
        .map(|line| wrap_line(line.trim_end(), columns))
        .collect::<Vec<_>>()
        .join("\n")
}

fn wrap_line(line: &str, columns: usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in line.split(' ') {
        let current_length = current.chars().count();
        let word_length = word.chars().count();
        let is_overflowing = current_length > 0 && current_length + 1 + word_length > columns;
        if is_overflowing {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
        while current.chars().count() > columns {
            let head: String = current.chars().take(columns).collect();
            let tail: String = current.chars().skip(columns).collect();
            lines.push(head);
            current = tail;
        }
    }
    lines.push(current);
    lines.join("\n")
}

fn truncate_with_ellipsis(text: &str, max_characters: usize) -> String {
    let mut truncated: String = text.chars().take(max_characters - 1).collect();
    truncated = truncated.trim_end().to_string();
    truncated.push(ELLIPSIS);
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_at_eighty_columns() {
        let text = "word ".repeat(40);
        let formatted = format_entry_text(&text);
        assert!(
            formatted
                .lines()
                .all(|line| line.chars().count() <= WRAP_COLUMNS)
        );
        assert!(formatted.lines().count() > 1);
    }

    #[test]
    fn breaks_words_longer_than_a_line() {
        let formatted = format_entry_text(&"x".repeat(200));
        assert!(
            formatted
                .lines()
                .all(|line| line.chars().count() <= WRAP_COLUMNS)
        );
    }

    #[test]
    fn trims_to_ten_lines_with_ellipsis() {
        let text = "line\n".repeat(30);
        let formatted = format_entry_text(&text);
        assert!(formatted.lines().count() <= MAX_LINES);
        assert!(formatted.ends_with(ELLIPSIS));
    }

    #[test]
    fn keeps_short_text_intact() {
        assert_eq!(format_entry_text("  hello world \n"), "hello world");
    }
}
