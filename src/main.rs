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

mod editor_config;

pub fn main() -> std::io::Result<()> {
    let matches = App::new("Rudit")
        .version("0.1.0")
        .author("Zachary Dodge")
        .about("A simple text editor written in Rust")
        .arg(Arg::with_name("FILE"))
        .get_matches();

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

    let initial_size = size().unwrap();
    let mut e = editor_config::EditorConfig::new(initial_size.1.into(), initial_size.0.into());

    if let Some(file) = matches.value_of("FILE") {
        e.open(&file)?;
    }

    let mut stdout = std::io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        SetBackgroundColor(background_color),
        SetForegroundColor(foreground_color)
    )
    .unwrap();

    enable_raw_mode().unwrap();

    e.draw(&mut stdout)?;
    e.move_cursor(0, 0); // Force editor_config to adjust cursor position
    let cur_pos = e.get_cursor();
    execute!(stdout, MoveTo(cur_pos.0, cur_pos.1)).unwrap();

    loop {
        let event = read().unwrap();

        match event {
            Event::Resize(width, height) => {
                e.resize(width.into(), height.into());
                execute!(
                    stdout,
                    SetBackgroundColor(background_color),
                    SetForegroundColor(foreground_color)
                )
                .unwrap();
            }
            Event::Key(event) => {
                if event.code == KeyCode::Esc {
                    break;
                } else if event.modifiers == KeyModifiers::CONTROL {
                    match event.code {
                        KeyCode::Char('c') => {
                            continue;
                        }
                        KeyCode::Char('s') => {
                            continue;
                        }
                        KeyCode::Char('z') => {
                            continue;
                        }
                        KeyCode::Char('v') => {
                            continue;
                        }
                        KeyCode::Char('m') => {
                            continue;
                        }
                        KeyCode::Char('q') => {
                            break;
                        }
                        _ => {}
                    }
                } else if event.modifiers == KeyModifiers::NONE {
                    match event.code {
                        KeyCode::Left => {
                            e.move_cursor(-1, 0);
                        }
                        KeyCode::Right => {
                            e.move_cursor(1, 0);
                        }
                        KeyCode::Up => {
                            e.move_cursor(0, -1);
                        }
                        KeyCode::Down => {
                            e.move_cursor(0, 1);
                        }
                        KeyCode::Home => {
                            e.cursor_home();
                        }
                        KeyCode::End => {
                            e.cursor_end();
                        }
                        KeyCode::Char(c) => {
                            e.handle_char(c);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        let cur_pos = e.get_cursor();
        execute!(
            stdout,
            Hide,
            MoveTo(cur_pos.0, cur_pos.1),
            SavePosition,
            Clear(ClearType::CurrentLine),
            MoveTo(0, 0)
        )
        .unwrap();
        e.draw(&mut stdout)?;
        execute!(stdout, RestorePosition, Show).unwrap();
    }

    disable_raw_mode().unwrap();
    execute!(stdout, LeaveAlternateScreen).unwrap();

    Ok(())
}
