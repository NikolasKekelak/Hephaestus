mod editor;
mod scripting;
mod project;

use std::env;
use std::io;
use std::path::PathBuf;
use crate::editor::Editor;
use crate::project::ProjectRegistry;

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("heph - A simple text editor");
        println!();
        println!("Usage:");
        println!("   [filename]        Open a file for editing");
        println!("   -p <name>         Open a project");
        println!("   -p new <name>     Create a new project in current folder");
        println!("   -p list           List all projects");
        println!("   -p rem <name>     Remove project from registry");
        println!("   -h  | --help            Show this help message");
        println!();
        println!("Controls:");
        println!("  Ctrl+S    Save");
        println!("  Ctrl+C    Quit ");
        println!("  Ctrl+Z    Undo");
        println!("  Ctrl+Y    Redo");
        println!("  Arrows    Navigate");
        return Ok(());
    }

    let mut filename = None;
    let mut project_root = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-p" | "--project" => {
                if i + 1 < args.len() {
                    let registry = ProjectRegistry::new();
                    match args[i+1].as_str() {
                        "new" => {
                            if i + 2 < args.len() {
                                let name = &args[i+2];
                                let root = env::current_dir()?.join(name);
                                std::fs::create_dir_all(&root)?;
                                registry.remember(name.clone(), root.clone())?;
                                println!("Created project '{}' at {:?}", name, root);
                                project_root = Some(root);
                                i += 3;
                            } else {
                                eprintln!("Error: --project new requires a project name");
                                std::process::exit(1);
                            }
                        }
                        "list" => {
                            let entries = registry.list()?;
                            println!("Registered Projects:");
                            for e in entries {
                                println!("  {} -> {:?} (last opened: {})", e.name, e.root, e.last_opened);
                            }
                            return Ok(());
                        }
                        "rem" | "remove" => {
                            if i + 2 < args.len() {
                                let name = &args[i+2];
                                registry.remove(name)?;
                                println!("Removed project '{}' from registry", name);
                                return Ok(());
                            } else {
                                eprintln!("Error: --project rem requires a project name");
                                std::process::exit(1);
                            }
                        }
                        _ => {
                            project_root = Some(registry.resolve(&args[i+1])?);
                            i += 2;
                        }
                    }
                } else {
                    eprintln!("Error: --project requires a project name or path");
                    std::process::exit(1);
                }
            }
            _ => {
                let path = PathBuf::from(&args[i]);
                if path.exists() {
                    filename = Some(path);
                } else {
                    eprintln!("Error: File not found: {}", args[i]);
                    std::process::exit(1);
                }
                i += 1;
            }
        }
    }

    let mut editor = Editor::new(filename, project_root)?;
    editor.run()
}
