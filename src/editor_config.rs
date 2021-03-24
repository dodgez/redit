#![allow(unused)]

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

#[derive(Default)]
pub struct EditorConfig {
    cx: u16,
    cy: u16,
    left_gutter_size: u16,
    row_offset: usize,
    rows: Vec<String>,
    screen_cols: usize,
    screen_rows: usize,
}

impl EditorConfig {
    pub fn new(rows: usize, cols: usize) -> Self {
        EditorConfig {
            screen_rows: rows,
            screen_cols: cols,
            left_gutter_size: Self::calculate_gutter(0, rows),
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

        Ok(())
    }

    pub fn draw<W: Write>(&self, stdout: &mut W) -> std::io::Result<()> {
        for y in self.row_offset..self.row_offset + self.screen_rows {
            let gutter_size = std::primitive::f32::log10(if y == 0 { 1 } else { y } as f32) as u16;
            // left_gutter - 1 because of pipe char
            stdout.write_all(
                format!(
                    "{}{}|",
                    y,
                    " ".repeat((self.left_gutter_size - gutter_size - 1) as usize)
                )
                .as_bytes(),
            );
            if y >= self.rows.len() {
                if self.rows.is_empty() && y == self.row_offset {
                    stdout.write_all(b"Rudit -- version 0.1.0")?;
                }
            } else {
                let row = self.rows.get(y).unwrap();
                let mut len = row.len();
                if len > self.screen_cols {
                    len = self.screen_cols;
                }
                stdout.write_all(row.trim_end().as_bytes())?;
            }

            stdout.write_all(b"\x1b[K")?;
            if y < self.row_offset + self.screen_rows - 1 {
                stdout.write_all(b"\r\n");
            }
        }

        Ok(())
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.screen_cols = width;
        self.screen_rows = height;
        self.left_gutter_size = Self::calculate_gutter(self.row_offset, self.screen_rows);
    }

    pub fn move_cursor(&mut self, dx: i16, dy: i16) -> (u16, u16) {
        let mut cx = self.cx as i16 + dx;
        let mut cy = self.cy as i16 + dy;

        if cx >= self.screen_cols as i16 {
            cx = self.screen_cols as i16 - 1;
        }
        if cx <= self.left_gutter_size as i16 {
            cx = self.left_gutter_size as i16 + 1;
        }

        if cy >= self.screen_rows as i16 {
            cy = self.screen_rows as i16 - 1;
        }
        if cy < 0 {
            cy = 0;
        }

        self.cx = cx as u16;
        self.cy = cy as u16;
        self.get_cursor()
    }

    pub fn get_cursor(&self) -> (u16, u16) {
        (self.cx, self.cy)
    }

    pub fn handle_char(&mut self, c: char) {
        let row_index = self.cy as usize + self.row_offset;
        let mut col_index = (self.cx - self.left_gutter_size) as usize;

        let mut new_line = self
            .rows
            .get(self.cy as usize + self.row_offset)
            .unwrap_or(&"".to_string())
            .clone();
        if col_index >= new_line.len() {
            new_line = new_line.clone() + &" ".repeat(col_index - new_line.len());
            col_index = new_line.len() - 1;
        }
        new_line.insert(col_index, c);

        if self.rows.len() <= row_index {
            self.rows.resize(row_index + 1, "".to_string());
        }
        self.rows[row_index] = new_line;
        self.move_cursor(1, 0);
    }

    fn calculate_gutter(row_offset: usize, screen_rows: usize) -> u16 {
        1 + std::primitive::f32::log10((row_offset + screen_rows) as f32) as u16
    }
}
