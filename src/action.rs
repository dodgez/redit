use super::line::Line;

pub enum Action {
    InsertChar(usize, usize, char),
    DeleteChar(usize, usize),
    InsertTextRegion((usize, usize), (usize, usize), Vec<Line>),
    RemoveTextRegion((usize, usize), (usize, usize)),
}
