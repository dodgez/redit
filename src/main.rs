use clap::{App, Arg};
use crossterm::{
    cursor::{Hide, MoveTo, RestorePosition, SavePosition, Show},
    event::{read, Event, KeyCode, KeyModifiers},
    execute,
    style::{Color, SetBackgroundColor, SetForegroundColor},
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use syntect::highlighting::{Color as SynColor, ThemeSet};

mod editor;

fn edit(file: Option<&str>) -> crossterm::Result<()> {
    // let mut ps = SyntaxSet::load_defaults_newlines().into_builder();
    // ps.add_from_folder(std::path::Path::new("syntaxes"), true).unwrap();
    // let ps = ps.build();
    let theme = &ThemeSet::load_defaults().themes["Solarized (dark)"];
    let background_color = theme.settings.background.unwrap_or(SynColor::BLACK);
    let background_color = Color::Rgb {
        r: background_color.r,
        g: background_color.g,
        b: background_color.b,
    };
    let foreground_color = theme.settings.foreground.unwrap_or(SynColor::WHITE);
    let foreground_color = Color::Rgb {
        r: foreground_color.r,
        g: foreground_color.g,
        b: foreground_color.b,
    };

    let initial_size = size()?;
    let mut editors = vec![editor::Editor::new(
        initial_size.1.into(),
        initial_size.0.into(),
    )];
    let mut editor_index = 0;
    let mut e = editors.get_mut(editor_index).unwrap();
    if let Some(file) = file {
        e.open_file(&file)?
    };

    let mut stdout = std::io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        SetBackgroundColor(background_color),
        SetForegroundColor(foreground_color)
    )?;

    enable_raw_mode()?;

    e.draw(&mut stdout)?;
    e.move_cursor(editor::Movement::BegFile, false);
    let cur_pos = e.get_rel_cursor();
    execute!(stdout, MoveTo(cur_pos.0, cur_pos.1))?;

    loop {
        let event = read()?;
        e = editors.get_mut(editor_index).unwrap();

        match event {
            Event::Resize(width, height) => {
                execute!(stdout, Clear(ClearType::All))?;
                e.resize(width.into(), height.into());
                execute!(
                    stdout,
                    SetBackgroundColor(background_color),
                    SetForegroundColor(foreground_color)
                )?;
            }
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
                        editors.push(editor::Editor::new(size.1.into(), size.0.into()));
                        let n = editors.len() - 1;
                        e = editors.get_mut(n).unwrap();
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
                    KeyCode::Left => {
                        e.move_cursor(
                            editor::Movement::Relative(-dist, 0),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Right => {
                        e.move_cursor(
                            editor::Movement::Relative(dist, 0),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Up => {
                        e.move_cursor(
                            editor::Movement::Relative(0, -dist),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Down => {
                        e.move_cursor(
                            editor::Movement::Relative(0, dist),
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::Home => {
                        e.move_cursor(
                            editor::Movement::Home,
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::End => {
                        e.move_cursor(
                            editor::Movement::End,
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::PageUp => {
                        e.move_cursor(
                            editor::Movement::PageUp,
                            event.modifiers.intersects(KeyModifiers::SHIFT),
                        );
                    }
                    KeyCode::PageDown => {
                        e.move_cursor(
                            editor::Movement::PageDown,
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
            _ => {}
        }

        let cur_pos = e.get_rel_cursor();
        execute!(
            stdout,
            Hide,
            MoveTo(cur_pos.0, cur_pos.1),
            SavePosition,
            Clear(ClearType::CurrentLine),
            MoveTo(0, 0)
        )?;
        e.draw(&mut stdout)?;
        execute!(stdout, RestorePosition, Show)?;
    }

    disable_raw_mode()?;
    execute!(stdout, LeaveAlternateScreen)?;

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
