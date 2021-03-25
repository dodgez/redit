#![allow(unused)]

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

pub enum Movement {
    BegFile,
    EndFile,
    Home,
    End,
    Absolute(usize, usize),
    Relative(isize, isize),
}

#[derive(Default)]
pub struct EditorConfig {
    col_offset: usize,
    cx: usize,
    cy: usize,
    left_gutter_size: usize,
    row_offset: usize,
    rows: Vec<String>,
    screen_cols: usize,
    screen_rows: usize,
}

impl EditorConfig {
    pub fn new(rows: usize, cols: usize) -> Self {
        let left_gutter_size = Self::calculate_gutter(0, rows, 1);
        EditorConfig {
            left_gutter_size,
            rows: vec!["Rudit version 0.1.0 - New file".to_string()],
            screen_rows: rows,
            screen_cols: cols - left_gutter_size,
            ..EditorConfig::default()
        }
    }

    pub fn open(&mut self, file: &dyn AsRef<Path>) -> std::io::Result<()> {
        let file = File::open(file)?;
        let mut reader = BufReader::new(file);
        self.rows = vec![];

        loop {
            let mut temp = String::new();
            let n = reader.read_line(&mut temp)?;
            self.rows.push(temp);
            if n == 0 {
                break;
            }
        }

        self.update_gutter();

        Ok(())
    }

    pub fn draw<W: Write>(&self, stdout: &mut W) -> std::io::Result<()> {
        for y in
            self.row_offset..std::cmp::min(self.rows.len(), self.row_offset + self.screen_rows + 1)
        {
            let gutter_size = (if y < 2 { 2 } else { 1 + y } as f32).log10().ceil() as usize;
            stdout.write_all(
                format!(
                    "{}{}|",
                    y,
                    " ".repeat(self.left_gutter_size - gutter_size - 1) // Get difference not including separator
                )
                .as_bytes(),
            );
            let row = self.rows.get(y).unwrap().trim_end(); // Safe because of array bounds
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

            stdout.write_all(b"\x1b[K")?;
            if y < self.row_offset + self.screen_rows { // Don't append new \r\n when at the very bottom of the window
                stdout.write_all(b"\r\n");
            }
        }

        Ok(())
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.screen_rows = height;
        self.left_gutter_size =
            Self::calculate_gutter(self.row_offset, self.screen_rows, self.rows.len());
        self.screen_cols = width - self.left_gutter_size;
        self.scroll();
    }

    pub fn get_rel_cursor(&self) -> (u16, u16) {
        ((self.cx - self.col_offset + self.left_gutter_size) as u16, (self.cy - self.row_offset) as u16)
    }

    pub fn move_cursor(&mut self, pos: Movement) {
        match pos {
            Movement::BegFile => {
                self.cx = 0;
                self.cy = 0;
                self.row_offset = 0;
                self.col_offset = 0;
            }
            Movement::Home => {
                self.cx = 0;
                self.col_offset = 0;
            }
            Movement::End => {
                if let Some(line) = self.rows.get(self.cy) {
                    self.cx = if line.is_empty() {0} else {line.trim_end().len()};
                }
            }
            // Up
            Movement::Relative(0, dy) if dy < 0 => {
                let new_cy = self.cy as isize + dy;
                if new_cy >= 0 {
                    if let Some(line) = self.rows.get(new_cy as usize) {
                        self.cy = new_cy as usize;
                        if self.cx > line.trim_end().len() {
                            self.move_cursor(Movement::End);
                        }
                    }
                }
            }
            // Down
            Movement::Relative(0, dy) if dy > 0 => {
                if let Some(line) = self.rows.get(self.cy + dy as usize) {
                    self.cy += dy as usize;
                    if self.cx > line.trim_end().len() {
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
                if let Some(line) = self.rows.get(self.cy) {
                    if self.cx + dx as usize > line.trim_end().len() {
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
        self.update_gutter();
    }

    pub fn write_char(&mut self, c: char) {
        if let Some(mut line) = self.rows.get(self.cy).map(|l| l.to_string()) {
            line.insert(self.cx, c);

            self.rows[self.cy] = line;
            self.move_cursor(Movement::Relative(1, 0));
        }
    }

    fn update_gutter(&mut self) {
        let new_gutter = Self::calculate_gutter(self.row_offset, self.screen_rows, self.rows.len());
        self.screen_cols = (self.screen_cols + self.left_gutter_size) - new_gutter;
        self.left_gutter_size = new_gutter;
    }

    fn scroll(&mut self) {
        if self.cx < self.col_offset {
            self.col_offset = self.cx;
        }
        if self.cx - self.col_offset > self.screen_cols {
            self.col_offset = self.cx - self.screen_cols;
        }
        if self.cy < self.row_offset {
            self.row_offset = self.cy;
        }
        if self.cy - self.row_offset > self.screen_rows {
            self.row_offset = self.cy - self.screen_rows;
        }
    }

    fn calculate_gutter(row_offset: usize, screen_rows: usize, rows: usize) -> usize {
        // 1 to include pipe char and 1.0+ so that 10^n -> n+1
        1 + if screen_rows < rows - row_offset {
            (1.0 + (row_offset + screen_rows) as f32).log10().ceil()
        } else {
            (1.0 + rows as f32).log10().ceil()
        } as usize
    }
}
