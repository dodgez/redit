use std::path::PathBuf;

use clap::{App, Arg};
use crossterm::{
    event::{read, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use dirs::home_dir;
use serde_derive::Deserialize;
use syntect::{
    highlighting::{Color as SynColor, ThemeSet},
    parsing::SyntaxSet,
};
use tui::{
    backend::CrosstermBackend,
    style::{Color as TuiColor, Style as TuiStyle},
    Terminal,
};

use redit::{
    editor::{Editor, Movement},
    prompt::Prompt,
};

#[derive(Deserialize)]
struct Config {
    theme: String,
}

fn edit(file: Option<&str>) -> crossterm::Result<()> {
    let mut ps = SyntaxSet::load_defaults_newlines().into_builder();
    let config_dir = home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".config/redit");
    let config_file = config_dir.join("settings.toml");
    let mut config: Config = Config {
        theme: "Solarized (dark)".to_string(),
    };
    if config_file.exists() {
        let contents = std::fs::read_to_string(config_file)?;
        config = toml::from_str(&contents).expect("Failed to parse settings");
    }
    let syntax_dir = config_dir.join("syntaxes");
    if syntax_dir.exists() && ps.add_from_folder(syntax_dir, true).is_err() {
        eprintln!("Couldn't load syntaxes");
    }
    let ps = ps.build();
    let theme_dir = config_dir.join("themes");
    let mut theme_set = ThemeSet::load_defaults();
    if theme_dir.exists() && theme_set.add_from_folder(theme_dir).is_err() {
        eprintln!("Couldn't load themes");
    }
    let theme = &theme_set.themes[&config.theme];
    let bg = theme.settings.background.unwrap_or(SynColor::BLACK);
    let bg_color = TuiColor::Rgb(bg.r, bg.g, bg.b);
    let fg = theme.settings.foreground.unwrap_or(SynColor::WHITE);
    let fg_color = TuiColor::Rgb(fg.r, fg.g, fg.b);
    let sel = theme.settings.accent.unwrap_or(SynColor {r: 0, g: 0xFF, b: 0xFF, a: 0xFF});
    let sel_color = TuiColor::Rgb(sel.r, sel.g, sel.b);

    let mut editors = vec![Editor::new(ps.clone())];
    let mut editor_index = 0;
    let mut e = editors.get_mut(editor_index).unwrap();
    e.load_theme(theme.clone());
    if let Some(file) = file {
        if file.starts_with('~') {
            let path = home_dir().expect("Cannot find home directory").join(file.split_at(2).1);
            e.open_file(&path.to_str().expect("Failed to use home directory"))?;
        } else {
            e.open_file(&file)?;
        }
    }

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut clipboard = None;
    let mut prompt: Option<Prompt> = None;

    terminal.draw(|f| {
        use tui::{
            layout::{Constraint, Direction, Layout},
            style::Style,
            text::Spans,
            widgets::{Block, Borders, Tabs},
        };
        let size = f.size();
        let main_block = Block::default()
            .borders(Borders::ALL)
            .style(TuiStyle::default().fg(fg_color).bg(bg_color));
        let inner_area = main_block.inner(size);
        let mut constraints = vec![Constraint::Length(1), Constraint::Min(1)];
        if prompt.is_some() {
            constraints.push(Constraint::Length(2));
        }
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner_area);
        let tabs = Tabs::new(editors.iter().map(|e| Spans::from(e.get_title())).collect())
            .select(editor_index)
            .highlight_style(Style::default().fg(sel_color))
            .divider("|");
        f.render_widget(main_block, size);
        f.render_widget(tabs, chunks[0]);
        f.render_widget(&mut editors[editor_index], chunks[1]);
        if let Some(prompt) = prompt.clone() {
            f.render_widget(prompt, chunks[2]);
            // prompt_cursor = chunks[2];
        }
    })?;
    let cur_pos = editors[editor_index].get_rel_cursor();

    terminal.set_cursor(cur_pos.0, cur_pos.1)?;
    terminal.show_cursor()?;

    loop {
        e = editors.get_mut(editor_index).unwrap();

        let event = read()?;
        match event {
            Event::Resize(width, height) => {
                #[cfg(target_family = "windows")]
                terminal.resize(tui::layout::Rect {
                    x: 0,
                    y: 0,
                    width,
                    height,
                })?;
                #[cfg(target_family = "unix")]
                terminal.resize(tui::layout::Rect {
                    x: 0,
                    y: 0,
                    width: width - 1,
                    height: height - 1,
                })?;
            }
            Event::Mouse(event) => match event.kind {
                MouseEventKind::ScrollDown => {
                    e.move_cursor(
                        Movement::Relative(0, 1),
                        event.modifiers.intersects(KeyModifiers::SHIFT),
                    );
                }
                MouseEventKind::ScrollUp => {
                    e.move_cursor(
                        Movement::Relative(0, -1),
                        event.modifiers.intersects(KeyModifiers::SHIFT),
                    );
                }
                MouseEventKind::Down(_) => {
                    let cur_pos = (event.column as usize, event.row as usize);
                    e.move_cursor(
                        Movement::AbsoluteScreen(cur_pos.0 - e.draw_area.x as usize, cur_pos.1 - e.draw_area.y as usize),
                        event.modifiers.intersects(KeyModifiers::SHIFT),
                    );
                }
                MouseEventKind::Drag(_) => {
                    let cur_pos = (event.column as usize, event.row as usize);
                    e.move_cursor(
                        Movement::AbsoluteScreen(cur_pos.0 - e.draw_area.x as usize, cur_pos.1 - e.draw_area.y as usize),
                        true,
                    );
                }
                _ => {
                    continue;
                }
            },
            Event::Key(event) => {
                let dist = if event.modifiers.intersects(KeyModifiers::CONTROL) {
                    5
                } else {
                    1
                };
                match event.code {
                    KeyCode::Char('q') if event.modifiers == KeyModifiers::CONTROL => {
                        if e.try_quit() {
                            if editors.len() == 1 {
                                break;
                            } else {
                                editors.remove(editor_index);
                                editor_index = 0;
                            }
                        }
                    }
                    KeyCode::Char('z') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            e.undo();
                        }
                    }
                    KeyCode::Char('y') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            e.redo();
                        }
                    }
                    KeyCode::Char('p') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            if editor_index == 0 {
                                editor_index = editors.len() - 1;
                            } else {
                                editor_index -= 1;
                            }
                        }
                    }
                    KeyCode::Char('n') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            if editor_index == editors.len() - 1 {
                                editor_index = 0;
                            } else {
                                editor_index += 1;
                            }
                        }
                    }
                    KeyCode::Char('b') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            editors.push(Editor::new(ps.clone()));
                            let n = editors.len() - 1;
                            e = editors.get_mut(n).unwrap();
                            e.load_theme(theme.clone());
                        }
                    }
                    KeyCode::Char('r') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            e.try_reload()?;
                        }
                    }
                    KeyCode::Char('s') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() && !e.save()? {
                            prompt = Some(Prompt::new(Some("save ".to_string())));
                        }
                    }
                    KeyCode::Char('o') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            prompt = Some(Prompt::new(Some("open ".to_string())));
                        }
                    }
                    KeyCode::Char('c') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            clipboard = Some(e.copy());
                        }
                    }
                    KeyCode::Char('x') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            clipboard = Some(e.cut());
                        }
                    }
                    KeyCode::Char('v') if event.modifiers == KeyModifiers::CONTROL => {
                        if prompt.is_none() {
                            e.paste(&clipboard);
                        }
                    }
                    KeyCode::Left => {
                        if let Some(ref mut prompt) = prompt {
                            prompt.move_cursor(-1);
                        } else {
                            e.move_cursor(
                                Movement::Relative(-dist, 0),
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::Right => {
                        if let Some(ref mut prompt) = prompt {
                            prompt.move_cursor(1);
                        } else {
                            e.move_cursor(
                                Movement::Relative(dist, 0),
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::Up => {
                        if prompt.is_none() {
                            e.move_cursor(
                                Movement::Relative(0, -dist),
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::Down => {
                        if prompt.is_none() {
                            e.move_cursor(
                                Movement::Relative(0, dist),
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::Home => {
                        if prompt.is_none() {
                            e.move_cursor(
                                Movement::Home,
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::End => {
                        if prompt.is_none() {
                            e.move_cursor(
                                Movement::End,
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::PageUp => {
                        if prompt.is_none() {
                            e.move_cursor(
                                Movement::PageUp,
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::PageDown => {
                        if prompt.is_none() {
                            e.move_cursor(
                                Movement::PageDown,
                                event.modifiers.intersects(KeyModifiers::SHIFT),
                            );
                        }
                    }
                    KeyCode::Backspace if event.modifiers == KeyModifiers::NONE => {
                        if let Some(ref mut prompt) = prompt {
                            prompt.backspace();
                        } else {
                            e.backspace_char();
                        }
                    }
                    KeyCode::Enter if event.modifiers == KeyModifiers::NONE => {
                        if prompt.is_none() {
                            e.do_return();
                        } else {
                            let response = prompt
                                .unwrap()
                                .take_answer()
                                .unwrap_or_else(|| "".to_string());
                            prompt = None;
                            let info: Vec<&str> = response.split(' ').collect();
                            match info[0] {
                                "save" => {
                                    if info.len() > 1 {
                                        e.save_as(std::path::PathBuf::from(info[1]))?;
                                    } else {
                                        e.set_message(&"Specify path to save");
                                    }
                                }
                                "open" => {
                                    if info.len() > 1 {
                                        let path = std::path::PathBuf::from(info[1]);
                                        if !path.exists() {
                                            e.set_message(&"File does not exist");
                                        } else {
                                            e.open_file(&path)?;
                                        }
                                    } else {
                                        e.set_message(&"Specify file to open");
                                    }
                                }
                                _ => {
                                    e.set_message(&format!("Command not recognized {}", info[0]));
                                }
                            }
                        }
                    }
                    KeyCode::Delete if event.modifiers == KeyModifiers::NONE => {
                        if let Some(ref mut prompt) = prompt {
                            prompt.delete_char();
                        } else {
                            e.delete_char();
                        }
                    }
                    KeyCode::Esc if event.modifiers == KeyModifiers::NONE => {
                        if prompt.is_some() {
                            let mut un_prompt = prompt.unwrap();
                            un_prompt.take_answer();
                            prompt = None;
                        }
                    }
                    KeyCode::Char(c)
                        if event.modifiers == KeyModifiers::NONE
                            || event.modifiers == KeyModifiers::SHIFT =>
                    {
                        if let Some(ref mut prompt) = prompt {
                            prompt.add_char(c);
                        } else {
                            e.write_char(c);
                        }
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }

        if prompt.is_none() {
            if let Some(prompt_message) = editors[editor_index].take_prompt() {
                prompt = Some(Prompt::new(Some(prompt_message)));
            }
        }

        let mut prompt_cursor = tui::layout::Rect::default();
        terminal.hide_cursor()?;
        terminal.draw(|f| {
            use tui::{
                layout::{Constraint, Direction, Layout},
                style::Style,
                text::Spans,
                widgets::{Block, Borders, Tabs},
            };
            let size = f.size();
            let main_block = Block::default()
                .borders(Borders::ALL)
                .style(TuiStyle::default().fg(fg_color).bg(bg_color));
            let inner_area = main_block.inner(size);
            let mut constraints = vec![Constraint::Length(1), Constraint::Min(1)];
            if prompt.is_some() {
                constraints.push(Constraint::Length(2));
            }
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(constraints)
                .split(inner_area);
            let tabs = Tabs::new(editors.iter().map(|e| Spans::from(e.get_title())).collect())
                .select(editor_index)
                .highlight_style(Style::default().fg(sel_color))
                .divider("|");
            f.render_widget(main_block, size);
            f.render_widget(tabs, chunks[0]);
            f.render_widget(&mut editors[editor_index], chunks[1]);
            if let Some(prompt) = prompt.clone() {
                f.render_widget(prompt, chunks[2]);
                prompt_cursor = chunks[2];
            }
        })?;
        let cur_pos = if let Some(prompt) = prompt.clone() {
            let cur = prompt.get_cursor();
            (prompt_cursor.x + cur.0, prompt_cursor.y + cur.1)
        } else {
            editors[editor_index].get_rel_cursor()
        };
        terminal.set_cursor(cur_pos.0, cur_pos.1)?;
        terminal.show_cursor()?;
    }

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;

    Ok(())
}

pub fn main() -> std::io::Result<()> {
    let matches = App::new("Redit")
        .version("0.1.0")
        .author("Zachary Dodge")
        .about("A simple text editor written in Rust")
        .arg(Arg::with_name("FILE"))
        .get_matches();

    if let Err(e) = edit(matches.value_of("FILE")) {
        eprintln!("{:?}", e);
    }

    Ok(())
}
