#![allow(unused)]

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

#[derive(Default)]
pub struct EditorConfig {
    cx: usize,
    cy: usize,
    screen_rows: usize,
    screen_cols: usize,
    pub num_rows: usize,
    pub row: String,
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
        self.row = String::new();
        let _ = reader.read_line(&mut self.row)?;
        self.num_rows = 1;

        Ok(())
    }

    pub fn draw<W: Write>(&self, stdout: &mut W) -> std::io::Result<()> {
        for y in 0..self.screen_rows {
            if y >= self.num_rows {
                if self.num_rows == 0 && y == 0 {
                    stdout.write_all(b"Rudit -- version 0.1.0")?;
                } else {
                    stdout.write_all(b"~")?;
                }
                stdout.write_all(b"\r\n")?;
            } else {
                let mut len = self.row.len();
                if len > self.screen_cols {
                    len = self.screen_cols;
                }
                stdout.write_all(self.row.as_bytes())?;
            }

            stdout.write_all(b"\x1b[K")?;
        }

        Ok(())
    }
}
