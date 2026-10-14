use arboard::Clipboard as SystemClipboard;

pub struct Clipboard {
    system: Option<SystemClipboard>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self {
            system: SystemClipboard::new().ok(),
        }
    }

    pub fn read(&mut self) -> Option<String> {
        self.system.as_mut()?.get_text().ok()
    }

    pub fn write(&mut self, text: &str) {
        let Some(system) = self.system.as_mut() else {
            return;
        };
        let _ = system.set_text(text);
    }
}
