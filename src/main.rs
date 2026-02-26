mod editor;
mod scripting;
mod project;

use std::env;
use std::io;
use std::path::PathBuf;
use crate::editor::Editor;
use crate::project::{ProjectRegistry, Project};

fn print_help() {
    println!("heph - A simple text editor");
    println!();
    println!("Usage:");
    println!("   [filename]        Open a file for editing");
    println!("   -p , --project <name>            Open a project");
    println!("   -p , --project new <name> [type] Create a new project in current folder with optional type");
    println!("   -p , --project menu              Open project selection menu");
    println!("   -p , --project list [clear]      List all projects or clear the registry");
    println!("   -p , --project rem <name>        Remove project from registry");
    println!("   -h , --help                      Show this help message");
    println!("   -v , --version                   Show version");
    println!("   -typeh [type]                    Show help for a specific project type");
    println!();
    println!("Parameters called when -p | --project new/add is used before");
    println!("   -type [type]                     Declares project type, based on .lua scripts in /scripts/pinits/pinits.lua ,");
    println!("                                    default (pre-defined) project types are: C++, C, Java, Python, Rust.");
    println!("                                    If no type is provided, the project will be created but with type Unknown.");
    println!("   -param [[param1],...,[param2]]   Inicialize project types outside default project settings (more to learn using -typeh)                             ");
    println!("   -git                             Creates fresh github repository together with the project.");
    println!("   -git-clone [url]                 Clone git repository into current folder and creates project.");
    println!();
    println!("Controls:");
    println!("  Ctrl+S    Save");
    println!("  Ctrl+C    Quit ");
    println!("  Ctrl+Z    Undo");
    println!("  Ctrl+Y    Redo");
    println!("  Arrows    Navigate");
}

fn handle_project_new(args: &[String], i: &mut usize, registry: &ProjectRegistry) -> io::Result<PathBuf> {
    if *i + 2 < args.len() {
        let name = &args[*i + 2];
        let mut project_type = None;

        // Check for -type or --type
        let mut j = *i + 3;
        while j < args.len() {
            if args[j] == "-type" || args[j] == "--type" {
                if j + 1 < args.len() {
                    project_type = Some(&args[j + 1]);
                    j += 2;
                } else {
                    eprintln!("Error: -type requires an argument");
                    std::process::exit(1);
                }
            } else if args[j].starts_with("-") {
                // Some other flag we don't know, skip for now
                j += 1;
            } else {
                // If no flag, it might be the positional type
                if project_type.is_none() {
                    project_type = Some(&args[j]);
                    j += 1;
                } else {
                    break;
                }
            }
        }

        let root = env::current_dir()?.join(name);
        std::fs::create_dir_all(&root)?;
        registry.remember(name.clone(), root.clone(), project_type.map(|s| s.to_string()))?;

        // Create .anvil directory and contents
        let project = Project::new(root.clone(), project_type.map(|s| s.to_string()));
        project.write_anvil()?;

        println!("Created project '{}' at {:?}", name, root);

        // Handle project initialization via Lua if type is provided
        if let Some(p_type) = project_type {
            let mut editor = Editor::new(None, Some(root.clone()))?;
            editor.init_lua().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            if !editor.init_project(p_type)? {
                eprintln!("Error: Unknown project type '{}'", p_type);
                // Clean up created directory if initialization failed
                let _ = std::fs::remove_dir_all(&root);
                // Also remove from registry
                registry.remove(name)?;
                std::process::exit(1);
            }
        }

        *i = j;
        Ok(root)
    } else {
        eprintln!("Error: --project new requires a project name");
        std::process::exit(1);
    }
}

fn handle_project_list(args: &[String], i: usize, registry: &ProjectRegistry) -> io::Result<()> {
    if i + 2 < args.len() && args[i + 2] == "clear" {
        registry.clear()?;
        println!("Project registry cleared.");
        return Ok(());
    }
    let entries = registry.list()?;
    if entries.is_empty() {
        println!("No projects registered.");
        return Ok(());
    }
    println!("Registered Projects:");
    for e in entries {
        let type_str = e.project_type.as_deref().unwrap_or("unknown");
        println!("  {} [{}] -> {:?} (last opened: {})", e.name, type_str, e.root, e.last_opened);
    }
    Ok(())
}

fn handle_project_remove(args: &[String], i: usize, registry: &ProjectRegistry) -> io::Result<()> {
    if i + 2 < args.len() {
        let name = &args[i + 2];
        registry.remove(name)?;
        println!("Removed project '{}' from registry", name);
        Ok(())
    } else {
        eprintln!("Error: --project rem requires a project name");
        std::process::exit(1);
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_help();
        return Ok(());
    }

    let mut filename = None;
    let mut project_root = None;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "-p" || arg == "--project" {
            if i + 1 < args.len() {
                let registry = ProjectRegistry::new();
                match args[i+1].as_str() {
                    "new" => {
                        project_root = Some(handle_project_new(&args, &mut i, &registry)?);
                        continue;
                    }
                    "menu" => {
                        let mut editor = Editor::new(None, None)?;
                        editor.run_project_menu()?;
                        return Ok(());
                    }
                    "list" => {
                        handle_project_list(&args, i, &registry)?;
                        return Ok(());
                    }
                    "rem" | "remove" => {
                        handle_project_remove(&args, i, &registry)?;
                        return Ok(());
                    }
                    _ => {
                        project_root = Some(registry.resolve(&args[i+1])?);
                        i += 2;
                        continue;
                    }
                }
            } else {
                eprintln!("Error: --project requires a project name or path");
                std::process::exit(1);
            }
        } else if arg == "-h" || arg == "--help" {
            i += 1;
        } else {
            let path = PathBuf::from(arg);
            if path.exists() {
                filename = Some(path);
                i += 1;
            } else {
                eprintln!("File not found {}", arg);
                std::process::exit(1);
            }
        }
    }

    let mut editor = Editor::new(filename, project_root)?;
    editor.run()
}