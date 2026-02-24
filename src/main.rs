mod editor;
mod scripting;

use std::env;
use std::io;
use std::path::PathBuf;
use crate::editor::Editor;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("Hephaestus - A simple text editor");
        println!();
        println!("Usage:");
        println!("  ./Hephaestus [filename]    Open a file for editing");
        println!("  ./Hephaestus --help        Show this help message");
        println!();
        println!("Controls:");
        println!("  Ctrl+S    Save");
        println!("  Ctrl+C    Quit (with confirmation)");
        println!("  Ctrl+Z    Undo");
        println!("  Ctrl+Y    Redo");
        println!("  Arrows    Navigate");
        return Ok(());
    }

    let filename = if args.len() > 1 {
        Some(PathBuf::from(&args[1]))
    } else {
        None
    };

    let mut editor = Editor::new(filename)?;
    editor.run()
}
