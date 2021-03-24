use clap::{App, Arg};
use crossterm::{
    cursor::{MoveTo, RestorePosition, SavePosition},
    event::{read, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};

mod editor_config;

fn move_cursor(
    stdout: &mut std::io::Stdout,
    editor_config: &mut editor_config::EditorConfig,
    dx: i16,
    dy: i16,
) {
    let new_pos = editor_config.move_cursor(dx, dy);
    execute!(stdout, MoveTo(new_pos.0, new_pos.1)).unwrap();
}

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
    // let theme = &ThemeSet::load_defaults().themes["Solarized (light)"];

    let initial_size = size().unwrap();
    let mut e = editor_config::EditorConfig::new(initial_size.1.into(), initial_size.0.into());

    if let Some(file) = matches.value_of("FILE") {
        e.open(&file)?;
    }

    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen).unwrap();

    enable_raw_mode().unwrap();

    e.draw(&mut stdout)?;

    loop {
        let event = read().unwrap();

        match event {
            Event::Resize(width, height) => {
                e.resize(width.into(), height.into());
            }
            Event::Key(event) => {
                if event.code == KeyCode::Char('q') && event.modifiers == KeyModifiers::CONTROL {
                    break;
                }
                if event.modifiers == KeyModifiers::CONTROL {
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
                        _ => {}
                    }
                }
                if event.modifiers == KeyModifiers::NONE {
                    match event.code {
                        KeyCode::Left => {
                            move_cursor(&mut stdout, &mut e, -1, 0);
                        }
                        KeyCode::Right => {
                            move_cursor(&mut stdout, &mut e, 1, 0);
                        }
                        KeyCode::Up => {
                            move_cursor(&mut stdout, &mut e, 0, -1);
                        }
                        KeyCode::Down => {
                            move_cursor(&mut stdout, &mut e, 0, 1);
                        }
                        _ => {}
                    }
                }
                if event.code == KeyCode::Esc {
                    break;
                }
            }
            _ => {}
        }

        execute!(
            stdout,
            SavePosition,
            Clear(ClearType::CurrentLine),
            MoveTo(0, 0)
        )
        .unwrap();
        e.draw(&mut stdout)?;
        execute!(stdout, RestorePosition).unwrap();
    }

    disable_raw_mode().unwrap();
    execute!(stdout, LeaveAlternateScreen).unwrap();

    Ok(())
}
