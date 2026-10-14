pub struct Prompt {
    text: String,
    pub visibility: f64,
    pub caret_phase: f64,
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            visibility: 0.0,
            caret_phase: 0.0,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn get_is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn insert(&mut self, text: &str) {
        let printable: String = text.chars().filter(|c| !c.is_control()).collect();
        self.text.push_str(&printable);
        self.caret_phase = 0.0;
    }

    pub fn backspace(&mut self) {
        self.text.pop();
        self.caret_phase = 0.0;
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn take(&mut self) -> String {
        std::mem::take(&mut self.text)
    }
}
