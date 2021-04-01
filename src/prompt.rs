use tui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, Borders, Widget},
};

#[derive(Clone)]
pub struct Prompt {
    cx: usize,
    response: Option<String>,
}

impl Widget for Prompt {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default().borders(Borders::TOP);
        let inner_area = block.inner(area);
        block.render(area, buf);
        buf.set_stringn(
            inner_area.x,
            inner_area.y,
            ">".to_string() + self.response.as_ref().unwrap_or(&"".to_string()),
            inner_area.width as usize,
            Style::default(),
        );
    }
}

impl Prompt {
    pub fn new(message: Option<String>) -> Self {
        Prompt {
            cx: message.clone().map(|s| s.len()).unwrap_or(0),
            response: message,
        }
    }

    pub fn delete_char(&mut self) {
        if let Some(res) = &self.response {
            if self.cx < res.len() {
                let mut res = res.to_string();
                res.remove(self.cx).to_string();
                self.response = Some(res);
            }
        }
    }

    pub fn add_char(&mut self, c: char) {
        let mut res = self.response.as_ref().unwrap_or(&"".to_string()).clone();
        res.push(c);
        self.response = Some(res);
        self.cx += 1;
    }

    pub fn backspace(&mut self) {
        if self.cx > 0 {
            self.cx -= 1;
            self.delete_char();
        }
    }

    pub fn move_cursor(&mut self, dx: isize) {
        if dx >= 0 {
            self.cx = std::cmp::min(
                self.response.as_ref().unwrap_or(&"".to_string()).len(),
                self.cx + dx as usize,
            );
        } else if self.cx as isize + dx >= 0 {
            self.cx = (self.cx as isize + dx) as usize;
        } else {
            self.cx = 0;
        }
    }

    pub fn get_cursor(&self) -> (u16, u16) {
        (self.cx as u16 + 1, 1) // +1 for > character and 1 for top border
    }

    pub fn take_answer(&mut self) -> Option<String> {
        self.cx = 0;
        self.response.take()
    }
}
