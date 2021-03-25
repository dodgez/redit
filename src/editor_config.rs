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
            left_gutter_size: Self::calculate_gutter(0, rows, 1),
            rows: vec!["Rudit version 0.1.0 - New file".to_string()],
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

        self.left_gutter_size =
            Self::calculate_gutter(self.row_offset, self.screen_rows, self.rows.len());

        Ok(())
    }

    pub fn draw<W: Write>(&self, stdout: &mut W) -> std::io::Result<()> {
        for y in
            self.row_offset..std::cmp::min(self.rows.len(), self.row_offset + self.screen_rows + 1)
        {
            let gutter_size = std::primitive::f32::log10(if y == 0 { 1 } else { y } as f32) as u16;
            // left_gutter - 1 because of pipe char
            // println!("{}, {}", self.left_gutter_size, gutter_size);
            stdout.write_all(
                format!(
                    "{}{}|",
                    y,
                    " ".repeat((self.left_gutter_size - gutter_size - 1) as usize)
                )
                .as_bytes(),
            );
            let row = self.rows.get(y).unwrap();
            let mut len = row.len();
            if len > self.screen_cols - self.left_gutter_size as usize {
                len = self.screen_cols - self.left_gutter_size as usize;
            }
            stdout.write_all(row.split_at(len).0.trim_end().as_bytes())?;

            stdout.write_all(b"\x1b[K")?;
            if y < self.row_offset + self.screen_rows {
                stdout.write_all(b"\r\n");
            }
        }

        Ok(())
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.screen_cols = width;
        self.screen_rows = height;
        self.left_gutter_size =
            Self::calculate_gutter(self.row_offset, self.screen_rows, self.rows.len());
    }

    pub fn move_cursor(&mut self, dx: i16, dy: i16) {
        let mut cx = self.cx as i16 + dx;
        let mut cy = self.cy as i16 + dy;
        let mut line = self.rows.get(self.row_offset + self.cy as usize).unwrap();

        if cy >= 0 && cy as usize >= self.rows.len() - self.row_offset {
            cy = (self.rows.len() - self.row_offset - 1) as i16;
        }
        if cy >= 0 && cy as usize > self.screen_rows {
            cy = self.screen_rows as i16;
            self.row_offset += 1;
        }
        if cy < 0 {
            if self.row_offset as isize + cy as isize >= 0 {
                self.row_offset = (self.row_offset as isize + cy as isize) as usize;
            }
            cy = 0;
        }

        line = self.rows.get(self.row_offset + cy as usize).unwrap();
        let max_width = std::cmp::min(self.screen_cols, line.len() as usize + 2);
        if cx >= 0 && cx as usize > max_width {
            cx = max_width as i16;
        }
        if cx <= self.left_gutter_size as i16 {
            cx = self.left_gutter_size as i16 + 1;
        }

        self.cx = cx as u16;
        self.cy = cy as u16;
        // TODO: fix bug where gutter size changes after cursor movement
        self.left_gutter_size =
            Self::calculate_gutter(self.row_offset, self.screen_rows, self.rows.len());
    }

    pub fn get_cursor(&self) -> (u16, u16) {
        (self.cx, self.cy)
    }

    pub fn cursor_home(&mut self) {
        self.move_cursor(0 - self.cx as i16, 0);
    }

    pub fn cursor_end(&mut self) {
        self.move_cursor(
            self.rows
                .get(self.row_offset + self.cy as usize)
                .unwrap_or(&"".to_string())
                .len() as i16,
            0,
        );
    }

    pub fn handle_char(&mut self, c: char) {
        let row_index = self.cy as usize + self.row_offset;
        let mut col_index = (self.cx - self.left_gutter_size - 1) as usize;

        let mut new_line = self
            .rows
            .get(self.cy as usize + self.row_offset)
            .unwrap()
            .clone();
        new_line.insert(col_index, c);

        if self.rows.len() <= row_index {
            self.rows.resize(row_index + 1, "".to_string());
        }
        self.rows[row_index] = new_line;
        self.move_cursor(1, 0);
    }

    fn calculate_gutter(row_offset: usize, screen_rows: usize, rows: usize) -> u16 {
        1 + if row_offset + screen_rows < rows - row_offset {
            std::primitive::f32::log10((row_offset + screen_rows) as f32) as u16
        } else {
            std::primitive::f32::log10((rows - row_offset) as f32) as u16
        }
    }
}
