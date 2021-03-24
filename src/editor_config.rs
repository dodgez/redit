#![allow(unused)]

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

#[derive(Default)]
pub struct EditorConfig {
    cx: usize,
    cy: usize,
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
            if y >= self.rows.len() {
                if self.rows.is_empty() && y == self.row_offset {
                    stdout.write_all(b"Rudit -- version 0.1.0")?;
                } else {
                    stdout.write_all(b"~")?;
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

    pub fn scroll(&mut self) {
        if (self.cy < self.row_offset) {
            self.row_offset = self.cy;
        }
        if (self.cy >= self.row_offset + self.screen_rows) {
            self.row_offset = self.cy - self.screen_rows + 1;
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.screen_cols = width;
        self.screen_rows = height;
    }
}
