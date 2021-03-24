use clap::{App, Arg};
use crossterm::{
    cursor::MoveTo,
    event::{read, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};

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
                if event.code == KeyCode::Char('c') && event.modifiers == KeyModifiers::CONTROL {
                    continue;
                }
                if event.code == KeyCode::Char('q') && event.modifiers == KeyModifiers::CONTROL {
                    break;
                }
                if event.code == KeyCode::Esc {
                    break;
                }
            }
            _ => {}
        }

        execute!(stdout, Clear(ClearType::CurrentLine), MoveTo(0, 0)).unwrap();
        e.draw(&mut stdout)?;
    }

    disable_raw_mode().unwrap();
    execute!(stdout, LeaveAlternateScreen).unwrap();

    Ok(())
}
