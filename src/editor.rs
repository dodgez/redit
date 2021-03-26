#![allow(unused)]

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub enum Movement {
    BegFile,
    EndFile,
    Home,
    End,
    PageUp,
    PageDown,
    Absolute(usize, usize),
    Relative(isize, isize),
}

pub struct RenderConfig {
    tab_size: usize,
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig { tab_size: 4 }
    }
}

struct Line {
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

#[derive(Default)]
pub struct Editor {
    bottom_gutter_size: usize,
    col_offset: usize,
    cx: usize,
    cy: usize,
    dirty: bool,
    file_path: Option<PathBuf>,
    left_gutter_size: usize,
    message: Option<String>,
    render_opts: RenderConfig,
    row_offset: usize,
    rows: Vec<Line>,
    rx: usize,
    screen_cols: usize,
    screen_rows: usize,
}

// Essentially just replaces tabs with 4 spaces
fn convert_cx_to_rx(line: &Line, cx: usize, render_opts: &RenderConfig) -> usize {
    if cx >= line.get_raw().len() {
        line.render(render_opts).len();
    }
    let raw = line.get_raw().split_at(cx).0;
    raw.matches('\t').count() * 3 + cx
}

impl Editor {
    pub fn new(rows: usize, cols: usize) -> Self {
        let bottom_gutter_size = Self::calculate_bottom_gutter();
        let left_gutter_size = Self::calculate_left_gutter(0, rows, 1);
        Editor {
            bottom_gutter_size,
            left_gutter_size,
            rows: vec![Line::new("Rudit version 0.1.0".to_string())],
            screen_rows: rows - bottom_gutter_size,
            screen_cols: cols - left_gutter_size,
            ..Editor::default()
        }
    }

    pub fn open(&mut self, file_name: &dyn AsRef<Path>) -> std::io::Result<()> {
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

        Ok(())
    }

    pub fn save(&mut self) -> std::io::Result<()> {
        if let Some(file_path) = &self.file_path {
            let mut file = std::fs::OpenOptions::new().write(true).open(file_path)?;

            let contents = self
                .rows
                .iter()
                .map(|l| l.get_raw())
                .collect::<Vec<&str>>()
                .join("");
            file.write_all(contents.as_bytes());
            self.set_message(&"File saved.");
            self.dirty = false;
        }

        Ok(())
    }

    pub fn draw<W: Write>(&self, stdout: &mut W) -> std::io::Result<()> {
        for y in
            self.row_offset..std::cmp::min(self.rows.len(), self.row_offset + self.screen_rows + 1)
        {
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
            stdout.write_all(col_split.split_at(len).0.as_bytes())?;

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
            format!("{}{} L{}:C{}", status_start, file_s, self.cy + 1, self.rx).as_bytes(),
        )?;
        stdout.write_all(b"\r\n")?;

        // Message status bar
        stdout.write_all(b"\x1b[K")?;
        match &self.message {
            Some(message) => {
                stdout.write_all(format!("Message at {}", message).as_bytes())?;
            }
            None => {
                stdout.write_all(b"[No Messages]")?;
            }
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
        (
            (self.rx - self.col_offset + self.left_gutter_size) as u16,
            (self.cy - self.row_offset) as u16,
        )
    }

    pub fn move_cursor(&mut self, pos: Movement) {
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
                self.cy = self.row_offset;
                self.move_cursor(Movement::Relative(0, 0 - (self.screen_rows as isize)));
            }
            Movement::PageDown => {
                self.cy = self.row_offset + self.screen_rows;
                if self.cy > self.rows.len() {
                    self.cy = self.rows.len();
                }
                self.move_cursor(Movement::Relative(0, self.screen_rows as isize));
            }
            // Up
            Movement::Relative(0, dy) if dy < 0 => {
                let new_cy = self.cy as isize + dy;
                let new_cy = if new_cy < 0 { 0 } else { new_cy };
                if new_cy >= 0 {
                    if let Some(line) = self.rows.get(new_cy as usize).map(|l| l.get_clean_raw()) {
                        self.cy = new_cy as usize;
                        if self.cx > line.len() {
                            self.move_cursor(Movement::End);
                        }
                    }
                }
            }
            // Down
            Movement::Relative(0, dy) if dy > 0 => {
                let new_cy = self.cy + dy as usize;
                let new_cy = if new_cy > self.rows.len() {
                    self.rows.len() - 1
                } else {
                    new_cy
                };
                if let Some(line) = self.rows.get(new_cy).map(|l| l.get_clean_raw()) {
                    self.cy = new_cy;
                    if self.cx > line.len() {
                        self.move_cursor(Movement::End);
                    }
                }
            }
            // Left
            Movement::Relative(dx, 0) if dx < 0 => {
                if self.cx as isize + dx < 0 {
                    if self.cy > 0 {
                        self.move_cursor(Movement::Relative(0, -1));
                        self.move_cursor(Movement::End);
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
                            self.move_cursor(Movement::Relative(0, 1));
                            self.move_cursor(Movement::Home);
                        }
                    } else {
                        self.cx += dx as usize;
                    }
                }
            }
            _ => {}
        }

        self.scroll();
        self.update_left_gutter();
    }

    pub fn write_char(&mut self, c: char) {
        if let Some(line) = self.rows.get(self.cy) {
            let mut s = line.get_raw().to_string();
            s.insert(self.cx, c);

            self.rows[self.cy] = Line::new(s);
            self.move_cursor(Movement::Relative(1, 0));
            self.dirty = true;
        }
    }

    fn set_message(&mut self, message: &dyn AsRef<str>) {
        self.message = Some(format!(
            "{}: {}",
            chrono::Local::now().format("%I:%M:%S %P"),
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
            (2.0 + rows as f32).log10().ceil()
        } as usize
    }

    fn calculate_bottom_gutter() -> usize {
        2
    }
}
