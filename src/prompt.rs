use std::io::prelude::*;

pub enum PromptPurpose {
    None,
    Open,
    Save,
}

impl Default for PromptPurpose {
    fn default() -> PromptPurpose {
        PromptPurpose::None
    }
}

#[derive(Default)]
pub struct Prompt {
    active: bool,
    answer: Option<String>,
    message: Option<String>,
    pub purpose: PromptPurpose,
}

impl Prompt {
    pub fn new(message: String, purpose: PromptPurpose) -> Prompt {
        Prompt {
            active: true,
            answer: None,
            message: Some(message),
            purpose,
        }
    }

    pub fn get_answer(&self) -> Option<&String> {
        self.answer.as_ref()
    }

    pub fn exit(&mut self) {
        self.active = false;
        self.answer = None;
        self.message = None;
    }

    pub fn add_char(&mut self, c: char) {
        match &mut self.answer {
            None => {
                self.answer = Some(c.to_string());
            }
            Some(s) => {
                s.push(c);
            }
        }
    }

    pub fn remove_char(&mut self) {
        if let Some(mut answer) = self.answer.clone() {
            answer.pop();
            self.answer = Some(answer);
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn draw<W: Write>(&self, stdout: &mut W) -> std::io::Result<()> {
        let answer = self
            .answer
            .as_ref()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "".to_string());
        if let Some(message) = self.message.as_ref() {
            stdout.write_all(format!("{}: {}", message, answer).as_bytes())?;
        } else {
            stdout.write_all(answer.as_bytes())?;
        }
        Ok(())
    }

    pub fn get_length(&self) -> u16 {
        (self.message.as_ref().map(|m| m.len() + 2).unwrap_or(0)
            + self.answer.as_ref().map(|s| s.len()).unwrap_or(0)) as u16
    }
}
