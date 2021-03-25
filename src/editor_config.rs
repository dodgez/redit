#![allow(unused)]

use std::fs::File;
use std::io::prelude::*;
use std::io::BufReader;
use std::path::Path;

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
            let gutter_size =
                std::primitive::f32::log10(if y == 0 { 1 } else { y } as f32) as usize;
            // left_gutter - 1 because of pipe char
            stdout.write_all(
                format!(
                    "{}{}|",
                    y,
                    " ".repeat(self.left_gutter_size - gutter_size - 1)
                )
                .as_bytes(),
            );
            let row = self.rows.get(y).unwrap();
            let col_split = if (self.col_offset >= row.len()) {
                ""
            } else {
                row.split_at(self.col_offset).1
            };
            let mut len = col_split.len();
            if len > self.screen_cols - self.left_gutter_size {
                len = self.screen_cols - self.left_gutter_size;
            }
            stdout.write_all(col_split.split_at(len).0.trim_end().as_bytes())?;

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

    pub fn move_cursor(&mut self, dx: isize, dy: isize) {
        let mut cx = self.cx as isize + dx;
        let mut cy = self.cy as isize + dy;
        let mut line = self.rows.get(self.row_offset + self.cy).unwrap();

        if cy >= 0 && cy as usize >= self.rows.len() - self.row_offset {
            cy = self.rows.len() as isize - self.row_offset as isize - 1;
        }
        if cy >= 0 && cy as usize > self.screen_rows {
            cy = self.screen_rows as isize;
            self.row_offset += 1;
        }
        if cy < 0 {
            if self.row_offset as isize + cy >= 0 {
                self.row_offset = (self.row_offset as isize + cy) as usize;
            }
            cy = 0;
        }

        line = self.rows.get(self.row_offset + cy as usize).unwrap();
        let mut line_len = line.len() + self.left_gutter_size;
        if cx >= 0 && cx as usize + self.col_offset > line_len {
            if self.cx + self.col_offset == line_len && cy < self.screen_rows as isize {
                cx = self.left_gutter_size as isize + 1;
                cy += 1
            } else {
                cx = line_len as isize - self.col_offset as isize;
            }
        }
        if cx >= 0 && cx as usize > self.screen_cols {
            self.col_offset = std::cmp::min(self.col_offset + cx as usize, line_len)
                - std::cmp::min(self.screen_cols, line_len);
            cx = self.screen_cols as isize;
        }
        if cx <= self.left_gutter_size as isize {
            if self.col_offset > 0 {
                let cx_diff = self.left_gutter_size as isize - cx + 1;
                if (self.col_offset as isize - cx_diff) < 0 {
                    self.col_offset = 0;
                } else {
                    self.col_offset = (self.col_offset as isize - cx_diff) as usize;
                }
                cx = self.left_gutter_size as isize + 1;
            } else if cy > 0 {
                cy -= 1;
                line = self.rows.get(self.row_offset + cy as usize).unwrap();
                line_len = line.len() + self.left_gutter_size;
                if line_len > self.screen_cols {
                    self.col_offset = line_len - self.screen_cols;
                    cx = self.screen_cols as isize;
                } else {
                    cx = line_len as isize;
                };
            } else {
                cx = self.left_gutter_size as isize + 1;
            }
        }

        let new_gutter = Self::calculate_gutter(self.row_offset, self.screen_rows, self.rows.len());

        self.cx = (cx + new_gutter as isize - self.left_gutter_size as isize) as usize;
        self.cy = cy as usize;

        self.left_gutter_size = new_gutter;
    }

    pub fn get_cursor(&self) -> (u16, u16) {
        (self.cx as u16, self.cy as u16)
    }

    pub fn cursor_home(&mut self) {
        self.move_cursor(
            0 - self.cx as isize - self.col_offset as isize + self.left_gutter_size as isize + 1,
            0,
        );
    }

    pub fn cursor_end(&mut self) {
        self.move_cursor(
            self.rows.get(self.row_offset + self.cy).unwrap().len() as isize
                - self.cx as isize
                - self.col_offset as isize
                + self.left_gutter_size as isize,
            0,
        );
    }

    pub fn handle_char(&mut self, c: char) {
        let row_index = self.cy + self.row_offset;
        let mut col_index = self.col_offset + (self.cx - self.left_gutter_size - 1);

        let mut new_line = self.rows.get(self.cy + self.row_offset).unwrap().clone();
        new_line.insert(col_index, c);

        if self.rows.len() <= row_index {
            self.rows.resize(row_index + 1, "".to_string());
        }
        self.rows[row_index] = new_line;
        self.move_cursor(1, 0);
    }

    fn calculate_gutter(row_offset: usize, screen_rows: usize, rows: usize) -> usize {
        1 + if screen_rows < rows - row_offset {
            std::primitive::f32::floor(std::primitive::f32::log10(
                (row_offset + screen_rows) as f32,
            )) as usize
        } else {
            std::primitive::f32::floor(std::primitive::f32::log10(rows as f32)) as usize
        }
    }
}
