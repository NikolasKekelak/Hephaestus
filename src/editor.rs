use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// use crate::terminal::Terminal;

use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use mlua::prelude::*;

use crate::project::{Project, ProjectRegistry};

pub struct ExplorerItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub children: Vec<ExplorerItem>,
}

pub struct EditorState {
    pub filename: Option<PathBuf>,
    pub lines: Vec<String>,
    pub cursor_x: usize,
    pub cursor_y: usize,
    pub status_message: String,
    pub key_mappings: HashMap<String, String>,
    pub should_quit: bool,
    pub undo_stack: Vec<(Vec<String>, usize, usize)>,
    pub redo_stack: Vec<(Vec<String>, usize, usize)>,
    pub is_dirty: bool,
    pub row_off: usize,
    pub project: Option<Project>,
    pub is_explorer_visible: bool,
    pub explorer_items: Vec<ExplorerItem>,
    pub explorer_selected_index: usize,
    pub focus_on_explorer: bool,
    pub recent_files: Vec<PathBuf>,
}

pub struct Editor {
    pub state: Arc<Mutex<EditorState>>,
    pub lua: Lua,
}

impl Editor {
    pub fn new(filename: Option<PathBuf>, project_root: Option<PathBuf>) -> io::Result<Self> {
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

        let project = project_root.as_ref().map(|root| {
            let p = Project::new(root.clone());
            let registry = ProjectRegistry::new();
            let _ = registry.remember(p.name.clone(), p.root.clone());
            p
        });

        let mut explorer_items = Vec::new();
        if let Some(ref p) = project {
            explorer_items = Self::list_dir(&p.root)?;
        }

        let state = Arc::new(Mutex::new(EditorState {
            filename,
            lines,
            cursor_x: 0,
            cursor_y: 0,
            status_message: String::from("Welcome to heph! Ctrl+S: save, Ctrl+C: quit"),
            key_mappings: HashMap::new(),
            should_quit: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            is_dirty: false,
            row_off: 0,
            project,
            is_explorer_visible: project_root.is_some(),
            explorer_items,
            explorer_selected_index: 0,
            focus_on_explorer: false,
            recent_files: Vec::new(),
        }));

        let lua = Lua::new();

        Ok(Self { state, lua })
    }

    pub fn run(&mut self) -> io::Result<()> {
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
        
        let editor_height = rows.saturating_sub(1);
        let mut editor_width = cols;
        let mut explorer_width = 0;

        if s.is_explorer_visible {
            explorer_width = 30.min(cols / 3);
            editor_width = cols.saturating_sub(explorer_width);
        }

        // Adjust row_off to keep cursor visible
        if s.cursor_y < s.row_off {
            s.row_off = s.cursor_y;
        }
        if s.cursor_y >= s.row_off + editor_height as usize {
            s.row_off = s.cursor_y - editor_height as usize + 1;
        }

        execute!(stdout, cursor::MoveTo(0, 0), Clear(ClearType::All))?;

        // Draw Explorer
        if s.is_explorer_visible {
            let mut flat_items = Vec::new();
            Self::flatten_explorer(&s.explorer_items, 0, &mut flat_items);
            
            for i in 0..editor_height as usize {
                execute!(stdout, cursor::MoveTo(0, i as u16))?;
                if let Some((item, depth)) = flat_items.get(i) {
                    let prefix = if item.is_dir {
                        if item.is_expanded { "[-] " } else { "[+] " }
                    } else {
                        "  "
                    };
                    let indent = "  ".repeat(*depth);
                    let line = format!("{}{}{}", indent, prefix, item.name);
                    let truncated = if line.len() > explorer_width as usize {
                        &line[..explorer_width as usize]
                    } else {
                        &line
                    };
                    if s.focus_on_explorer && s.explorer_selected_index == i {
                        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse))?;
                        write!(stdout, "{:<width$}", truncated, width = explorer_width as usize)?;
                        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset))?;
                    } else {
                        write!(stdout, "{}", truncated)?;
                    }
                }
                // Draw vertical separator
                execute!(stdout, cursor::MoveTo(explorer_width as u16, i as u16))?;
                write!(stdout, "│")?;
            }
        }

        // Draw text lines
        for i in 0..editor_height as usize {
            let file_row = i + s.row_off;
            if let Some(line) = s.lines.get(file_row) {
                execute!(stdout, cursor::MoveTo(explorer_width + 1, i as u16))?;
                let available_width = editor_width.saturating_sub(1) as usize;
                let truncated = if line.len() > available_width {
                    &line[..available_width]
                } else {
                    line
                };
                write!(stdout, "{}", truncated)?;
            }
        }

        // Draw status line
        execute!(stdout, cursor::MoveTo(0, rows.saturating_sub(1) as u16))?;
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
            cursor::MoveTo(0, rows.saturating_sub(1)),
            terminal::Clear(terminal::ClearType::CurrentLine),
            crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse)
        )?;
        write!(stdout, "{:<width$}", status, width = cols as usize)?;
        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset))?;

        if s.focus_on_explorer {
            execute!(stdout, cursor::Hide)?;
        } else {
            execute!(stdout, cursor::MoveTo(explorer_width + 1 + s.cursor_x as u16, (s.cursor_y - s.row_off) as u16), cursor::Show)?;
        }
        stdout.flush()?;
        Ok(())
    }

    fn flatten_explorer<'a>(items: &'a [ExplorerItem], depth: usize, flat: &mut Vec<(&'a ExplorerItem, usize)>) {
        for item in items {
            flat.push((item, depth));
            if item.is_expanded {
                Self::flatten_explorer(&item.children, depth + 1, flat);
            }
        }
    }

    pub fn sync_explorer_selection(s: &mut EditorState) {
        if let Some(ref current_file) = s.filename {
            let mut flat = Vec::new();
            Self::flatten_explorer(&s.explorer_items, 0, &mut flat);
            if let Some(index) = flat.iter().position(|(item, _)| {
                // Try exact match or canonicalized match
                if item.path == *current_file {
                    return true;
                }
                if let (Ok(p1), Ok(p2)) = (fs::canonicalize(&item.path), fs::canonicalize(current_file)) {
                    return p1 == p2;
                }
                false
            }) {
                s.explorer_selected_index = index;
            }
        }
    }

    fn process_event(&mut self) -> io::Result<()> {
        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    let mut s = self.state.lock().unwrap();

                    // Handle Lua key mappings first, especially Alt-Q
                    let mut key_str = String::new();
                    if key.modifiers.contains(KeyModifiers::CONTROL) {
                        key_str.push_str("C-");
                    }
                    if key.modifiers.contains(KeyModifiers::ALT) {
                        key_str.push_str("A-");
                    }
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        key_str.push_str("S-");
                    }
                    match key.code {
                        KeyCode::Char(c) => key_str.push(c.to_ascii_lowercase()),
                        KeyCode::F(n) => key_str.push_str(&format!("F{}", n)),
                        _ => {}
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

                    if s.focus_on_explorer {
                        match key.code {
                            KeyCode::Up => {
                                if s.explorer_selected_index > 0 {
                                    s.explorer_selected_index -= 1;
                                }
                            }
                            KeyCode::Down => {
                                let mut flat = Vec::new();
                                Self::flatten_explorer(&s.explorer_items, 0, &mut flat);
                                if s.explorer_selected_index + 1 < flat.len() {
                                    s.explorer_selected_index += 1;
                                }
                            }
                            KeyCode::Enter => {
                                let mut flat = Vec::new();
                                Self::flatten_explorer(&s.explorer_items, 0, &mut flat);
                                if let Some((item, _)) = flat.get(s.explorer_selected_index) {
                                    let path = item.path.clone();
                                    if item.is_dir {
                                        let path_to_toggle = path.clone();
                                        Self::toggle_dir_recursive(&mut s.explorer_items, &path_to_toggle)?;
                                    } else {
                                        // Open file
                                        drop(s);
                                        self.open_file(path)?;
                                        let mut s = self.state.lock().unwrap();
                                        s.focus_on_explorer = false;
                                        return Ok(());
                                    }
                                }
                            }
                            KeyCode::Backspace => {
                                let mut flat = Vec::new();
                                Self::flatten_explorer(&s.explorer_items, 0, &mut flat);
                                if let Some((item, _)) = flat.get(s.explorer_selected_index) {
                                    let path_to_collapse = item.path.clone();
                                    Self::collapse_dir_recursive(&mut s.explorer_items, &path_to_collapse);
                                }
                            }
                            KeyCode::Esc => {
                                s.focus_on_explorer = false;
                            }
                            _ => {}
                        }
                        return Ok(());
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
        if let Some(path) = s.filename.clone() {
            let content = s.lines.join("\n");
            fs::write(&path, content)?;
            s.is_dirty = false;
            s.status_message = format!("Saved {}", path.display());

            // Add to recent files
            s.recent_files.retain(|f| f != &path);
            s.recent_files.insert(0, path);
            if s.recent_files.len() > 20 {
                s.recent_files.truncate(20);
            }
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

    fn open_file(&self, path: PathBuf) -> io::Result<()> {
        let content = fs::read_to_string(&path)?;
        let lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let mut s = self.state.lock().unwrap();
        s.filename = Some(path.clone());
        s.lines = if lines.is_empty() { vec![String::new()] } else { lines };
        s.cursor_x = 0;
        s.cursor_y = 0;
        s.row_off = 0;
        s.is_dirty = false;
        s.undo_stack.clear();
        s.redo_stack.clear();
        s.status_message = format!("Opened {}", path.display());
        
        // Add to recent files
        s.recent_files.retain(|f| f != &path);
        s.recent_files.insert(0, path);
        if s.recent_files.len() > 20 {
            s.recent_files.truncate(20);
        }
        
        Ok(())
    }

    fn toggle_dir_recursive(items: &mut Vec<ExplorerItem>, path: &PathBuf) -> io::Result<()> {
        for item in items {
            if item.path == *path {
                item.is_expanded = !item.is_expanded;
                if item.is_expanded && item.children.is_empty() {
                    item.children = Self::list_dir(&item.path)?;
                }
                return Ok(());
            }
            if !item.children.is_empty() {
                Self::toggle_dir_recursive(&mut item.children, path)?;
            }
        }
        Ok(())
    }

    fn collapse_dir_recursive(items: &mut Vec<ExplorerItem>, path: &PathBuf) {
        for item in items {
            if item.path == *path {
                item.is_expanded = false;
                return;
            }
            if !item.children.is_empty() {
                Self::collapse_dir_recursive(&mut item.children, path);
            }
        }
    }

    fn list_dir(path: &PathBuf) -> io::Result<Vec<ExplorerItem>> {
        let mut items = Vec::new();
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    let name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?")
                        .to_string();
                    let is_dir = path.is_dir();
                    items.push(ExplorerItem {
                        name,
                        path,
                        is_dir,
                        is_expanded: false,
                        children: Vec::new(),
                    });
                }
            }
        }
        items.sort_by(|a, b| {
            if a.is_dir != b.is_dir {
                b.is_dir.cmp(&a.is_dir)
            } else {
                a.name.cmp(&b.name)
            }
        });
        Ok(items)
    }
}
