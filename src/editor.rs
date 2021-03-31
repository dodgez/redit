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
    highlighting::{Color as SynColor, Style, StyleModifier, Theme},
    parsing::SyntaxSet,
    util::modify_range,
};
use tui::{layout::Rect, style::Color as TuiColor};

use crate::buffer::Buffer;
use crate::line::Line;
use crate::render_config::RenderConfig;

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
    buffer: Buffer,
    col_offset: usize,
    confirm_dirty: bool,
    cx: usize,
    cy: usize,
    file_path: Option<PathBuf>,
    highlighting: bool,
    hx: usize,
    hy: usize,
    message: Option<String>,
    draw_area: Rect,
    render_opts: RenderConfig,
    row_offset: usize,
    rx: usize,
    syntaxes: SyntaxSet,
    theme: Theme,
}

// Essentially just replaces tabs with 4 spaces
fn convert_cx_to_rx(line: &Line, cx: usize, render_opts: &RenderConfig) -> usize {
    if cx >= line.get_clean_raw().len() {
        line.render(render_opts).len()
    } else {
        let raw = line.get_raw().split_at(cx).0;
        raw.matches('\t').count() * 3 + cx
    }
}

impl tui::widgets::Widget for &mut Editor {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let bg = self.theme.settings.background.unwrap_or(SynColor::BLACK);
        let bg_color = TuiColor::Rgb(bg.r, bg.g, bg.b);
        let fg = self.theme.settings.foreground.unwrap_or(SynColor::WHITE);
        let fg_color = TuiColor::Rgb(fg.r, fg.g, fg.b);
        let highlight_style = StyleModifier {
            background: Some(fg),
            foreground: Some(bg),
            font_style: None,
        };

        let block = tui::widgets::Block::default()
            .title(format!(
                "{} L{}:C{}",
                self.file_path
                    .as_ref()
                    .map(|p| p.to_str().unwrap().to_string())
                    .unwrap_or_else(|| "[No file]".to_string()),
                self.cy + 1,
                self.cx + 1
            ))
            .borders(tui::widgets::Borders::ALL)
            .style(tui::style::Style::default().fg(fg_color).bg(bg_color));
        let inner_area = block.inner(area);
        block.render(area, buf);
        self.draw_area = inner_area;
        let syntax = self
            .file_path
            .as_ref()
            .and_then(|f| f.extension())
            .and_then(|e| self.syntaxes.find_syntax_by_extension(&e.to_string_lossy()));
        let lines = self.buffer.get_line_count();
        let max_gutter_size = (if lines < 2 { 2 } else { lines + 1 } as f32)
            .log10()
            .ceil();
        for y in 0..inner_area.height as usize {
            if let Some(line) = self
                .buffer
                .get_line(self.row_offset + y)
                .map(|l| l.render(&self.render_opts))
            {
                let line_number = self.row_offset + y;
                let gutter_size = (if line_number < 2 { 2 } else { line_number + 2 } as f32)
                    .log10()
                    .ceil();
                let raw_line = if (self.col_offset >= line.len()) {
                    ""
                } else {
                    line.split_at(self.col_offset).1
                };

                let mut h = syntax.map(|s| HighlightLines::new(s, &self.theme));
                if let Some(mut h) = h {
                    let mut line = h.highlight(raw_line, &self.syntaxes);
                    if self.highlighting && y >= min(self.cy, self.hy) && y <= max(self.cy, self.hy)
                    {
                        if self.cy == self.hy {
                            if self.cx < self.hx {
                                line = modify_range(&line, self.cx..self.hx, highlight_style);
                            } else {
                                line = modify_range(&line, self.hx..self.cx, highlight_style);
                            }
                        } else if y == min(self.cy, self.hy) {
                            if self.cy < self.hy {
                                line =
                                    modify_range(&line, self.cx..raw_line.len(), highlight_style);
                            } else {
                                line =
                                    modify_range(&line, self.hx..raw_line.len(), highlight_style);
                            }
                        } else if y == max(self.cy, self.hy) {
                            if self.cy < self.hy {
                                line = modify_range(&line, 0..self.hx, highlight_style);
                            } else {
                                line = modify_range(&line, 0..self.cx, highlight_style);
                            }
                        } else {
                            line = modify_range(&line, 0..raw_line.len(), highlight_style);
                        }
                    }
                    let line = tui::text::Spans::from(
                        line.iter()
                            .map(|(style, text)| {
                                let fg_rgb = style.foreground;
                                let bg_rgb = style.background;
                                tui::text::Span {
                                    content: std::borrow::Cow::Borrowed(text),
                                    style: tui::style::Style::default()
                                        .fg(TuiColor::Rgb(fg_rgb.r, fg_rgb.g, fg_rgb.b))
                                        .bg(TuiColor::Rgb(bg_rgb.r, bg_rgb.g, bg_rgb.b)),
                                }
                            })
                            .collect::<Vec<tui::text::Span>>(),
                    );
                    buf.set_string(
                        inner_area.x,
                        inner_area.y + y as u16,
                        format!(
                            "{}{}|",
                            " ".repeat((max_gutter_size - gutter_size) as usize),
                            line_number + 1
                        ),
                        tui::style::Style::default(),
                    );
                    buf.set_spans(
                        inner_area.x + max_gutter_size as u16 + 1,
                        inner_area.y + y as u16,
                        &line,
                        inner_area.width as u16 - max_gutter_size as u16 - 1,
                    );
                } else {
                    buf.set_stringn(
                        inner_area.x,
                        inner_area.y + y as u16,
                        format!(
                            "{}{}|{}",
                            " ".repeat((max_gutter_size - gutter_size) as usize),
                            line_number + 1,
                            line
                        ),
                        inner_area.width as usize - max_gutter_size as usize - 1, // Account for line numbers
                        tui::style::Style::default(),
                    );
                };
            }
        }
    }
}

impl Editor {
    pub fn new(syntaxes: SyntaxSet) -> Self {
        Editor {
            buffer: Buffer::new(vec![Line::new("Redit version 0.1.0".to_string())]),
            syntaxes,
            ..Editor::default()
        }
    }

    pub fn open_file(&mut self, file_name: &dyn AsRef<Path>) -> std::io::Result<()> {
        let file = File::open(file_name)?;
        let mut reader = BufReader::new(file);
        let mut rows = vec![];

        loop {
            let mut temp = String::new();
            let n = reader.read_line(&mut temp)?;
            rows.push(Line::new(temp));
            if n == 0 {
                break;
            }
        }

        let mut file_name = file_name.as_ref().to_path_buf();
        if let Ok(path) = file_name.canonicalize() {
            file_name = path;
        }
        self.buffer = Buffer::new(rows);
        self.file_path = Some(file_name);
        self.set_message(&"File opened.");
        self.confirm_dirty = false;

        Ok(())
    }

    pub fn open(&mut self) {
        if !self.buffer.is_dirty() || self.confirm_dirty {
            panic!("Prompt not implemented");
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

            let contents = self.buffer.get_all();
            br.write_all(contents.as_bytes())?;
            self.set_message(&"File saved.");
            self.buffer.set_clean();
            self.confirm_dirty = false;
        } else {
            panic!("Prompt not implemented");
        }

        Ok(())
    }

    pub fn try_quit(&mut self) -> bool {
        if !self.buffer.is_dirty() || self.confirm_dirty {
            true
        } else {
            self.confirm_dirty = true;
            self.set_message(&"Press Ctrl-q again to quit");
            false
        }
    }

    pub fn try_reload(&mut self) -> std::io::Result<()> {
        if !self.buffer.is_dirty() || self.confirm_dirty {
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

    pub fn load_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    pub fn get_rel_cursor(&self) -> (u16, u16) {
        let lines = self.buffer.get_line_count();
        let max_gutter_size = (if lines < 2 { 2 } else { lines + 1 } as f32)
            .log10()
            .ceil();
        (
            (self.rx - self.col_offset + max_gutter_size as usize + 1) as u16
                + self.draw_area.x,
            (self.cy - self.row_offset) as u16 + self.draw_area.y,
        )
    }

    pub fn move_cursor(&mut self, pos: Movement, with_highlight: bool) {
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
                if let Some(line) = self.buffer.get_line(self.cy).map(|l| l.get_clean_raw()) {
                    self.cx = line.len();
                }
            }
            Movement::PageUp => {
                let rel = self.cy - self.row_offset;
                self.cy = self.row_offset;
                let rollback = self.row_offset >= self.draw_area.height as usize;
                self.move_cursor(
                    Movement::Relative(0, 0 - (self.draw_area.height as isize)),
                    with_highlight,
                );
                if rollback {
                    self.move_cursor(Movement::Relative(0, rel as isize), with_highlight);
                }
            }
            Movement::PageDown => {
                let rel = self.cy - self.row_offset;
                self.cy = self.row_offset + self.draw_area.height as usize;
                let rollback = self.cy < self.buffer.get_line_count() - 1; // -1 because row_offset can never get bigger
                self.move_cursor(
                    Movement::Relative(0, self.draw_area.height as isize),
                    with_highlight,
                );
                if rollback {
                    self.move_cursor(
                        Movement::Relative(0, 0 - (self.draw_area.height as usize - rel) as isize),
                        with_highlight,
                    );
                }
            }
            // Up
            Movement::Relative(0, dy) if dy < 0 => {
                let new_cy = self.cy as isize + dy;
                let new_cy = if new_cy < 0 { 0 } else { new_cy };
                if new_cy >= 0 {
                    if let Some(line) = self
                        .buffer
                        .get_line(new_cy as usize)
                        .map(|l| l.get_clean_raw())
                    {
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
                let new_cy = if new_cy >= self.buffer.get_line_count() {
                    self.buffer.get_line_count() - 1
                } else {
                    new_cy
                };
                if let Some(line) = self.buffer.get_line(new_cy).map(|l| l.get_clean_raw()) {
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
                if let Some(line) = self.buffer.get_line(self.cy).map(|l| l.get_clean_raw()) {
                    if self.cx + dx as usize > line.len() {
                        if self.cy < self.buffer.get_line_count() - 1 {
                            self.move_cursor(Movement::Relative(0, 1), with_highlight);
                            self.move_cursor(Movement::Home, with_highlight);
                        }
                    } else {
                        self.cx += dx as usize;
                    }
                }
            }
            Movement::Absolute(x, y) => {
                self.cy = min(y, self.buffer.get_line_count() - 1); // There should be at least one row
                self.cx = min(x, self.buffer.get_line(self.cy).unwrap().get_raw().len());
            }
            Movement::AbsoluteScreen(x, y) => {
                let lines = self.buffer.get_line_count();
                self.cy = min(self.row_offset + y, lines - 1);
                let row_len = self.buffer.get_line(self.cy).unwrap().get_raw().len();
                let max_gutter_size = (if lines < 2 { 2 } else { lines + 1 } as f32)
                    .log10()
                    .ceil() as usize
                    + 1;
                self.cx = min(
                    if max_gutter_size >= x {
                        0
                    } else {
                        x - max_gutter_size
                    },
                    if row_len > 0 { row_len - 1 } else { 0 },
                );
            }
            _ => {}
        }

        self.scroll();
    }

    fn remove_highlight(&mut self) {
        if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
            self.buffer
                .remove_region((self.cx, self.cy), (self.hx, self.hy), true);
            self.confirm_dirty = false;
        } else {
            self.buffer
                .remove_region((self.hx, self.hy), (self.cx, self.cy), true);
            self.move_cursor(Movement::Absolute(self.hx, self.hy), false);
            self.confirm_dirty = false;
        }
    }

    pub fn write_char(&mut self, c: char) {
        if self.cy < self.buffer.get_line_count() {
            self.buffer.insert_char(self.cy, self.cx, c, true);
            self.move_cursor(Movement::Relative(1, 0), false);
            self.confirm_dirty = false;
        }
    }

    pub fn delete_char(&mut self) {
        if self.highlighting {
            self.remove_highlight();
            self.highlighting = false;
        } else if self.cy < self.buffer.get_line_count() {
            self.buffer.delete_char(self.cy, self.cx, true);
            self.confirm_dirty = false;
        }
    }

    pub fn backspace_char(&mut self) {
        if self.highlighting {
                self.remove_highlight();
                self.highlighting = false;
            }
            if self.cx > 0 || self.cy > 0 {
                self.move_cursor(Movement::Relative(-1, 0), false);
                self.delete_char();
                self.confirm_dirty = false;
            }
    }

    pub fn do_return(&mut self) {
        if self.highlighting {
            self.remove_highlight();
            self.highlighting = false;
        }
        if self.cy < self.buffer.get_line_count() {
            self.buffer.split_line(self.cy, self.cx, true);
            self.move_cursor(Movement::Relative(0, 1), false);
            self.move_cursor(Movement::Home, false);
        }
    }

    pub fn cut(&mut self) -> Vec<Line> {
        let clipboard = self.copy();
        if self.highlighting {
            self.remove_highlight();
            self.highlighting = false;
        }
        clipboard
    }
    pub fn copy(&mut self) -> Vec<Line> {
        let mut clipboard = vec![];
        if self.highlighting {
            if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
                clipboard = self
                    .buffer
                    .get_region((self.cx, self.cy), (self.hx, self.hy));
            } else {
                clipboard = self
                    .buffer
                    .get_region((self.hx, self.hy), (self.cx, self.cy));
            }
        }
        clipboard
    }
    pub fn paste(&mut self, clipboard: &Option<Vec<Line>>) {
        if let Some(clipboard) = clipboard {
            if self.highlighting {
                self.remove_highlight();
                self.highlighting = false;
            }
            if self.cy < self.buffer.get_line_count() {
                let new_pos = self
                    .buffer
                    .insert_region((self.cx, self.cy), clipboard, true);
                self.move_cursor(Movement::Absolute(new_pos.0, new_pos.1), false);
                self.confirm_dirty = false;
            }
        }
    }

    // fn check_prompt(&mut self) {
    //     let answer = self.prompt.get_answer();
    //     match self.prompt.purpose {
    //         PromptPurpose::Save => {
    //             if let Some(answer) = answer {
    //                 self.file_path = Some(Path::new(answer).to_path_buf());
    //                 if let Err(e) = self.save() {
    //                     self.set_message(&"Error writing to file");
    //                 }
    //             }
    //         }
    //         PromptPurpose::Open => {
    //             if let Some(answer) = answer {
    //                 let path = Path::new(answer).to_path_buf();
    //                 if let Err(e) = self.open_file(&path) {
    //                     self.set_message(&"Error opening file");
    //                 }
    //             }
    //         }
    //         _ => {}
    //     }
    //     self.cancel_prompt();
    // }

    // pub fn cancel_prompt(&mut self) {
    //     self.confirm_dirty = false;
    //     self.prompt.exit();
    //     self.message = None;
    // }

    pub fn undo(&mut self) {
        self.buffer.undo();
        self.confirm_dirty = false;
    }

    pub fn redo(&mut self) {
        self.buffer.redo();
        self.confirm_dirty = false;
    }

    fn set_message(&mut self, message: &dyn AsRef<str>) {
        self.message = Some(format!(
            "{}: {}",
            Local::now().format("%I:%M:%S %P"),
            message.as_ref()
        ));
    }

    fn scroll(&mut self) {
        if self.buffer.get_line(self.cy).is_none() {
            return;
        }
        self.rx = convert_cx_to_rx(
            self.buffer.get_line(self.cy).unwrap(),
            self.cx,
            &self.render_opts,
        );

        if self.rx < self.col_offset {
            self.col_offset = self.rx;
        }
        let lines = self.buffer.get_line_count();
        let max_gutter_size = (if lines < 2 { 2 } else { lines + 1 } as f32)
            .log10()
            .ceil() as usize
            + 1;
        if self.draw_area.width != 0
            && self.rx + max_gutter_size >= self.col_offset + self.draw_area.width as usize
        {
            self.col_offset = self.rx + max_gutter_size - self.draw_area.width as usize;
        }
        if self.cy < self.row_offset {
            self.row_offset = self.cy;
        }
        if self.cy >= self.row_offset + self.draw_area.height as usize && self.draw_area.height != 0
        {
            self.row_offset = self.cy - self.draw_area.height as usize + 1;
        }
    }
}
