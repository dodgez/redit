use clap::{App, Arg};

mod editor_config;

pub fn main() -> std::io::Result<()> {
    stderrlog::new()
        .module(module_path!())
        .show_module_names(true)
        .init()
        .expect("Failed to initialize stderrlog");

    let matches = App::new("Rudit")
        .version("0.1.0")
        .author("Zachary Dodge")
        .about("A simple text editor written in Rust")
        .arg(Arg::with_name("FILE"))
        .get_matches();

    // let mut ps = SyntaxSet::load_defaults_newlines().into_builder();
    // if ps
    //     .add_from_folder(std::path::Path::new("syntaxes"), true)
    //     .is_err()
    // {
    //     warn!("Failed to load syntax folder at ./syntaxes");
    // }
    // let ps = ps.build();

    // let theme = &ThemeSet::load_defaults().themes["Solarized (light)"];

    let mut e = editor_config::EditorConfig::new(25, 80);

    if let Some(file) = matches.value_of("FILE") {
        e.open(&file)?;
    }

    e.draw(&mut std::io::stdout())?;

    Ok(())
}
