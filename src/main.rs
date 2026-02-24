use std::collections::HashMap;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use mlua::prelude::*;

struct EditorState {
    filename: Option<PathBuf>,
    lines: Vec<String>,
    cursor_x: usize,
    cursor_y: usize,
    status_message: String,
    key_mappings: HashMap<String, String>,
    should_quit: bool,
    undo_stack: Vec<(Vec<String>, usize, usize)>,
    redo_stack: Vec<(Vec<String>, usize, usize)>,
    is_dirty: bool,
    row_off: usize,
}

struct Editor {
    state: Arc<Mutex<EditorState>>,
    lua: Lua,
}

impl Editor {
    fn new(filename: Option<PathBuf>) -> io::Result<Self> {
        let mut lines = Vec::new();
        if let Some(ref path) = filename {
            if path.exists() {
                let content = fs::read_to_string(path)?;
                lines = content.lines().map(|s| s.to_string()).collect();
            }
        }
        if lines.is_empty() {
            lines.push(String::new());
        }

        let state = Arc::new(Mutex::new(EditorState {
            filename,
            lines,
            cursor_x: 0,
            cursor_y: 0,
            status_message: String::from("Welcome to Hephaestus! Ctrl+S: save, Ctrl+C: quit"),
            key_mappings: HashMap::new(),
            should_quit: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            is_dirty: false,
            row_off: 0,
        }));

        let lua = Lua::new();

        Ok(Self { state, lua })
    }

    fn init_lua(&self) -> LuaResult<()> {
        let state = self.state.clone();
        let get_cursor = self.lua.create_function(move |_, (): ()| {
            let s = state.lock().unwrap();
            Ok((s.cursor_x, s.cursor_y))
        })?;

        let state_clone = self.state.clone();
        let set_cursor = self.lua.create_function(move |_, (x, y): (usize, usize)| {
            let mut s = state_clone.lock().unwrap();
            if s.lines.is_empty() {
                s.lines.push(String::new());
            }
            s.cursor_y = y.min(s.lines.len() - 1);
            let y = s.cursor_y;
            s.cursor_x = x.min(s.lines[y].len());
            Ok(())
        })?;

        let state_clone = self.state.clone();
        let insert_text = self.lua.create_function(move |_, text: String| {
            let mut s = state_clone.lock().unwrap();
            if s.lines.is_empty() {
                s.lines.push(String::new());
            }
            for c in text.chars() {
                let y = s.cursor_y;
                let x = s.cursor_x;
                if c == '\n' {
                    let new_line = s.lines[y].split_off(x);
                    s.lines.insert(y + 1, new_line);
                    s.cursor_y += 1;
                    s.cursor_x = 0;
                } else {
                    s.lines[y].insert(x, c);
                    s.cursor_x += 1;
                }
            }
            Ok(())
        })?;

        let globals = self.lua.globals();
        let editor_api = self.lua.create_table()?;
        editor_api.set("get_cursor", get_cursor)?;
        editor_api.set("set_cursor", set_cursor)?;
        editor_api.set("insert_text", insert_text)?;
        
        let state_clone = self.state.clone();
        let print_msg = self.lua.create_function(move |_, text: String| {
            let mut s = state_clone.lock().unwrap();
            s.status_message = text;
            Ok(())
        })?;
        editor_api.set("print", print_msg)?;

        let state_clone = self.state.clone();
        let set_keymap = self.lua.create_function(move |_, (key, func_name): (String, String)| {
            let mut s = state_clone.lock().unwrap();
            s.key_mappings.insert(key, func_name);
            Ok(())
        })?;
        editor_api.set("set_keymap", set_keymap)?;

        let state_clone = self.state.clone();
        let lua_undo = self.lua.create_function(move |_, (): ()| {
            let mut s = state_clone.lock().unwrap();
            if let Some((lines, x, y)) = s.undo_stack.pop() {
                let current_state = (s.lines.clone(), s.cursor_x, s.cursor_y);
                s.redo_stack.push(current_state);
                s.lines = lines;
                s.cursor_x = x;
                s.cursor_y = y;
                s.status_message = String::from("Undo");
            }
            Ok(())
        })?;
        editor_api.set("undo", lua_undo)?;

        let state_clone = self.state.clone();
        let lua_redo = self.lua.create_function(move |_, (): ()| {
            let mut s = state_clone.lock().unwrap();
            if let Some((lines, x, y)) = s.redo_stack.pop() {
                let current_state = (s.lines.clone(), s.cursor_x, s.cursor_y);
                s.undo_stack.push(current_state);
                s.lines = lines;
                s.cursor_x = x;
                s.cursor_y = y;
                s.status_message = String::from("Redo");
            }
            Ok(())
        })?;
        editor_api.set("redo", lua_redo)?;

        let state_clone = self.state.clone();
        let lua_save = self.lua.create_function(move |_, (): ()| {
            let mut s = state_clone.lock().unwrap();
            if let Some(ref path) = s.filename {
                let content = s.lines.join("\n");
                let _ = fs::write(path, content);
                s.is_dirty = false;
                s.status_message = String::from("Saved!");
            } else {
                s.status_message = String::from("No filename!");
            }
            Ok(())
        })?;
        editor_api.set("save", lua_save)?;

        let state_clone = self.state.clone();
        let lua_quit = self.lua.create_function(move |_, (): ()| {
            let mut s = state_clone.lock().unwrap();
            s.should_quit = true;
            Ok(())
        })?;
        editor_api.set("quit", lua_quit)?;

        globals.set("editor", editor_api)?;

        // Load init.lua if it exists
        if let Ok(content) = fs::read_to_string("init.lua") {
            self.lua.load(&content).exec()?;
        }

        Ok(())
    }

    fn run(&mut self) -> io::Result<()> {
        self.init_lua().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, cursor::Hide, event::EnableMouseCapture)?;

        loop {
            self.draw(&mut stdout)?;
            self.process_event()?;
            let should_quit = self.state.lock().unwrap().should_quit;
            if should_quit {
                break;
            }
        }

        execute!(stdout, cursor::Show, LeaveAlternateScreen, event::DisableMouseCapture)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn draw(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        let mut s = self.state.lock().unwrap();
        let (cols, rows) = terminal::size()?;
        
        // Adjust row_off to keep cursor visible
        if s.cursor_y < s.row_off {
            s.row_off = s.cursor_y;
        }
        if s.cursor_y >= s.row_off + (rows - 1) as usize {
            s.row_off = s.cursor_y - (rows - 1) as usize + 1;
        }

        execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        // Draw text lines (leave room for status bar)
        for i in 0..(rows - 1) as usize {
            let file_row = i + s.row_off;
            if let Some(line) = s.lines.get(file_row) {
                execute!(stdout, cursor::MoveTo(0, i as u16))?;
                // Truncate line if it's longer than screen width
                let truncated = if line.len() > cols as usize {
                    &line[..cols as usize]
                } else {
                    line
                };
                write!(stdout, "{}", truncated)?;
            }
        }

        // Draw status line
        let filename = s.filename.as_deref()
            .and_then(|p| p.to_str())
            .unwrap_or("[No Name]");
        let status = format!(" {} | {}", filename, s.status_message);
        let status = if status.len() > cols as usize {
            &status[..cols as usize]
        } else {
            &status
        };

        execute!(
            stdout,
            cursor::MoveTo(0, rows - 1),
            terminal::Clear(terminal::ClearType::CurrentLine),
            crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse)
        )?;
        write!(stdout, "{:<width$}", status, width = cols as usize)?;
        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset))?;

        execute!(stdout, cursor::MoveTo(s.cursor_x as u16, (s.cursor_y - s.row_off) as u16), cursor::Show)?;
        stdout.flush()?;
        Ok(())
    }

    fn process_event(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    let mut s = self.state.lock().unwrap();
                    
                    // Handle Lua key mappings
                    let mut key_str = String::new();
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        key_str.push_str("C-");
                    }
                    if key.modifiers.contains(KeyModifiers::ALT) {
                        key_str.push_str("A-");
                    }
                    match key.code {
                        KeyCode::Char(c) => key_str.push(c),
                        _ => {} // Handle other keys if needed
                    }
                    
                    if !key_str.is_empty() {
                        if let Some(func_name) = s.key_mappings.get(&key_str).cloned() {
                            drop(s); // Release lock before calling Lua
                            let globals = self.lua.globals();
                            if let Ok(func) = globals.get::<_, LuaFunction>(func_name) {
                                let _ = func.call::<_, ()>(());
                            }
                            return Ok(());
                        }
                    }

                    match key.code {
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if s.is_dirty {
                                s.status_message = String::from("Quit? (y: quit, s: save&quit, n: cancel)");
                                drop(s);
                                self.draw(&mut io::stdout())?;
                                loop {
                                    if event::poll(std::time::Duration::from_millis(100))? {
                                        if let Event::Key(k) = event::read()? {
                                            let mut s = self.state.lock().unwrap();
                                            match k.code {
                                                KeyCode::Char('y') => {
                                                    s.should_quit = true;
                                                    break;
                                                }
                                                KeyCode::Char('s') => {
                                                    self.save_state(s)?;
                                                    let mut s = self.state.lock().unwrap();
                                                    s.should_quit = true;
                                                    break;
                                                }
                                                KeyCode::Char('n') | KeyCode::Esc => {
                                                    s.status_message = String::from("Canceled quit");
                                                    break;
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            } else {
                                s.should_quit = true;
                            }
                        }
                        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.save_state(s)?;
                        }
                        KeyCode::Char('z') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.undo(&mut s);
                        }
                        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            self.redo(&mut s);
                        }
                        KeyCode::Up => {
                            if s.cursor_y > 0 {
                                s.cursor_y -= 1;
                                s.cursor_x = s.cursor_x.min(s.lines[s.cursor_y].len());
                            }
                        }
                        KeyCode::Down => {
                            if s.cursor_y < s.lines.len() - 1 {
                                s.cursor_y += 1;
                                s.cursor_x = s.cursor_x.min(s.lines[s.cursor_y].len());
                            }
                        }
                        KeyCode::PageUp => {
                            let (_, rows) = terminal::size()?;
                            let rows = (rows - 1) as usize;
                            if s.cursor_y > rows {
                                s.cursor_y -= rows;
                            } else {
                                s.cursor_y = 0;
                            }
                            s.cursor_x = s.cursor_x.min(s.lines[s.cursor_y].len());
                        }
                        KeyCode::PageDown => {
                            let (_, rows) = terminal::size()?;
                            let rows = (rows - 1) as usize;
                            s.cursor_y += rows;
                            if s.cursor_y >= s.lines.len() {
                                s.cursor_y = s.lines.len() - 1;
                            }
                            s.cursor_x = s.cursor_x.min(s.lines[s.cursor_y].len());
                        }
                        KeyCode::Left => {
                            if s.cursor_x > 0 {
                                s.cursor_x -= 1;
                            } else if s.cursor_y > 0 {
                                s.cursor_y -= 1;
                                s.cursor_x = s.lines[s.cursor_y].len();
                            }
                        }
                        KeyCode::Right => {
                            if s.cursor_x < s.lines[s.cursor_y].len() {
                                s.cursor_x += 1;
                            } else if s.cursor_y < s.lines.len() - 1 {
                                s.cursor_y += 1;
                                s.cursor_x = 0;
                            }
                        }
                        KeyCode::Char(c) => {
                            self.push_undo(&mut s);
                            let y = s.cursor_y;
                            let x = s.cursor_x;
                            s.lines[y].insert(x, c);
                            s.cursor_x += 1;
                        }
                        KeyCode::Enter => {
                            self.push_undo(&mut s);
                            let y = s.cursor_y;
                            let x = s.cursor_x;
                            let new_line = s.lines[y].split_off(x);
                            s.lines.insert(y + 1, new_line);
                            s.cursor_y += 1;
                            s.cursor_x = 0;
                        }
                        KeyCode::Backspace => {
                            let y = s.cursor_y;
                            let x = s.cursor_x;
                            if x > 0 || y > 0 {
                                self.push_undo(&mut s);
                            }
                            
                            let y = s.cursor_y;
                            let x = s.cursor_x;
                            if x > 0 {
                                s.lines[y].remove(x - 1);
                                s.cursor_x -= 1;
                            } else if y > 0 {
                                let current_line = s.lines.remove(y);
                                s.cursor_y -= 1;
                                let prev_y = s.cursor_y;
                                s.cursor_x = s.lines[prev_y].len();
                                s.lines[prev_y].push_str(&current_line);
                            }
                        }
                        _ => {}
                    }
                }
                Event::Mouse(mouse_event) => {
                    let mut s = self.state.lock().unwrap();
                    match mouse_event.kind {
                        event::MouseEventKind::ScrollUp => {
                            if s.row_off > 0 {
                                s.row_off -= 1;
                                // Keep cursor in view if it was at the bottom
                                let (_, rows) = terminal::size()?;
                                let visible_rows = (rows - 1) as usize;
                                if s.cursor_y >= s.row_off + visible_rows {
                                    s.cursor_y = s.row_off + visible_rows - 1;
                                    s.cursor_x = s.cursor_x.min(s.lines[s.cursor_y].len());
                                }
                            }
                        }
                        event::MouseEventKind::ScrollDown => {
                            if s.row_off + 1 < s.lines.len() {
                                s.row_off += 1;
                                // Keep cursor in view if it was at the top
                                if s.cursor_y < s.row_off {
                                    s.cursor_y = s.row_off;
                                    s.cursor_x = s.cursor_x.min(s.lines[s.cursor_y].len());
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn save_state(&self, mut s: std::sync::MutexGuard<EditorState>) -> io::Result<()> {
        if let Some(ref path) = s.filename {
            let content = s.lines.join("\n");
            fs::write(path, content)?;
            s.is_dirty = false;
            s.status_message = String::from("Saved!");
        } else {
            s.status_message = String::from("No filename!");
        }
        Ok(())
    }

    fn push_undo(&self, s: &mut EditorState) {
        let current_state = (s.lines.clone(), s.cursor_x, s.cursor_y);
        s.undo_stack.push(current_state);
        if s.undo_stack.len() > 100 {
            s.undo_stack.remove(0);
        }
        s.redo_stack.clear();
        s.is_dirty = true;
    }

    fn undo(&self, s: &mut EditorState) {
        if let Some((lines, x, y)) = s.undo_stack.pop() {
            let current_state = (s.lines.clone(), s.cursor_x, s.cursor_y);
            s.redo_stack.push(current_state);
            s.lines = lines;
            s.cursor_x = x;
            s.cursor_y = y;
            s.status_message = String::from("Undo");
        }
    }

    fn redo(&self, s: &mut EditorState) {
        if let Some((lines, x, y)) = s.redo_stack.pop() {
            let current_state = (s.lines.clone(), s.cursor_x, s.cursor_y);
            s.undo_stack.push(current_state);
            s.lines = lines;
            s.cursor_x = x;
            s.cursor_y = y;
            s.status_message = String::from("Redo");
        }
    }
}

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
