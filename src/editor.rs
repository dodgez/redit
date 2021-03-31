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

use crate::buffer::Buffer;
use crate::line::Line;
use crate::prompt::{Prompt, PromptPurpose};
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
    bottom_gutter_size: usize,
    buffer: Buffer,
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
    prompt: Prompt,
    render_opts: RenderConfig,
    row_offset: usize,
    rx: usize,
    screen_cols: usize,
    screen_rows: usize,
    syntaxes: SyntaxSet,
    theme: Theme,
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

impl tui::widgets::Widget for &mut Editor {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        use tui::style::Color as TuiColor;
        let bg = self.theme.settings.background.unwrap_or(SynColor::BLACK);
        let bg_color = TuiColor::Rgb(
            bg.r,
            bg.g,
            bg.b,
        );
        let fg = self.theme.settings.foreground.unwrap_or(SynColor::WHITE);
        let fg_color = TuiColor::Rgb(
            fg.r,
            fg.g,
            fg.b,
        );
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

        let block = tui::widgets::Block::default().title(self.file_path.as_ref().map(|p| p.to_str().unwrap().to_string()).unwrap_or_else(|| "[No file]".to_string())).borders(tui::widgets::Borders::ALL).style(tui::style::Style::default().fg(fg_color).bg(bg_color));
        let inner_area = block.inner(area);
        block.render(area, buf);
        for y in 0..inner_area.height as usize {
            if let Some(line) = self.buffer.get_line(self.row_offset + y).map(|l| l.get_clean_raw()) {
                let line = if (self.col_offset >= line.len()) {
                    ""
                } else {
                    line.split_at(self.col_offset).1
                };

                let mut h = syntax.map(|s| HighlightLines::new(s, &self.theme));
                if let Some(mut h) = h {
                    let line = tui::text::Spans::from(h.highlight(line, &self.syntaxes).iter().map(|(style, text)| {
                        let fg_rgb = style.foreground;
                        let bg_rgb = style.background;
                        tui::text::Span {
                            content: std::borrow::Cow::Borrowed(text),
                            style: tui::style::Style::default().fg(TuiColor::Rgb(fg_rgb.r, fg_rgb.g, fg_rgb.b)).bg(TuiColor::Rgb(bg_rgb.r, bg_rgb.g, bg_rgb.b))
                        }
                    }).collect::<Vec<tui::text::Span>>());
                    buf.set_spans(inner_area.x, inner_area.y + y as u16, &line, inner_area.width);
                } else {
                    buf.set_stringn(inner_area.x, inner_area.y + y as u16, line, inner_area.width as usize, tui::style::Style::default());
                };
            }
        }
    }
}

impl Editor {
    pub fn new(rows: usize, cols: usize, syntaxes: SyntaxSet) -> Self {
        let mut e = Editor {
            buffer: Buffer::new(vec![Line::new("Redit version 0.1.0".to_string())]),
            syntaxes,
            ..Editor::default()
        };
        e.resize(cols, rows);
        e
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
        self.update_left_gutter();
        self.file_path = Some(file_name);
        self.set_message(&"File opened.");
        self.dirty = false;
        self.confirm_dirty = false;

        Ok(())
    }

    pub fn open(&mut self) {
        if !self.dirty || self.confirm_dirty {
            self.prompt = Prompt::new("File to open".to_string(), PromptPurpose::Open);
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
            self.dirty = false;
            self.confirm_dirty = false;
        } else {
            self.prompt = Prompt::new("New file name".to_string(), PromptPurpose::Save);
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

    pub fn load_theme(&mut self, theme: Theme) {
        self.theme = theme;
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

        for y in self.row_offset
            ..min(
                self.buffer.get_line_count(),
                self.row_offset + self.screen_rows + 1,
            )
        {
            let gutter_size = (if y < 2 { 2 } else { 2 + y } as f32).log10().ceil() as usize; // 2+ so line numbers start at 1
            stdout.write_all(
                format!(
                    "{}{}|",
                    " ".repeat(self.left_gutter_size - gutter_size - 1), // Get difference not including separator
                    y + 1 // Line numbering starts at 1
                )
                .as_bytes(),
            );
            let row = self.buffer.get_line(y).unwrap().render(&self.render_opts); // Safe because of array bounds
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
        for y in self.buffer.get_line_count()..self.row_offset + self.screen_rows + 1 {
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
        self.left_gutter_size = Self::calculate_left_gutter(
            self.row_offset,
            self.screen_rows,
            self.buffer.get_line_count(),
        );
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
                if let Some(line) = self.buffer.get_line(self.cy).map(|l| l.get_clean_raw()) {
                    self.cx = line.len();
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
                let rollback = self.cy < self.buffer.get_line_count() - 1; // -1 because row_offset can never get bigger
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
                self.cy = min(self.row_offset + y, self.buffer.get_line_count() - 1);
                let row_len = self.buffer.get_line(self.cy).unwrap().get_raw().len();
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

    fn remove_highlight(&mut self) {
        if self.cy < self.hy || (self.cy == self.hy && self.cx <= self.hx) {
            self.buffer
                .remove_region((self.cx, self.cy), (self.hx, self.hy), true);
        } else {
            self.buffer
                .remove_region((self.hx, self.hy), (self.cx, self.cy), true);
            self.move_cursor(Movement::Absolute(self.hx, self.hy), false);
        }
    }

    pub fn write_char(&mut self, c: char) {
        if self.prompt.is_active() {
            self.prompt.add_char(c);
        } else if self.cy < self.buffer.get_line_count() {
            self.buffer.insert_char(self.cy, self.cx, c, true);
            self.move_cursor(Movement::Relative(1, 0), false);
            self.make_dirty();
        }
    }

    pub fn delete_char(&mut self) {
        if self.prompt.is_active() {
            return;
        }
        if self.highlighting {
            self.remove_highlight();
            self.highlighting = false;
            self.make_dirty();
        } else if self.cy < self.buffer.get_line_count()
            && self.buffer.delete_char(self.cy, self.cx, true)
        {
            self.make_dirty();
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
                self.remove_highlight();
                self.highlighting = false;
                self.make_dirty();
            }
            if self.cy < self.buffer.get_line_count() {
                self.buffer.split_line(self.cy, self.cx, true);
                self.move_cursor(Movement::Relative(0, 1), false);
                self.move_cursor(Movement::Home, false);
                self.make_dirty();
            }
        }
    }

    pub fn cut(&mut self) -> Vec<Line> {
        let clipboard = self.copy();
        if self.highlighting {
            self.remove_highlight();
            self.highlighting = false;
            self.make_dirty();
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
            }
            self.make_dirty();
        }
    }

    fn check_prompt(&mut self) {
        let answer = self.prompt.get_answer();
        match self.prompt.purpose {
            PromptPurpose::Save => {
                if let Some(answer) = answer {
                    self.file_path = Some(Path::new(answer).to_path_buf());
                    if let Err(e) = self.save() {
                        self.set_message(&"Error writing to file");
                    }
                }
            }
            PromptPurpose::Open => {
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

    pub fn undo(&mut self) {
        self.buffer.undo();
    }

    pub fn redo(&mut self) {
        self.buffer.redo();
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
        let new_gutter = Self::calculate_left_gutter(
            self.row_offset,
            self.screen_rows,
            self.buffer.get_line_count(),
        );
        self.screen_cols = (self.screen_cols + self.left_gutter_size) - new_gutter;
        self.left_gutter_size = new_gutter;
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
