use std::path::PathBuf;

use clap::{App, Arg};
use crossterm::{
    event::{read, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, EnterAlternateScreen, LeaveAlternateScreen,
    },
    ExecutableCommand,
};
use dirs::home_dir;
use syntect::{highlighting::ThemeSet, parsing::SyntaxSet};
use tui::{backend::CrosstermBackend, Terminal};

use redit::editor::{Editor, Movement};

fn edit(file: Option<&str>) -> crossterm::Result<()> {
    let mut ps = SyntaxSet::load_defaults_newlines().into_builder();
    let config_dir = home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".config/redit");
    let syntax_dir = config_dir.join("syntaxes");
    if syntax_dir.exists() && ps.add_from_folder(syntax_dir, true).is_err() {
        eprintln!("Couldn't load syntaxes");
    }
    let ps = ps.build();
    let theme = &ThemeSet::load_defaults().themes["Solarized (dark)"];

    #[cfg(target_family = "windows")]
    let initial_size = size()?;
    #[cfg(target_family = "unix")]
    let initial_size = {
        let size = size()?;
        (size.0 - 1, size.1 - 1)
    };
    let mut editors = vec![Editor::new(
        initial_size.1.into(),
        initial_size.0.into(),
        ps.clone(),
    )];
    let mut editor_index = 0;
    let mut e = editors.get_mut(editor_index).unwrap();
    e.load_theme(theme.clone());
    if let Some(file) = file {
        e.open_file(&file)?
    };

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut clipboard = None;

    let cur_pos = e.get_rel_cursor();
    terminal.draw(|f| {
        let size = f.size();
        f.render_widget(e, size);
    })?;
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
                    e.move_cursor(
                        Movement::AbsoluteScreen(event.column as usize, event.row as usize),
                        event.modifiers.intersects(KeyModifiers::SHIFT),
                    );
                }
                MouseEventKind::Drag(_) => {
                    e.move_cursor(
                        Movement::AbsoluteScreen(event.column as usize, event.row as usize),
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
                                e = editors.get_mut(editor_index).unwrap();
                            }
                        }
                    }
                    KeyCode::Char('z') if event.modifiers == KeyModifiers::CONTROL => {
                        e.undo();
                    }
                    KeyCode::Char('y') if event.modifiers == KeyModifiers::CONTROL => {
                        e.redo();
                    }
                    KeyCode::Char('[') => {
                        if editor_index == 0 {
                            editor_index = editors.len() - 1;
                        } else {
                            editor_index -= 1;
                        }
                        e = editors.get_mut(editor_index).unwrap();
                    }
                    KeyCode::Char(']') => {
                        if editor_index == editors.len() - 1 {
                            editor_index = 0;
                        } else {
                            editor_index += 1;
                        }
                        e = editors.get_mut(editor_index).unwrap();
                    }
                    KeyCode::Char('\\') => {
                        let size = size()?;
                        editors.push(Editor::new(size.1.into(), size.0.into(), ps.clone()));
                        let n = editors.len() - 1;
                        e = editors.get_mut(n).unwrap();
                        e.load_theme(theme.clone());
                    }
                    KeyCode::Char('r') if event.modifiers == KeyModifiers::CONTROL => {
                        e.try_reload()?;
                    }
                    KeyCode::Char('s') if event.modifiers == KeyModifiers::CONTROL => {
                        e.save()?;
                    }
                    KeyCode::Char('o') if event.modifiers == KeyModifiers::CONTROL => {
                        e.open();
                    }
                    KeyCode::Char('c') if event.modifiers == KeyModifiers::CONTROL => {
                        clipboard = Some(e.copy());
                    }
                    KeyCode::Char('x') if event.modifiers == KeyModifiers::CONTROL => {
                        clipboard = Some(e.cut());
                    }
                    KeyCode::Char('v') if event.modifiers == KeyModifiers::CONTROL => {
                        e.paste(&clipboard);
                    }
                    KeyCode::Left => {
                        e.move_cursor(
                            Movement::Relative(-dist, 0),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Right => {
                        e.move_cursor(
                            Movement::Relative(dist, 0),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Up => {
                        e.move_cursor(
                            Movement::Relative(0, -dist),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Down => {
                        e.move_cursor(
                            Movement::Relative(0, dist),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Home => {
                        e.move_cursor(
                            Movement::Home,
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::End => {
                        e.move_cursor(
                            Movement::End,
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::PageUp => {
                        e.move_cursor(
                            Movement::PageUp,
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::PageDown => {
                        e.move_cursor(
                            Movement::PageDown,
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Backspace if event.modifiers == KeyModifiers::NONE => {
                        e.backspace_char();
                    }
                    KeyCode::Enter if event.modifiers == KeyModifiers::NONE => {
                        e.do_return();
                    }
                    KeyCode::Delete if event.modifiers == KeyModifiers::NONE => {
                        e.delete_char();
                    }
                    KeyCode::Esc if event.modifiers == KeyModifiers::NONE => {
                        e.cancel_prompt();
                    }
                    KeyCode::Char(c)
                        if event.modifiers == KeyModifiers::NONE
                            || event.modifiers == KeyModifiers::SHIFT =>
                    {
                        e.write_char(c);
                    }
                    _ => {
                        continue;
                    }
                }
            }
        }

        let cur_pos = e.get_rel_cursor();
        terminal.draw(|f| {
            let size = f.size();
            f.render_widget(e, size);
        })?;
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
        eprintln!("{}", e);
    }

    Ok(())
}
