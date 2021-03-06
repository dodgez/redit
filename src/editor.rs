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
use tui::{
    buffer::Buffer as TuiBuffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color as TuiColor, Style as TuiStyle},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

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
    ScrollUp(usize),
    ScrollDown(usize),
    Absolute(usize, usize),
    AbsoluteScreen(u16, u16),
    Relative(isize, isize),
}

#[derive(Default)]
pub struct Editor {
    buffer: Buffer,
    col_offset: usize,
    confirm_dirty: bool,
    cx: usize,
    cy: usize,
    pub draw_area: Rect,
    file_path: Option<PathBuf>,
    highlighting: bool,
    hx: usize,
    hy: usize,
    message: Option<String>,
    prompt_message: Option<String>,
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

impl Widget for &mut Editor {
    fn render(self, area: Rect, buf: &mut TuiBuffer) {
        let bg = self.theme.settings.background.unwrap_or(SynColor::BLACK);
        let bg_color = TuiColor::Rgb(bg.r, bg.g, bg.b);
        let fg = self.theme.settings.foreground.unwrap_or(SynColor::WHITE);
        let fg_color = TuiColor::Rgb(fg.r, fg.g, fg.b);
        let default_style = Style::default().apply(StyleModifier {
            background: Some(bg),
            foreground: Some(fg),
            font_style: None,
        });
        let highlight_style = StyleModifier {
            background: Some(fg),
            foreground: Some(bg),
            font_style: None,
        };

        let block = Block::default()
            .borders(Borders::TOP)
            .style(TuiStyle::default().fg(fg_color).bg(bg_color));
        let inner_area = block.inner(area);
        block.render(area, buf);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(inner_area.height - 2), Constraint::Min(2)])
            .split(inner_area);
        self.draw_area = chunks[0];
        let syntax = self
            .file_path
            .as_ref()
            .and_then(|f| f.extension())
            .and_then(|e| self.syntaxes.find_syntax_by_extension(&e.to_string_lossy()));
        let lines = self.buffer.get_line_count();
        let max_gutter_size = (if lines < 2 { 2 } else { lines + 1 } as f32)
            .log10()
            .ceil();
        for y in 0..self.draw_area.height as usize {
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
                let mut line = h
                    .map(|mut h| h.highlight(raw_line, &self.syntaxes))
                    .unwrap_or_else(|| vec![(default_style, raw_line)]);
                if self.highlighting
                    && line_number >= min(self.cy, self.hy)
                    && line_number <= max(self.cy, self.hy)
                {
                    if self.cy == self.hy {
                        if self.cx < self.hx {
                            line = modify_range(&line, self.cx..self.hx, highlight_style);
                        } else {
                            line = modify_range(&line, self.hx..self.cx, highlight_style);
                        }
                    } else if line_number == min(self.cy, self.hy) {
                        if self.cy < self.hy {
                            line = modify_range(&line, self.cx..raw_line.len(), highlight_style);
                        } else {
                            line = modify_range(&line, self.hx..raw_line.len(), highlight_style);
                        }
                    } else if line_number == max(self.cy, self.hy) {
                        if self.cy < self.hy {
                            line = modify_range(&line, 0..self.hx, highlight_style);
                        } else {
                            line = modify_range(&line, 0..self.cx, highlight_style);
                        }
                    } else {
                        line = modify_range(&line, 0..raw_line.len(), highlight_style);
                    }
                }
                let line = Spans::from(
                    line.iter()
                        .map(|(style, text)| {
                            let fg_rgb = style.foreground;
                            let bg_rgb = style.background;
                            Span {
                                content: std::borrow::Cow::Borrowed(text),
                                style: TuiStyle::default()
                                    .fg(TuiColor::Rgb(fg_rgb.r, fg_rgb.g, fg_rgb.b))
                                    .bg(TuiColor::Rgb(bg_rgb.r, bg_rgb.g, bg_rgb.b)),
                            }
                        })
                        .collect::<Vec<Span>>(),
                );
                buf.set_string(
                    self.draw_area.x,
                    self.draw_area.y + y as u16,
                    format!(
                        "{}{}| ",
                        " ".repeat((max_gutter_size - gutter_size) as usize),
                        line_number + 1
                    ),
                    TuiStyle::default(),
                );
                buf.set_spans(
                    self.draw_area.x + max_gutter_size as u16 + 2, //+1 for pipe and space
                    self.draw_area.y + y as u16,
                    &line,
                    self.draw_area.width as u16 - max_gutter_size as u16 - 2, // -1 for pipe and space
                );
            }
        }

        // Draw the message
        let p = Paragraph::new(Span::raw(
            self.message
                .as_ref()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "[No Message]".to_string()),
        ))
        .block(
            Block::default()
                .title(format!("L{}:C{} {}", self.cy + 1, self.cx + 1, "Message "))
                .borders(Borders::TOP),
        )
        .wrap(Wrap { trim: true });
        p.render(chunks[1], buf);
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

        let file_name = file_name.as_ref().to_path_buf();
        self.buffer = Buffer::new(rows);
        self.file_path = Some(file_name);
        self.set_message(&"File opened.");
        self.confirm_dirty = false;

        Ok(())
    }

    pub fn save(&mut self) -> std::io::Result<bool> {
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
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn save_as(&mut self, path: PathBuf) -> std::io::Result<()> {
        self.file_path = Some(path);
        self.save()?;
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
                self.open_file(&file)?;
                self.set_message(&"File reloaded from disk");
            } else {
                self.set_message(&"No file to reload");
            }
        } else {
            self.confirm_dirty = true;
            self.set_message(&"Press Ctrl-r again to reload from disk");
        }
        Ok(())
    }

    pub fn get_title(&self) -> String {
        self.file_path
            .as_ref()
            .map(|p| p.to_str().unwrap())
            .unwrap_or_else(|| "[No file]")
            .to_string()
    }

    pub fn take_prompt(&mut self) -> Option<String> {
        self.prompt_message.take()
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
            (self.rx - self.col_offset + max_gutter_size as usize + 2) as u16 + self.draw_area.x, // +2 for pipe and space
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
                        Movement::Relative(0, 1 - (self.draw_area.height as usize - rel) as isize), // 1- so not to go off screen
                        with_highlight,
                    );
                }
            }
            Movement::ScrollUp(dy) => {
                self.row_offset -= min(self.row_offset, dy);
                if self.cy >= self.row_offset + self.draw_area.height as usize
                    && self.draw_area.height != 0
                {
                    self.cy = self.row_offset + self.draw_area.height as usize - 1;
                }
            }
            Movement::ScrollDown(dy) => {
                self.row_offset += min(self.buffer.get_line_count() - self.row_offset - 1, dy);
                if self.cy < self.row_offset {
                    self.cy = self.row_offset;
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
                let x = x as usize;
                let y = y as usize;
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
        } else if self.cx > 0 || self.cy > 0 {
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

    pub fn undo(&mut self) {
        self.buffer.undo();
        self.confirm_dirty = false;
    }

    pub fn redo(&mut self) {
        self.buffer.redo();
        self.confirm_dirty = false;
    }

    pub fn set_message(&mut self, message: &dyn AsRef<str>) {
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
