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
    let mut e = editor::Editor::new(initial_size.1.into(), initial_size.0.into());
    if let Some(file) = file {
        e.open(&file)?
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
    e.move_cursor(editor::Movement::BegFile);
    let cur_pos = e.get_rel_cursor();
    execute!(stdout, MoveTo(cur_pos.0, cur_pos.1))?;

    loop {
        let event = read()?;

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
                if event.code == KeyCode::Esc {
                    break;
                } else if event.modifiers == KeyModifiers::CONTROL {
                    match event.code {
                        KeyCode::Char('q') => {
                            if e.try_quit() {
                                break;
                            }
                        }
                        KeyCode::Char('s') => {
                            e.save()?;
                        }
                        _ => {
                            continue;
                        }
                    }
                } else if event.modifiers == KeyModifiers::SHIFT {
                    match event.code {
                        KeyCode::Char(c) => {
                            e.write_char(c);
                        }
                        _ => {
                            continue;
                        }
                    }
                } else if event.modifiers == KeyModifiers::NONE {
                    match event.code {
                        KeyCode::Left => {
                            e.move_cursor(editor::Movement::Relative(-1, 0));
                        }
                        KeyCode::Right => {
                            e.move_cursor(editor::Movement::Relative(1, 0));
                        }
                        KeyCode::Up => {
                            e.move_cursor(editor::Movement::Relative(0, -1));
                        }
                        KeyCode::Down => {
                            e.move_cursor(editor::Movement::Relative(0, 1));
                        }
                        KeyCode::Home => {
                            e.move_cursor(editor::Movement::Home);
                        }
                        KeyCode::End => {
                            e.move_cursor(editor::Movement::End);
                        }
                        KeyCode::PageUp => {
                            e.move_cursor(editor::Movement::PageUp);
                        }
                        KeyCode::PageDown => {
                            e.move_cursor(editor::Movement::PageDown);
                        }
                        KeyCode::Backspace => {
                            e.backspace_char();
                        }
                        KeyCode::Enter => {
                            e.do_return();
                        }
                        KeyCode::Delete => {
                            e.delete_char();
                        }
                        KeyCode::Char(c) => {
                            e.write_char(c);
                        }
                        _ => {
                            continue;
                        }
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
    let matches = App::new("Rudit")
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
