#![allow(unused)]

use std::cmp::{max, min};
use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use chrono::Local;
use crossterm::{execute, style::Color, style::SetBackgroundColor, style::SetForegroundColor};
use syntect::{
    easy::HighlightLines,
    highlighting::{Color as SynColor, FontStyle, Style, StyleModifier, Theme},
    parsing::SyntaxSet,
    util::{as_24_bit_terminal_escaped, modify_range},
};

mod editor_prompt;
mod line;
mod render_config;

use editor_prompt::{EditorPrompt, EditorPromptPurpose};
use line::Line;
use render_config::RenderConfig;

pub enum Movement {
    BegFile,
    EndFile,
    Home,
    End,
    PageUp,
    PageDown,
    Absolute(usize, usize),
    AbsoluteScreen(usize, usize),
    Relative(isize, isize),
}

#[derive(Default)]
pub struct Editor {
    bottom_gutter_size: usize,
    col_offset: usize,
    confirm_dirty: bool,
    cx: usize,
    cy: usize,
    dirty: bool,
    file_path: Option<PathBuf>,
    highlighting: bool,
    hx: usize,
    hy: usize,
    left_gutter_size: usize,
    message: Option<String>,
    prompt: EditorPrompt,
    render_opts: RenderConfig,
    row_offset: usize,
    rows: Vec<Line>,
    rx: usize,
    screen_cols: usize,
    screen_rows: usize,
    syntaxes: SyntaxSet,
}

// Essentially just replaces tabs with 4 spaces
fn convert_cx_to_rx(line: &Line, cx: usize, render_opts: &RenderConfig) -> usize {
    if cx >= line.get_raw().len() {
        line.render(render_opts).len();
    }
    let raw = line.get_raw().split_at(cx).0;
    raw.matches('\t').count() * 3 + cx
}

fn set_stdout_color<W: Write>(
    stdout: &mut W,
    background: Color,
    foreground: Color,
) -> crossterm::Result<()> {
    execute!(
        stdout,
        SetBackgroundColor(background),
        SetForegroundColor(foreground)
    )
}

impl Editor {
    pub fn new(rows: usize, cols: usize, syntaxes: SyntaxSet) -> Self {
        let mut e = Editor {
            rows: vec![Line::new("Redit version 0.1.0".to_string())],
            syntaxes,
            ..Editor::default()
        };
        e.resize(cols, rows);
        e
    }

    pub fn open_file(&mut self, file_name: &dyn AsRef<Path>) -> std::io::Result<()> {
        let file = File::open(file_name)?;
        let mut reader = BufReader::new(file);
        self.rows = vec![];

        loop {
            let mut temp = String::new();
            let n = reader.read_line(&mut temp)?;
            self.rows.push(Line::new(temp));
            if n == 0 {
                break;
            }
        }

        self.update_left_gutter();
        let mut file_name = file_name.as_ref().to_path_buf();
        if let Ok(path) = file_name.canonicalize() {
            file_name = path;
        }
        self.file_path = Some(file_name);
        self.set_message(&"File opened.");
        self.dirty = false;
        self.confirm_dirty = false;

        Ok(())
    }

    pub fn open(&mut self) {
        if !self.dirty || self.confirm_dirty {
            self.prompt = EditorPrompt::new("File to open".to_string(), EditorPromptPurpose::Open);
        } else {
            self.confirm_dirty = true;
            self.set_message(&"Press Ctrl-o again to open a file");
        }
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(file_path) = &self.file_path {
            let file = std::fs::OpenOptions::new()
                .truncate(true)
                .write(true)
                .create(true)
                .open(file_path)?;
            let mut br = std::io::BufWriter::new(file);

            let contents = self
                .rows
                .iter()
                .map(|l| l.get_raw())
                .collect::<Vec<&str>>()
                .join("");
            br.write_all(contents.as_bytes())?;
            self.set_message(&"File saved.");
            self.dirty = false;
            self.confirm_dirty = false;
        } else {
            self.prompt = EditorPrompt::new("New file name".to_string(), EditorPromptPurpose::Save);
        }

        Ok(())
    }

    pub fn try_quit(&mut self) -> bool {
        if !self.dirty || self.confirm_dirty {
            true
        } else {
            self.confirm_dirty = true;
            self.set_message(&"Press Ctrl-q again to quit");
            false
        }
    }

    pub fn try_reload(&mut self) -> std::io::Result<()> {
        if !self.dirty || self.confirm_dirty {
            if let Some(file) = self.file_path.clone() {
                self.open_file(&file)
            } else {
                self.set_message(&"No file to reload");
                Ok(())
            }
        } else {
            self.confirm_dirty = true;
            self.set_message(&"Press Ctrl-r again to reload from disk");
            Ok(())
        }
    }

    pub fn draw<W: Write>(&self, stdout: &mut W, theme: &Theme) -> crossterm::Result<()> {
        let bg = theme.settings.background.unwrap_or(SynColor::BLACK);
        let bg_color = Color::Rgb {
            r: bg.r,
            g: bg.g,
            b: bg.b,
        };
        let fg = theme.settings.foreground.unwrap_or(SynColor::WHITE);
        let fg_color = Color::Rgb {
            r: fg.r,
            g: fg.g,
            b: fg.b,
        };
        let default_style = Style {
            background: bg,
            foreground: fg,
            font_style: FontStyle::empty(),
        };
        let highlight_style = StyleModifier {
            background: Some(fg),
            foreground: Some(bg),
            font_style: None,
        };

        let syntax = self
            .file_path
            .as_ref()
            .and_then(|f| f.extension())
            .and_then(|e| self.syntaxes.find_syntax_by_extension(&e.to_string_lossy()));

        for y in self.row_offset..min(self.rows.len(), self.row_offset + self.screen_rows + 1) {
            let gutter_size = (if y < 2 { 2 } else { 2 + y } as f32).log10().ceil() as usize; // 2+ so line numbers start at 1
            stdout.write_all(
                format!(
                    "{}{}|",
                    y + 1, // Line numbering starts at 1
                    " ".repeat(self.left_gutter_size - gutter_size - 1) // Get difference not including separator
                )
                .as_bytes(),
            );
            let row = self.rows.get(y).unwrap().render(&self.render_opts); // Safe because of array bounds
            let col_split = if (self.col_offset >= row.len()) {
                ""
            } else {
                row.split_at(self.col_offset).1
            };
            let mut len = col_split.len();
            if len > self.screen_cols {
                len = self.screen_cols;
            }

            let mut write_escaped = |s: &[(Style, &str)]| {
                stdout.write_all(as_24_bit_terminal_escaped(&s, true).as_bytes())
            };

            let mut h = syntax.map(|s| HighlightLines::new(s, theme));
            let raw_row = col_split.split_at(len).0;
            let row = if let Some(mut h) = h {
                h.highlight(raw_row, &self.syntaxes)
            } else {
                vec![(default_style, raw_row)]
            };
            if self.highlighting && y >= min(self.cy, self.hy) && y <= max(self.cy, self.hy) {
                if self.cy == self.hy {
                    if self.cx < self.hx {
                        write_escaped(&modify_range(&row, self.cx..self.hx, highlight_style))?;
                    } else {
                        write_escaped(&modify_range(&row, self.hx..self.cx, highlight_style))?;
                    }
                } else if y == min(self.cy, self.hy) {
                    if self.cy < self.hy {
                        write_escaped(&modify_range(
                            &row,
                            self.cx..raw_row.len(),
                            highlight_style,
                        ))?;
                    } else {
                        write_escaped(&modify_range(
                            &row,
                            self.hx..raw_row.len(),
                            highlight_style,
                        ))?;
                    }
                } else if y == max(self.cy, self.hy) {
                    if self.cy < self.hy {
                        write_escaped(&modify_range(&row, 0..self.hx, highlight_style))?;
                    } else {
                        write_escaped(&modify_range(&row, 0..self.cx, highlight_style))?;
                    }
                } else {
                    write_escaped(&modify_range(&row, 0..raw_row.len(), highlight_style))?;
                }
            } else {
                stdout.write_all(as_24_bit_terminal_escaped(&row, true).as_bytes())?;
            }
            execute!(
                stdout,
                SetBackgroundColor(bg_color),
                SetForegroundColor(fg_color)
            )?;

            stdout.write_all(b"\x1b[K")?; // Clear line
            stdout.write_all(b"\r\n")?;
        }

        // Force status bar to be at the bottom
        for y in self.rows.len()..self.row_offset + self.screen_rows + 1 {
            stdout.write_all(b"\x1b[K")?; // Clear line
            stdout.write_all(b"\r\n")?;
        }

        // File status bar
        stdout.write_all(b"\x1b[K")?;
        let mut file_s = self
            .file_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[No Name]".to_string());
        let status_start = if self.dirty {
            "File (modified): "
        } else {
            "File: "
        };
        let max_length = self.screen_cols + self.left_gutter_size - status_start.len() - 21; // 21 for line col status up to 4 chars each
        if file_s.len() > max_length {
            file_s = file_s.split_at(file_s.len() - max_length).1.to_string();
        }
        stdout.write_all(
            format!(
                "{}{} L{}:C{}",
                status_start,
                file_s,
                self.cy + 1,
                self.rx + 1
            )
            .as_bytes(),
        )?;
        stdout.write_all(b"\r\n")?;

        // Message status bar
        stdout.write_all(b"\x1b[K")?;
        match &self.message {
            Some(message) => {
                stdout.write_all(format!("Message at {} ", message).as_bytes())?;
            }
            None => {
                stdout.write_all(b"[No Messages] ")?;
            }
        }

        if self.prompt.is_active() {
            self.prompt.draw(stdout);
        }

        Ok(())
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        let bottom_gutter_size = Self::calculate_bottom_gutter();
        self.screen_rows = height - bottom_gutter_size;
        self.left_gutter_size =
            Self::calculate_left_gutter(self.row_offset, self.screen_rows, self.rows.len());
        self.screen_cols = width - self.left_gutter_size;
        self.scroll();
    }

    pub fn get_rel_cursor(&self) -> (u16, u16) {
        if !self.prompt.is_active() {
            (
                (self.rx - self.col_offset + self.left_gutter_size) as u16,
                (self.cy - self.row_offset) as u16,
            )
        } else {
            let message_length = if let Some(message) = &self.message {
                format!("Message at {} ", message).len()
            } else {
                "[No Messages] ".len()
            };
            (
                message_length as u16 + self.prompt.get_length(),
                self.screen_rows as u16 + 2, // +2 because prompt is on second line
            )
        }
    }

    pub fn move_cursor(&mut self, pos: Movement, with_highlight: bool) {
        if self.prompt.is_active() {
            return;
        }
        if with_highlight && !self.highlighting {
            self.hx = self.cx;
            self.hy = self.cy;
            self.highlighting = true;
        } else if !with_highlight && self.highlighting {
            self.highlighting = false;
        }
        match pos {
            Movement::BegFile => {
                self.col_offset = 0;
                self.cx = 0;
                self.cy = 0;
                self.row_offset = 0;
            }
            Movement::Home => {
                self.col_offset = 0;
                self.cx = 0;
            }
            Movement::End => {
                if let Some(line) = self.rows.get(self.cy).map(|l| l.get_clean_raw()) {
                    self.cx = if line.is_empty() { 0 } else { line.len() };
                }
            }
            Movement::PageUp => {
                let rel = self.cy - self.row_offset;
                self.cy = self.row_offset;
                let rollback = self.row_offset >= self.screen_rows;
                self.move_cursor(
                    Movement::Relative(0, 0 - (self.screen_rows as isize)),
                    with_highlight,
                );
                if rollback {
                    self.move_cursor(Movement::Relative(0, rel as isize), with_highlight);
                }
            }
            Movement::PageDown => {
                let rel = self.cy - self.row_offset;
                self.cy = self.row_offset + self.screen_rows;
                let rollback = self.cy < self.rows.len() - 1; // -1 because row_offset can never get bigger
                self.move_cursor(
                    Movement::Relative(0, self.screen_rows as isize),
                    with_highlight,
                );
                if rollback {
                    self.move_cursor(
                        Movement::Relative(0, 0 - (self.screen_rows - rel) as isize),
                        with_highlight,
                    );
                }
            }
            // Up
            Movement::Relative(0, dy) if dy < 0 => {
                let new_cy = self.cy as isize + dy;
                let new_cy = if new_cy < 0 { 0 } else { new_cy };
                if new_cy >= 0 {
                    if let Some(line) = self.rows.get(new_cy as usize).map(|l| l.get_clean_raw()) {
                        self.cy = new_cy as usize;
                        if self.cx > line.len() {
                            self.move_cursor(Movement::End, with_highlight);
                        }
                    }
                }
            }
            // Down
            Movement::Relative(0, dy) if dy > 0 => {
                let new_cy = self.cy + dy as usize;
                let new_cy = if new_cy >= self.rows.len() {
                    self.rows.len() - 1
                } else {
                    new_cy
                };
                if let Some(line) = self.rows.get(new_cy).map(|l| l.get_clean_raw()) {
                    self.cy = new_cy;
                    if self.cx > line.len() {
                        self.move_cursor(Movement::End, with_highlight);
                    }
                }
            }
            // Left
            Movement::Relative(dx, 0) if dx < 0 => {
                if self.cx as isize + dx < 0 {
                    if self.cy > 0 {
                        self.move_cursor(Movement::Relative(0, -1), with_highlight);
                        self.move_cursor(Movement::End, with_highlight);
                    }
                } else {
                    self.cx = (self.cx as isize + dx) as usize;
                }
            }
            // Right
            Movement::Relative(dx, 0) if dx > 0 => {
                if let Some(line) = self.rows.get(self.cy).map(|l| l.get_clean_raw()) {
                    if self.cx + dx as usize > line.len() {
                        if self.cy < self.rows.len() - 1 {
                            self.move_cursor(Movement::Relative(0, 1), with_highlight);
                            self.move_cursor(Movement::Home, with_highlight);
                        }
                    } else {
                        self.cx += dx as usize;
                    }
                }
            }
            Movement::Absolute(x, y) => {
                self.cy = min(y, self.rows.len() - 1); // There should be at least one row
                self.cx = min(x, self.rows.get(self.cy).unwrap().get_raw().len());
            }
            Movement::AbsoluteScreen(x, y) => {
                self.cy = min(self.row_offset + y, self.rows.len() - 1);
                let row_len = self.rows.get(self.cy).unwrap().get_raw().len();
                self.cx = min(
                    if self.left_gutter_size > x {
                        0
                    } else {
                        x - self.left_gutter_size
                    },
                    if row_len > 0 { row_len - 1 } else { 0 },
                );
            }
            _ => {}
        }

        self.scroll();
        self.update_left_gutter();
    }

    pub fn write_char(&mut self, c: char) {
        if self.prompt.is_active() {
            self.prompt.add_char(c);
        } else if let Some(line) = self.rows.get(self.cy) {
            let mut s = line.get_raw().to_string();
            s.insert(self.cx, c);

            self.replace_row(self.cy, s);
            self.move_cursor(Movement::Relative(1, 0), false);
            self.make_dirty();
        }
    }

    pub fn delete_char(&mut self) {
        if self.prompt.is_active() {
            return;
        }
        if self.highlighting {
            if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
                self.remove_text_region(self.cx, self.cy, self.hx, self.hy);
            } else {
                self.remove_text_region(self.hx, self.hy, self.cx, self.cy);
                self.move_cursor(Movement::Absolute(self.hx, self.hy), false);
            }
            self.highlighting = false;
            self.make_dirty();
        } else if let Some(line) = self.rows.get(self.cy) {
            let mut s = line.get_raw().to_string();
            if self.cx < line.get_clean_raw().len() {
                s.remove(self.cx);
                self.replace_row(self.cy, s);
                self.make_dirty();
            } else if let Some(other_line) = self.rows.get(self.cy + 1) {
                s = line.get_clean_raw() + other_line.get_raw();
                self.replace_row(self.cy, s);
                self.remove_row(self.cy + 1);
                self.make_dirty();
            }
        }
    }

    pub fn backspace_char(&mut self) {
        if self.prompt.is_active() {
            self.prompt.remove_char();
        } else if self.cx > 0 || self.cy > 0 {
            self.move_cursor(Movement::Relative(-1, 0), false);
            self.delete_char();
        }
    }

    pub fn do_return(&mut self) {
        if self.prompt.is_active() {
            self.check_prompt();
        } else {
            if self.highlighting {
                if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
                    self.remove_text_region(self.cx, self.cy, self.hx, self.hy);
                } else {
                    self.remove_text_region(self.hx, self.hy, self.cx, self.cy);
                    self.move_cursor(Movement::Absolute(self.hx, self.hy), false);
                }
                self.highlighting = false;
                self.make_dirty();
            }
            if let Some(line) = self.rows.get(self.cy) {
                let line_ending = line.get_raw().split_at(line.get_clean_raw().len()).1;
                let raw = line.get_raw().to_string();
                let parts = raw.split_at(self.cx);
                let split_row = parts.0.to_string() + line_ending;
                self.replace_row(self.cy, split_row);
                self.insert_row(self.cy + 1, parts.1.to_string());
                self.move_cursor(Movement::Relative(0, 1), false);
                self.move_cursor(Movement::Home, false);
                self.make_dirty();
            }
        }
    }

    fn insert_row(&mut self, row_index: usize, contents: String) {
        self.rows.insert(row_index, Line::new(contents));
    }

    fn replace_row(&mut self, row: usize, contents: String) {
        self.rows[row] = Line::new(contents);
    }

    fn remove_row(&mut self, row: usize) {
        self.rows.remove(row);
        self.update_left_gutter();
    }

    fn get_text_region(
        &self,
        start_x: usize,
        start_y: usize,
        end_x: usize,
        end_y: usize,
    ) -> Vec<Line> {
        // Ensure the markers are inside the file
        let start_y = min(start_y, self.rows.len());
        let start_x = min(start_x, self.rows.get(start_y).unwrap().get_raw().len());
        let end_y = min(end_y, self.rows.len());
        let end_x = min(end_x, self.rows.get(end_y).unwrap().get_raw().len());

        let mut text = vec![];
        if start_y != end_y {
            text.push(Line::new(
                self.rows
                    .get(start_y)
                    .unwrap()
                    .get_raw()
                    .split_at(start_x)
                    .1
                    .to_string(),
            ));
            for i in start_y + 1..end_y {
                text.push(self.rows.get(i).unwrap().to_owned());
            }
            text.push(Line::new(
                self.rows
                    .get(end_y)
                    .unwrap()
                    .get_raw()
                    .split_at(end_x)
                    .0
                    .to_string(),
            ));
        } else {
            text.push(Line::new(
                self.rows
                    .get(start_y)
                    .unwrap()
                    .get_raw()
                    .get(start_x..end_x)
                    .unwrap()
                    .to_string(),
            ));
        }
        text
    }

    fn remove_text_region(&mut self, start_x: usize, start_y: usize, end_x: usize, end_y: usize) {
        // Ensure the markers are inside the file
        let start_y = min(start_y, self.rows.len());
        let start_x = min(start_x, self.rows.get(start_y).unwrap().get_raw().len());
        let end_y = min(end_y, self.rows.len());
        let end_x = min(end_x, self.rows.get(end_y).unwrap().get_raw().len());

        if start_y != end_y {
            self.replace_row(
                start_y,
                self.rows
                    .get(start_y)
                    .unwrap()
                    .get_raw()
                    .split_at(start_x)
                    .0
                    .to_string()
                    + self.rows.get(end_y).unwrap().get_raw().split_at(end_x).1,
            );
            for _ in start_y + 1..end_y + 1 {
                self.remove_row(start_y + 1);
            }
        } else {
            let mut row = self.rows.get(start_y).unwrap().get_raw().to_string();
            row.replace_range(start_x..end_x, "");
            self.replace_row(start_y, row);
        }
    }

    pub fn cut(&mut self) -> Vec<Line> {
        let clipboard = self.copy();
        if self.highlighting {
            if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
                self.remove_text_region(self.cx, self.cy, self.hx, self.hy);
            } else {
                self.remove_text_region(self.hx, self.hy, self.cx, self.cy);
                self.move_cursor(Movement::Absolute(self.hx, self.hy), false);
            }
            self.highlighting = false;
            self.make_dirty();
        }
        clipboard
    }
    pub fn copy(&mut self) -> Vec<Line> {
        let mut clipboard = vec![];
        if self.highlighting {
            if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
                clipboard = self.get_text_region(self.cx, self.cy, self.hx, self.hy);
            } else {
                clipboard = self.get_text_region(self.hx, self.hy, self.cx, self.cy);
            }
        }
        clipboard
    }
    pub fn paste(&mut self, clipboard: &Option<Vec<Line>>) {
        if let Some(clipboard) = clipboard {
            if self.highlighting {
                if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
                    self.remove_text_region(self.cx, self.cy, self.hx, self.hy);
                } else {
                    self.remove_text_region(self.hx, self.hy, self.cx, self.cy);
                    self.move_cursor(Movement::Absolute(self.hx, self.hy), false);
                }
            }
            if let Some(other_row) = self.rows.get(self.cy) {
                let other_row = other_row.get_raw().split_at(self.cx);
                let s = other_row.0.to_string()
                    + &clipboard.get(0).unwrap().get_clean_raw()
                    + other_row.1;
                self.replace_row(self.cy, s);
                self.move_cursor(
                    Movement::Relative(clipboard.get(0).unwrap().get_clean_raw().len() as isize, 0),
                    false,
                );
                for i in 1..clipboard.len() {
                    self.insert_row(self.cy + 1, clipboard.get(i).unwrap().get_raw().to_string());
                    self.move_cursor(Movement::Relative(0, 1), false);
                }
            }
            self.make_dirty();
        }
    }

    fn check_prompt(&mut self) {
        let answer = self.prompt.get_answer();
        match self.prompt.purpose {
            EditorPromptPurpose::Save => {
                if let Some(answer) = answer {
                    self.file_path = Some(Path::new(answer).to_path_buf());
                    if let Err(e) = self.save() {
                        self.set_message(&"Error writing to file");
                    }
                }
            }
            EditorPromptPurpose::Open => {
                if let Some(answer) = answer {
                    let path = Path::new(answer).to_path_buf();
                    if let Err(e) = self.open_file(&path) {
                        self.set_message(&"Error opening file");
                    }
                }
            }
            _ => {}
        }
        self.cancel_prompt();
    }

    pub fn cancel_prompt(&mut self) {
        self.confirm_dirty = false;
        self.prompt.exit();
        self.message = None;
    }

    fn make_dirty(&mut self) {
        // Turn off the confirm quit message if applicable
        if self.confirm_dirty {
            self.message = None;
        }
        self.dirty = true;
        self.confirm_dirty = false;
    }

    fn set_message(&mut self, message: &dyn AsRef<str>) {
        self.message = Some(format!(
            "{}: {}",
            Local::now().format("%I:%M:%S %P"),
            message.as_ref()
        ));
    }

    fn update_left_gutter(&mut self) {
        let new_gutter =
            Self::calculate_left_gutter(self.row_offset, self.screen_rows, self.rows.len());
        self.screen_cols = (self.screen_cols + self.left_gutter_size) - new_gutter;
        self.left_gutter_size = new_gutter;
    }

    fn scroll(&mut self) {
        if self.rows.get(self.cy).is_none() {
            return;
        }
        self.rx = convert_cx_to_rx(self.rows.get(self.cy).unwrap(), self.cx, &self.render_opts);

        if self.rx < self.col_offset {
            self.col_offset = self.rx;
        }
        if self.rx - self.col_offset > self.screen_cols {
            self.col_offset = self.rx - self.screen_cols;
        }
        if self.cy < self.row_offset {
            self.row_offset = self.cy;
        }
        if self.cy - self.row_offset > self.screen_rows {
            self.row_offset = self.cy - self.screen_rows;
        }
    }

    fn calculate_left_gutter(row_offset: usize, screen_rows: usize, rows: usize) -> usize {
        // 1 to include pipe char and 2.0+ so that 10^n -> n+1 and line numbers start at 1
        1 + if screen_rows < rows - row_offset {
            (2.0 + (row_offset + screen_rows) as f32).log10().ceil()
        } else {
            (1.0 + rows as f32).log10().ceil()
        } as usize
    }

    fn calculate_bottom_gutter() -> usize {
        2 // file status and prompt
    }
}
