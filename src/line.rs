use crate::render_config::RenderConfig;

#[derive(Clone)] // Needed in buffer
pub struct Line {
    raw: String,
}

impl Line {
    pub fn new(raw: String) -> Self {
        Line { raw }
    }

    pub fn get_raw(&self) -> &str {
        &self.raw
    }

    pub fn get_clean_raw(&self) -> String {
        self.raw.replace("\r", "").replace("\n", "")
    }

    pub fn render(&self, options: &RenderConfig) -> String {
        let rendered = self.raw.trim_end().to_string();

        rendered.replace('\t', &" ".repeat(options.tab_size))
    }
}
