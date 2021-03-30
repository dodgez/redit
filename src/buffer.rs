use std::cmp::min;

use crate::line::Line;

#[derive(Clone)]
enum Action {
    InsertChar(usize, usize, char),
    DeleteChar(usize, usize, char),
    InsertRegion((usize, usize), Vec<Line>),
    RemoveRegion((usize, usize), (usize, usize), Vec<Line>),
    JoinLine(usize, usize),
    SplitLine(usize, usize),
}

pub struct Buffer {
    lines: Vec<Line>,
    history: Vec<Action>,
    index: usize,
}

impl Default for Buffer {
    fn default() -> Self {
        Buffer {
            lines: vec![],
            history: vec![],
            index: 0,
        }
    }
}

impl Buffer {
    pub fn new(lines: Vec<Line>) -> Self {
        Buffer {
            lines,
            ..Buffer::default()
        }
    }

    pub fn insert_char(&mut self, line_index: usize, column: usize, c: char, log: bool) {
        let line = self.lines.get(line_index).unwrap();
        let mut s = line.get_raw().to_string();
        s.insert(column, c);
        self.lines[line_index] = Line::new(s);
        if log {
            self.log(Action::InsertChar(line_index, column, c));
        }
    }

    pub fn delete_char(&mut self, line_index: usize, column: usize, log: bool) -> bool {
        let line = self.lines.get(line_index).unwrap();
        if column < line.get_clean_raw().len() {
            let mut s = line.get_raw().to_string();
            if log {
                self.log(Action::DeleteChar(
                    line_index,
                    column,
                    *s.chars().collect::<Vec<char>>().get(column).unwrap(),
                ));
            }
            s.remove(column);
            self.lines[line_index] = Line::new(s);
            true
        } else if line_index + 1 < self.get_line_count() {
            let line = line.get_clean_raw();
            let other_line = self
                .lines
                .get(line_index + 1)
                .unwrap()
                .get_raw()
                .to_string();
            self.replace_line(line_index, line.to_string() + &other_line);
            if log {
                self.log(Action::JoinLine(line_index, line.len()));
            }
            true
        } else {
            false
        }
    }

    pub fn get_line(&self, line_index: usize) -> Option<&Line> {
        self.lines.get(line_index)
    }

    pub fn split_line(&mut self, line_index: usize, column: usize, log: bool) {
        let line = self.lines.get(line_index).unwrap();
        let line_ending = line.get_raw().split_at(line.get_clean_raw().len()).1;
        let raw = line.get_raw().to_string();
        let parts = raw.split_at(column);
        let split_row = parts.0.to_string() + line_ending;
        self.replace_line(line_index, split_row);
        self.insert_line(line_index + 1, Line::new(parts.1.to_string()));
        if log {
            self.log(Action::SplitLine(line_index, parts.0.len()));
        }
    }

    pub fn insert_region(
        &mut self,
        start: (usize, usize),
        lines: &[Line],
        log: bool,
    ) -> (usize, usize) {
        // Ensure the markers are inside the file
        let start_y = min(start.1, self.get_line_count());
        let start_x = min(start.0, self.get_line(start_y).unwrap().get_raw().len());
        let line = self.lines.get(start_y).unwrap();
        let clean = line.get_clean_raw();
        let first_half = clean.split_at(start_x).0;
        let second_half = line.get_raw().split_at(start_x).1.to_string();
        if log {
            self.log(Action::InsertRegion(start, lines.to_vec()));
        }

        match lines.len().cmp(&1) {
            std::cmp::Ordering::Greater => {
                self.replace_line(
                    start_y,
                    first_half.to_string() + lines.get(0).unwrap().get_raw(),
                );
                for i in 1..lines.len() - 1 {
                    self.insert_line(start_y + i, lines.get(i).unwrap().clone());
                }
                if self.get_line_count() < start_y + lines.len() {
                    self.replace_line(
                        start_y + lines.len(),
                        lines.last().unwrap().get_raw().to_string() + &second_half,
                    );
                } else {
                    self.lines.push(Line::new(
                        lines.last().unwrap().get_raw().to_string() + &second_half,
                    ));
                }
                (
                    lines.last().unwrap().get_raw().len(),
                    start_y + lines.len() - 1,
                )
            }
            std::cmp::Ordering::Equal => {
                self.replace_line(
                    start_y,
                    first_half.to_string() + &lines.get(0).unwrap().get_clean_raw() + &second_half,
                );
                (
                    first_half.len() + lines.get(0).unwrap().get_clean_raw().len(),
                    start_y,
                )
            }
            _ => (start_x, start_y),
        }
    }

    pub fn get_region(&self, start: (usize, usize), end: (usize, usize)) -> Vec<Line> {
        // Ensure the markers are inside the file
        let start_y = min(start.1, self.get_line_count());
        let start_x = min(start.0, self.get_line(start_y).unwrap().get_raw().len());
        let end_y = min(end.1, self.get_line_count());
        let end_x = min(end.0, self.get_line(end_y).unwrap().get_raw().len());

        let mut text = vec![];
        if start_y != end_y {
            text.push(Line::new(
                self.lines
                    .get(start_y)
                    .unwrap()
                    .get_raw()
                    .split_at(start_x)
                    .1
                    .to_string(),
            ));
            for i in start_y + 1..end_y {
                text.push(self.lines.get(i).unwrap().to_owned());
            }
            text.push(Line::new(
                self.lines
                    .get(end_y)
                    .unwrap()
                    .get_raw()
                    .split_at(end_x)
                    .0
                    .to_string(),
            ));
        } else {
            text.push(Line::new(
                self.lines
                    .get(start_y)
                    .unwrap()
                    .get_raw()
                    .get(start_x..end_x)
                    .unwrap()
                    .to_string(),
            ));
        }
        text
    }

    pub fn remove_region(&mut self, start: (usize, usize), end: (usize, usize), log: bool) {
        // Ensure the markers are inside the file
        let start_y = min(start.1, self.get_line_count());
        let start_x = min(start.0, self.get_line(start_y).unwrap().get_raw().len());
        let end_y = min(end.1, self.get_line_count());
        let end_x = min(end.0, self.get_line(end_y).unwrap().get_raw().len());
        if log {
            self.log(Action::RemoveRegion(
                start,
                end,
                self.get_region(start, end),
            ));
        }

        if start_y != end_y {
            self.replace_line(
                start_y,
                self.lines
                    .get(start_y)
                    .unwrap()
                    .get_raw()
                    .split_at(start_x)
                    .0
                    .to_string()
                    + self.lines.get(end_y).unwrap().get_raw().split_at(end_x).1,
            );
            for _ in start_y + 1..end_y + 1 {
                self.remove_line(start_y + 1);
            }
        } else {
            let mut line = self.lines.get(start_y).unwrap().get_raw().to_string();
            line.replace_range(start_x..end_x, "");
            self.replace_line(start_y, line);
        }
    }

    pub fn get_line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn get_all(&self) -> String {
        self.lines
            .iter()
            .map(|l| l.get_raw())
            .collect::<Vec<&str>>()
            .join("")
    }

    pub fn undo(&mut self) {
        if self.index > 0 {
            let last_item = self.history.get(self.index - 1).unwrap().clone();
            match last_item {
                Action::InsertChar(line_index, column, _) => {
                    self.delete_char(line_index, column, false);
                }
                Action::DeleteChar(line_index, column, c) => {
                    self.insert_char(line_index, column, c, false);
                }
                Action::InsertRegion((start_x, start_y), lines) => match lines.len().cmp(&1) {
                    std::cmp::Ordering::Greater => {
                        let end_y = start_y + lines.len() - 1;
                        let end_x = lines.last().unwrap().get_clean_raw().len();
                        self.remove_region((start_x, start_y), (end_x, end_y), false);
                    }
                    std::cmp::Ordering::Equal => {
                        let end_y = start_y;
                        let end_x = start_x + lines.get(0).unwrap().get_clean_raw().len();
                        self.remove_region((start_x, start_y), (end_x, end_y), false);
                    }
                    _ => {}
                },
                Action::RemoveRegion(start, _, lines) => {
                    self.insert_region(start, &lines, false);
                }
                Action::JoinLine(line_index, column) => {
                    self.split_line(line_index, column, false);
                }
                Action::SplitLine(line_index, _) => {
                    let line = self.get_line(line_index).unwrap().get_clean_raw();
                    let other_line = self
                        .lines
                        .get(line_index + 1)
                        .unwrap()
                        .get_raw()
                        .to_string();
                    self.replace_line(line_index, line + &other_line);
                }
            }
            self.index -= 1;
        }
    }

    pub fn redo(&mut self) {
        if self.index < self.history.len() {
            let last_item = self.history.get(self.index).unwrap().clone();
            match last_item {
                Action::InsertChar(line_index, column, c) => {
                    self.insert_char(line_index, column, c, false);
                }
                Action::DeleteChar(line_index, column, _) => {
                    self.delete_char(line_index, column, false);
                }
                Action::InsertRegion(start, lines) => {
                    self.insert_region(start, &lines, false);
                }
                Action::RemoveRegion(start, end, _) => {
                    self.remove_region(start, end, false);
                }
                Action::JoinLine(line_index, _) => {
                    let line = self.get_line(line_index).unwrap().get_clean_raw();
                    let other_line = self
                        .lines
                        .get(line_index + 1)
                        .unwrap()
                        .get_raw()
                        .to_string();
                    self.replace_line(line_index, line + &other_line);
                }
                Action::SplitLine(line_index, column) => {
                    self.split_line(line_index, column, false);
                }
            }
            self.index += 1;
        }
    }

    fn insert_line(&mut self, line_index: usize, line: Line) {
        self.lines.insert(line_index, line);
    }

    fn remove_line(&mut self, line_index: usize) {
        self.lines.remove(line_index);
    }

    fn replace_line(&mut self, line_index: usize, contents: String) {
        self.lines[line_index] = Line::new(contents);
    }

    fn log(&mut self, action: Action) {
        if self.index < self.history.len() {
            self.history = self.history.split_at(self.index).0.to_vec();
        }
        self.history.push(action);
        self.index += 1;
    }
}
