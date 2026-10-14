use std::ops::Range;

const MAX_HISTORY: usize = 200;

pub struct Prompt {
    text: String,
    caret: usize,
    anchor: usize,
    history: EditHistory,
    pub visibility: f64,
    pub caret_phase: f64,
    pub scroll_x: f64,
    pub scroll_y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    CharLeft,
    CharRight,
    WordLeft,
    WordRight,
    Start,
    End,
    LineUp,
    LineDown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditKind {
    Typing,
    Deleting,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Whitespace,
    Word,
    Punctuation,
}

#[derive(Clone)]
struct Snapshot {
    text: String,
    caret: usize,
    anchor: usize,
}

struct EditHistory {
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
    last_edit: Option<EditKind>,
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            caret: 0,
            anchor: 0,
            history: EditHistory::new(),
            visibility: 0.0,
            caret_phase: 0.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.caret
    }

    pub fn anchor(&self) -> usize {
        self.anchor
    }

    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    pub fn get_is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn get_has_selection(&self) -> bool {
        self.caret != self.anchor
    }

    pub fn get_is_caret_after_backslash(&self) -> bool {
        !self.get_has_selection() && self.text[..self.caret].ends_with('\\')
    }

    pub fn insert(&mut self, text: &str) {
        let printable: String = text.chars().filter(|c| !c.is_control()).collect();
        if printable.is_empty() {
            return;
        }
        let kind = match printable.chars().any(char::is_whitespace) {
            true => EditKind::Other,
            false => EditKind::Typing,
        };
        self.record(kind);
        self.replace_selection(&printable);
    }

    pub fn break_line(&mut self) {
        self.record(EditKind::Other);
        self.replace_selection("\n");
    }

    pub fn break_escaped_line(&mut self) {
        if !self.get_is_caret_after_backslash() {
            return;
        }
        self.record(EditKind::Other);
        self.anchor = self.caret - '\\'.len_utf8();
        self.replace_selection("\n");
    }

    pub fn paste(&mut self, text: &str) {
        let normalised = text.replace("\r\n", "\n").replace(['\r', '\t'], " ");
        let printable: String = normalised
            .chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .collect();
        if printable.is_empty() {
            return;
        }
        self.record(EditKind::Other);
        self.replace_selection(&printable);
    }

    pub fn delete(&mut self, motion: Motion) {
        if !self.get_has_selection() {
            self.anchor = self.find_target(motion);
        }
        if !self.get_has_selection() {
            return;
        }
        self.record(EditKind::Deleting);
        self.replace_selection("");
    }

    pub fn delete_selection(&mut self) {
        if !self.get_has_selection() {
            return;
        }
        self.record(EditKind::Other);
        self.replace_selection("");
    }

    pub fn move_caret(&mut self, motion: Motion, is_extending: bool) {
        let target = match (self.get_has_selection() && !is_extending, motion) {
            (true, Motion::CharLeft) => self.selection().start,
            (true, Motion::CharRight) => self.selection().end,
            _ => self.find_target(motion),
        };
        self.caret = target;
        if !is_extending {
            self.anchor = target;
        }
        self.end_edit_run();
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.caret = self.text.len();
        self.end_edit_run();
    }

    pub fn collapse_selection(&mut self) {
        self.anchor = self.caret;
        self.end_edit_run();
    }

    pub fn clear(&mut self) {
        if self.text.is_empty() {
            return;
        }
        self.select_all();
        self.delete_selection();
    }

    pub fn reset(&mut self) {
        self.text.clear();
        self.caret = 0;
        self.anchor = 0;
        self.scroll_x = 0.0;
        self.scroll_y = 0.0;
        self.history = EditHistory::new();
    }

    pub fn undo(&mut self) {
        let Some(snapshot) = self.history.undo_stack.pop() else {
            return;
        };
        let current = self.snapshot();
        self.history.redo_stack.push(current);
        self.restore(snapshot);
    }

    pub fn redo(&mut self) {
        let Some(snapshot) = self.history.redo_stack.pop() else {
            return;
        };
        let current = self.snapshot();
        self.history.undo_stack.push(current);
        self.restore(snapshot);
    }

    fn replace_selection(&mut self, replacement: &str) {
        let selection = self.selection();
        self.text.replace_range(selection.clone(), replacement);
        self.caret = selection.start + replacement.len();
        self.anchor = self.caret;
        self.caret_phase = 0.0;
    }

    fn find_target(&self, motion: Motion) -> usize {
        match motion {
            Motion::CharLeft => self.find_previous_char(),
            Motion::CharRight => self.find_next_char(),
            Motion::WordLeft => self.find_previous_word(),
            Motion::WordRight => self.find_next_word(),
            Motion::Start => 0,
            Motion::End => self.text.len(),
            Motion::LineUp => self.find_line_above(),
            Motion::LineDown => self.find_line_below(),
        }
    }

    fn find_line_above(&self) -> usize {
        let line_start = self.find_line_start(self.caret);
        if line_start == 0 {
            return 0;
        }
        let previous_line_start = self.find_line_start(line_start - 1);
        self.find_column_offset(previous_line_start, self.count_column())
    }

    fn find_line_below(&self) -> usize {
        let Some(newline) = self.text[self.caret..].find('\n') else {
            return self.text.len();
        };
        self.find_column_offset(self.caret + newline + 1, self.count_column())
    }

    fn find_line_start(&self, index: usize) -> usize {
        self.text[..index]
            .rfind('\n')
            .map_or(0, |newline| newline + 1)
    }

    fn count_column(&self) -> usize {
        let line_start = self.find_line_start(self.caret);
        self.text[line_start..self.caret].chars().count()
    }

    fn find_column_offset(&self, line_start: usize, column: usize) -> usize {
        let line = &self.text[line_start..];
        let line_end = line.find('\n').unwrap_or(line.len());
        line[..line_end]
            .char_indices()
            .nth(column)
            .map_or(line_start + line_end, |(index, _)| line_start + index)
    }

    fn find_previous_char(&self) -> usize {
        self.text[..self.caret]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index)
    }

    fn find_next_char(&self) -> usize {
        self.text[self.caret..]
            .chars()
            .next()
            .map_or(self.caret, |c| self.caret + c.len_utf8())
    }

    fn find_previous_word(&self) -> usize {
        let mut chars = self.text[..self.caret].char_indices().rev().peekable();
        while chars
            .next_if(|(_, c)| classify(*c) == CharClass::Whitespace)
            .is_some()
        {}
        let Some(&(_, first)) = chars.peek() else {
            return 0;
        };
        let class = classify(first);
        let mut boundary = self.caret;
        while let Some((index, _)) = chars.next_if(|(_, c)| classify(*c) == class) {
            boundary = index;
        }
        boundary
    }

    fn find_next_word(&self) -> usize {
        let mut chars = self.text[self.caret..].char_indices().peekable();
        while chars
            .next_if(|(_, c)| classify(*c) == CharClass::Whitespace)
            .is_some()
        {}
        let Some(&(_, first)) = chars.peek() else {
            return self.text.len();
        };
        let class = classify(first);
        while chars.next_if(|(_, c)| classify(*c) == class).is_some() {}
        chars
            .peek()
            .map_or(self.text.len(), |(index, _)| self.caret + index)
    }

    fn record(&mut self, kind: EditKind) {
        let is_continuing_run = kind != EditKind::Other && self.history.last_edit == Some(kind);
        self.history.last_edit = Some(kind);
        self.history.redo_stack.clear();
        if is_continuing_run {
            return;
        }
        let snapshot = self.snapshot();
        self.history.undo_stack.push(snapshot);
        if self.history.undo_stack.len() > MAX_HISTORY {
            self.history.undo_stack.remove(0);
        }
    }

    fn end_edit_run(&mut self) {
        self.history.last_edit = None;
        self.caret_phase = 0.0;
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            caret: self.caret,
            anchor: self.anchor,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.caret = snapshot.caret;
        self.anchor = snapshot.anchor;
        self.end_edit_run();
    }
}

impl EditHistory {
    fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            last_edit: None,
        }
    }
}

fn classify(c: char) -> CharClass {
    match c {
        c if c.is_whitespace() => CharClass::Whitespace,
        c if c.is_alphanumeric() || c == '_' => CharClass::Word,
        _ => CharClass::Punctuation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_with(text: &str) -> Prompt {
        let mut prompt = Prompt::new();
        prompt.insert(text);
        prompt
    }

    #[test]
    fn inserts_at_caret() {
        let mut prompt = prompt_with("helo");
        prompt.move_caret(Motion::CharLeft, false);
        prompt.insert("l");
        assert_eq!(prompt.text(), "hello");
        assert_eq!(prompt.caret(), 4);
    }

    #[test]
    fn moves_over_multibyte_chars() {
        let mut prompt = prompt_with("aé😀");
        prompt.move_caret(Motion::CharLeft, false);
        assert_eq!(prompt.caret(), 3);
        prompt.move_caret(Motion::CharLeft, false);
        assert_eq!(prompt.caret(), 1);
        prompt.delete(Motion::CharRight);
        assert_eq!(prompt.text(), "a😀");
    }

    #[test]
    fn jumps_words() {
        let mut prompt = prompt_with("fit the  map.rs now");
        prompt.move_caret(Motion::WordLeft, false);
        assert_eq!(prompt.caret(), 16);
        prompt.move_caret(Motion::WordLeft, false);
        assert_eq!(prompt.caret(), 13);
        prompt.move_caret(Motion::WordLeft, false);
        assert_eq!(prompt.caret(), 12);
        prompt.move_caret(Motion::Start, false);
        prompt.move_caret(Motion::WordRight, false);
        assert_eq!(prompt.caret(), 3);
        prompt.move_caret(Motion::WordRight, false);
        assert_eq!(prompt.caret(), 7);
    }

    #[test]
    fn extends_and_collapses_selection() {
        let mut prompt = prompt_with("hello world");
        prompt.move_caret(Motion::WordLeft, true);
        assert_eq!(prompt.selected_text(), "world");
        prompt.move_caret(Motion::CharLeft, false);
        assert_eq!(prompt.caret(), 6);
        assert!(!prompt.get_has_selection());
    }

    #[test]
    fn replaces_selection_on_insert() {
        let mut prompt = prompt_with("hello world");
        prompt.select_all();
        prompt.insert("x");
        assert_eq!(prompt.text(), "x");
    }

    #[test]
    fn deletes_words() {
        let mut prompt = prompt_with("hello big world");
        prompt.delete(Motion::WordLeft);
        assert_eq!(prompt.text(), "hello big ");
        prompt.move_caret(Motion::Start, false);
        prompt.delete(Motion::WordRight);
        assert_eq!(prompt.text(), " big ");
    }

    #[test]
    fn undoes_typing_runs() {
        let mut prompt = Prompt::new();
        for c in ["a", "b", "c"] {
            prompt.insert(c);
        }
        prompt.insert(" ");
        prompt.insert("d");
        prompt.undo();
        assert_eq!(prompt.text(), "abc ");
        prompt.undo();
        assert_eq!(prompt.text(), "abc");
        prompt.undo();
        assert_eq!(prompt.text(), "");
        prompt.redo();
        assert_eq!(prompt.text(), "abc");
    }

    #[test]
    fn pastes_line_breaks() {
        let mut prompt = Prompt::new();
        prompt.paste("one\r\ntwo\tthree");
        assert_eq!(prompt.text(), "one\ntwo three");
    }

    #[test]
    fn breaks_escaped_line() {
        let mut prompt = prompt_with("one\\");
        assert!(prompt.get_is_caret_after_backslash());
        prompt.break_escaped_line();
        prompt.insert("two");
        assert_eq!(prompt.text(), "one\ntwo");
        prompt.undo();
        prompt.undo();
        assert_eq!(prompt.text(), "one\\");
    }

    #[test]
    fn moves_between_lines() {
        let mut prompt = prompt_with("hello");
        prompt.break_line();
        prompt.insert("ab");
        prompt.move_caret(Motion::LineUp, false);
        assert_eq!(prompt.caret(), 2);
        prompt.move_caret(Motion::End, false);
        prompt.move_caret(Motion::LineUp, false);
        prompt.move_caret(Motion::LineUp, false);
        assert_eq!(prompt.caret(), 0);
        prompt.move_caret(Motion::WordRight, false);
        prompt.move_caret(Motion::LineDown, false);
        assert_eq!(prompt.caret(), 8);
        prompt.move_caret(Motion::LineDown, false);
        assert_eq!(prompt.caret(), 8);
    }
}
