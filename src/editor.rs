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

#[derive(Clone)]
pub struct ExplorerItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub children: Vec<ExplorerItem>,
}

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Focus {
    Editor,
    Explorer,
    Popup,
}

#[derive(Clone, Copy, Debug)]
pub struct EditorColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub struct Theme {
    pub text_color: EditorColor,
    pub bg_color: EditorColor,
    pub directory_color: EditorColor,
    pub dot_directory_color: EditorColor,
    pub file_color: EditorColor,
    pub font_name: String,
    pub font_path: String,
    pub file_extension_colors: HashMap<String, EditorColor>,
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
    pub focus: Focus,
    pub prev_focus: Focus,
    pub recent_files: Vec<PathBuf>,
    pub project_type_inits: HashMap<String, String>,
    pub file_templates: HashMap<String, String>,
    pub input_mode: InputMode,
    pub input_buffer: String,
    pub input_prompt: String,
    pub menu_selected_index: usize,
    pub theme: Theme,
}

#[derive(PartialEq, Clone)]
pub enum InputMode {
    Normal,
    Input,
    Menu,
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
            let registry = ProjectRegistry::new();
            let mut project_type = None;
            if let Ok(Some(entry)) = registry.find_by_name(root.file_name().and_then(|n| n.to_str()).unwrap_or("")) {
                project_type = entry.project_type;
            }
            let p = Project::new(root.clone(), project_type);
            let _ = registry.remember(p.name.clone(), p.root.clone(), p.project_type.clone());
            p
        });

        let mut explorer_items = Vec::new();
        if let Some(ref p) = project {
            explorer_items = vec![ExplorerItem {
                name: p.name.clone(),
                path: p.root.clone(),
                is_dir: true,
                is_expanded: true,
                children: Self::list_dir(&p.root)?,
            }];
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
            focus: Focus::Editor,
            prev_focus: Focus::Editor,
            recent_files: Vec::new(),
            project_type_inits: HashMap::new(),
            file_templates: HashMap::new(),
            input_mode: InputMode::Normal,
            input_buffer: String::new(),
            input_prompt: String::new(),
            menu_selected_index: 0,
            theme: Theme {
                text_color: EditorColor { r: 255, g: 255, b: 255 },
                bg_color: EditorColor { r: 0, g: 0, b: 0 },
                directory_color: EditorColor { r: 255, g: 255, b: 255 },
                dot_directory_color: EditorColor { r: 255, g: 255, b: 255 },
                file_color: EditorColor { r: 255, g: 255, b: 255 },
                font_name: String::new(),
                font_path: String::new(),
                file_extension_colors: HashMap::new(),
            },
        }));

        let lua = Lua::new();

        Ok(Self { state, lua })
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.init_lua().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Initialize project if type is known
        let project_type = {
            let s = self.state.lock().unwrap();
            s.project.as_ref().and_then(|p| p.project_type.clone())
        };
        if let Some(p_type) = project_type {
            let _ = self.init_project(&p_type);
        }

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

        let bg_color = crossterm::style::Color::Rgb {
            r: s.theme.bg_color.r,
            g: s.theme.bg_color.g,
            b: s.theme.bg_color.b,
        };
        let fg_color = crossterm::style::Color::Rgb {
            r: s.theme.text_color.r,
            g: s.theme.text_color.g,
            b: s.theme.text_color.b,
        };

        execute!(
            stdout,
            cursor::MoveTo(0, 0),
            crossterm::style::SetBackgroundColor(bg_color),
            crossterm::style::SetForegroundColor(fg_color),
            Clear(ClearType::All)
        )?;

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
                    
                    let mut current_fg;
                    if item.is_dir {
                        let color = if item.name.starts_with('.') {
                            s.theme.dot_directory_color
                        } else {
                            s.theme.directory_color
                        };
                        current_fg = crossterm::style::Color::Rgb {
                            r: color.r,
                            g: color.g,
                            b: color.b,
                        };
                    } else {
                        // Default file color
                        let file_color = s.theme.file_color;
                        current_fg = crossterm::style::Color::Rgb {
                            r: file_color.r,
                            g: file_color.g,
                            b: file_color.b,
                        };

                        if let Some(ext) = item.path.extension().and_then(|e| e.to_str()) {
                            let ext_with_dot = format!(".{}", ext);
                            if let Some(color) = s.theme.file_extension_colors.get(&ext_with_dot) {
                                current_fg = crossterm::style::Color::Rgb {
                                    r: color.r,
                                    g: color.g,
                                    b: color.b,
                                };
                            }

                            // Java-specific overrides for class/interface
                            if ext == "java" {
                                if let Ok(content) = fs::read_to_string(&item.path) {
                                    if content.contains("interface ") {
                                        if let Some(color) = s.theme.file_extension_colors.get("interface") {
                                            current_fg = crossterm::style::Color::Rgb {
                                                r: color.r,
                                                g: color.g,
                                                b: color.b,
                                            };
                                        }
                                    } else if content.contains("class ") {
                                        if let Some(color) = s.theme.file_extension_colors.get("class") {
                                            current_fg = crossterm::style::Color::Rgb {
                                                r: color.r,
                                                g: color.g,
                                                b: color.b,
                                            };
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let line = format!("{}{}{}", indent, prefix, item.name);
                    let truncated = if line.len() > explorer_width as usize {
                        &line[..explorer_width as usize]
                    } else {
                        &line
                    };
                    
                    execute!(stdout, crossterm::style::SetForegroundColor(current_fg))?;
                    if s.focus == Focus::Explorer && s.explorer_selected_index == i {
                        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse))?;
                        write!(stdout, "{:<width$}", truncated, width = explorer_width as usize)?;
                        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset))?;
                        execute!(stdout, crossterm::style::SetBackgroundColor(bg_color), crossterm::style::SetForegroundColor(fg_color))?;
                    } else {
                        write!(stdout, "{}", truncated)?;
                    }
                    execute!(stdout, crossterm::style::SetForegroundColor(fg_color))?;
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

        if s.focus == Focus::Explorer {
            execute!(stdout, cursor::Hide)?;
        } else {
            execute!(stdout, cursor::MoveTo(explorer_width + 1 + s.cursor_x as u16, (s.cursor_y - s.row_off) as u16), cursor::Show)?;
        }

        if s.focus == Focus::Popup {
            let menu_width = 40;
            let items = if s.input_mode == InputMode::Menu {
                let mut m = vec!["[File]".to_string(), "[Folder]".to_string()];
                let mut templates: Vec<String> = s.file_templates.keys().cloned().collect();
                templates.sort();
                m.extend(templates);
                m
            } else {
                vec![]
            };

            let menu_height = if s.input_mode == InputMode::Menu {
                items.len() + 2
            } else {
                3
            };

            let start_x = (cols.saturating_sub(menu_width)) / 2;
            let start_y = (rows.saturating_sub(menu_height as u16)) / 2;

            for i in 0..menu_height {
                execute!(stdout, cursor::MoveTo(start_x, start_y + i as u16))?;
                write!(stdout, "{:<width$}", " ", width = menu_width as usize)?;
            }

            execute!(stdout, cursor::MoveTo(start_x, start_y))?;
            write!(stdout, "┌{:─<width$}┐", "", width = (menu_width - 2) as usize)?;
            for i in 1..menu_height - 1 {
                execute!(stdout, cursor::MoveTo(start_x, start_y + i as u16))?;
                write!(stdout, "│")?;
                execute!(stdout, cursor::MoveTo(start_x + menu_width - 1, start_y + i as u16))?;
                write!(stdout, "│")?;
            }
            execute!(stdout, cursor::MoveTo(start_x, start_y + menu_height as u16 - 1))?;
            write!(stdout, "└{:─<width$}┘", "", width = (menu_width - 2) as usize)?;

            if s.input_mode == InputMode::Menu {
                for (i, item) in items.iter().enumerate() {
                    execute!(stdout, cursor::MoveTo(start_x + 2, start_y + 1 + i as u16))?;
                    if s.menu_selected_index == i {
                        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reverse))?;
                        write!(stdout, "{}", item)?;
                        execute!(stdout, crossterm::style::SetAttribute(crossterm::style::Attribute::Reset))?;
                    } else {
                        write!(stdout, "{}", item)?;
                    }
                }
            } else {
                execute!(stdout, cursor::MoveTo(start_x + 2, start_y + 1))?;
                write!(stdout, "{}: {}", s.input_prompt, s.input_buffer)?;
                execute!(stdout, cursor::Show, cursor::MoveTo(start_x + 2 + s.input_prompt.len() as u16 + 2 + s.input_buffer.len() as u16, start_y + 1))?;
            }
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

                    // 1. Priority: Lua key mappings (can be global)
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

                    // 2. Focus-based event handling
                    match s.focus {
                        Focus::Popup => {
                            if s.input_mode == InputMode::Menu {
                                match key.code {
                                    KeyCode::Up => {
                                        if s.menu_selected_index > 0 {
                                            s.menu_selected_index -= 1;
                                        }
                                    }
                                    KeyCode::Down => {
                                        let mut templates: Vec<String> = s.file_templates.keys().cloned().collect();
                                        templates.sort();
                                        let items_count = 2 + templates.len();
                                        if s.menu_selected_index + 1 < items_count {
                                            s.menu_selected_index += 1;
                                        }
                                    }
                                    KeyCode::Enter => {
                                        let index_in_menu = s.menu_selected_index;
                                        let mut templates: Vec<String> = s.file_templates.keys().cloned().collect();
                                        templates.sort();
                                        s.input_buffer.clear();
                                        if index_in_menu == 0 { // [File]
                                            s.input_mode = InputMode::Input;
                                            s.input_prompt = "File Name".to_string();
                                        } else if index_in_menu == 1 { // [Folder]
                                            s.input_mode = InputMode::Input;
                                            s.input_prompt = "Folder Name".to_string();
                                        } else if index_in_menu - 2 < templates.len() {
                                            let template_name = templates[index_in_menu - 2].clone();
                                            s.input_mode = InputMode::Input;
                                            s.input_prompt = format!("Name for {}", template_name);
                                        }
                                    }
                                    KeyCode::Esc => {
                                        s.focus = s.prev_focus;
                                        s.input_mode = InputMode::Normal;
                                    }
                                    _ => {}
                                }
                            } else if s.input_mode == InputMode::Input {
                                match key.code {
                                    KeyCode::Char(c) => {
                                        s.input_buffer.push(c);
                                    }
                                    KeyCode::Backspace => {
                                        s.input_buffer.pop();
                                    }
                                    KeyCode::Enter => {
                                        let name = s.input_buffer.clone();
                                        let prompt = s.input_prompt.clone();
                                        let index_in_menu = s.menu_selected_index;
                                        let explorer_index = s.explorer_selected_index;
                                        
                                        if !name.is_empty() {
                                            let mut templates: Vec<String> = s.file_templates.keys().cloned().collect();
                                            templates.sort();
                                            let project_root = s.project.as_ref().map(|p| p.root.clone());
                                            
                                            if let Some(root) = project_root {
                                                let mut flat = Vec::new();
                                                Self::flatten_explorer(&s.explorer_items, 0, &mut flat);
                                                let target_dir = if let Some((item, _)) = flat.get(explorer_index) {
                                                    if item.is_dir {
                                                        item.path.clone()
                                                    } else {
                                                        item.path.parent().unwrap_or(&root).to_path_buf()
                                                    }
                                                } else {
                                                    root.clone()
                                                };

                                                if prompt == "File Name" {
                                                    let path = target_dir.join(name);
                                                    let _ = fs::write(&path, "");
                                                    if let Some(root_item) = s.explorer_items.get_mut(0) {
                                                        if target_dir == root {
                                                            if let Ok(new_children) = Self::list_dir(&root) {
                                                                let mut new_items = Vec::new();
                                                                for mut new_item in new_children {
                                                                    if let Some(old_item) = root_item.children.iter().find(|i| i.path == new_item.path) {
                                                                        new_item.is_expanded = old_item.is_expanded;
                                                                        if !old_item.children.is_empty() {
                                                                            new_item.children = old_item.children.clone();
                                                                        }
                                                                    }
                                                                    new_items.push(new_item);
                                                                }
                                                                root_item.children = new_items;
                                                            }
                                                        } else {
                                                            Self::refresh_dir_recursive(&mut root_item.children, &target_dir);
                                                        }
                                                    }
                                                } else if prompt == "Folder Name" {
                                                    let path = target_dir.join(name);
                                                    let _ = fs::create_dir_all(&path);
                                                    if let Some(root_item) = s.explorer_items.get_mut(0) {
                                                        if target_dir == root {
                                                            if let Ok(new_children) = Self::list_dir(&root) {
                                                                let mut new_items = Vec::new();
                                                                for mut new_item in new_children {
                                                                    if let Some(old_item) = root_item.children.iter().find(|i| i.path == new_item.path) {
                                                                        new_item.is_expanded = old_item.is_expanded;
                                                                        if !old_item.children.is_empty() {
                                                                            new_item.children = old_item.children.clone();
                                                                        }
                                                                    }
                                                                    new_items.push(new_item);
                                                                }
                                                                root_item.children = new_items;
                                                            }
                                                        } else {
                                                            Self::refresh_dir_recursive(&mut root_item.children, &target_dir);
                                                        }
                                                    }
                                                } else {
                                                    let template_name = templates[index_in_menu - 2].clone();
                                                    if let Some(func_name) = s.file_templates.get(&template_name).cloned() {
                                                        let rel_target = target_dir.strip_prefix(&root).unwrap_or(std::path::Path::new("")).to_str().unwrap_or("").to_string();
                                                        let rel_target = if rel_target.is_empty() { String::new() } else { format!("{}/", rel_target) };
                                                        drop(s);
                                                        let globals = self.lua.globals();
                                                        if let Ok(func) = globals.get::<_, LuaFunction>(func_name) {
                                                            let _ = func.call::<_, ()>((rel_target, name));
                                                        }
                                                        let mut s = self.state.lock().unwrap();
                                                        if let Some(ref mut root_item) = s.explorer_items.get_mut(0) {
                                                            if target_dir == root {
                                                                if let Ok(new_children) = Self::list_dir(&root) {
                                                                    let mut new_items = Vec::new();
                                                                    for mut new_item in new_children {
                                                                        if let Some(old_item) = root_item.children.iter().find(|i| i.path == new_item.path) {
                                                                            new_item.is_expanded = old_item.is_expanded;
                                                                            if !old_item.children.is_empty() {
                                                                                new_item.children = old_item.children.clone();
                                                                            }
                                                                        }
                                                                        new_items.push(new_item);
                                                                    }
                                                                    root_item.children = new_items;
                                                                }
                                                            } else {
                                                                Self::refresh_dir_recursive(&mut root_item.children, &target_dir);
                                                            }
                                                        }
                                                        s.focus = s.prev_focus;
                                                        s.input_mode = InputMode::Normal;
                                                        return Ok(());
                                                    }
                                                }
                                            }
                                        }
                                        s.focus = s.prev_focus;
                                        s.input_mode = InputMode::Normal;
                                    }
                                    KeyCode::Esc => {
                                        s.focus = s.prev_focus;
                                        s.input_mode = InputMode::Normal;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        Focus::Explorer => {
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
                                            drop(s);
                                            self.open_file(path)?;
                                            let mut s = self.state.lock().unwrap();
                                            s.focus = Focus::Editor;
                                        }
                                    }
                                }
                                KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                    if s.is_explorer_visible {
                                        s.prev_focus = Focus::Explorer;
                                        s.focus = Focus::Popup;
                                        s.input_mode = InputMode::Menu;
                                        s.menu_selected_index = 0;
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
                                    s.focus = Focus::Editor;
                                }
                                _ => {}
                            }
                        }
                        Focus::Editor => {
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

    fn refresh_dir_recursive(items: &mut Vec<ExplorerItem>, path: &PathBuf) -> bool {
        for item in items {
            if item.path == *path {
                if let Ok(new_children) = Self::list_dir(&item.path) {
                    // Merge new children with existing ones to preserve expansion state
                    let mut new_items = Vec::new();
                    for mut new_item in new_children {
                        if let Some(old_item) = item.children.iter().find(|i| i.path == new_item.path) {
                            new_item.is_expanded = old_item.is_expanded;
                            if !old_item.children.is_empty() {
                                new_item.children = old_item.children.clone();
                            }
                        }
                        new_items.push(new_item);
                    }
                    item.children = new_items;
                    item.is_expanded = true; // Ensure it's expanded so user sees the new item
                    return true;
                }
            }
            if !item.children.is_empty() {
                if Self::refresh_dir_recursive(&mut item.children, path) {
                    return true;
                }
            }
        }
        false
    }

    pub fn init_project(&mut self, project_type: &str) -> io::Result<bool> {
        let script_name = {
            let s = self.state.lock().unwrap();
            s.project_type_inits.get(project_type).cloned()
        };

        let script_name = match script_name {
            Some(s) => s,
            None => return Ok(false),
        };

        if !std::path::Path::new(&script_name).exists() {
            return Ok(false);
        }

        let lua_code = fs::read_to_string(&script_name)?;
        self.lua.load(&lua_code).exec().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        
        // Refresh explorer after init
        let mut s = self.state.lock().unwrap();
        if let Some(ref mut p) = s.project {
            p.project_type = Some(project_type.to_string());
            let registry = ProjectRegistry::new();
            let _ = registry.remember(p.name.clone(), p.root.clone(), p.project_type.clone());
            
            if let Ok(items) = Self::list_dir(&p.root) {
                s.explorer_items = vec![ExplorerItem {
                    name: p.name.clone(),
                    path: p.root.clone(),
                    is_dir: true,
                    is_expanded: true,
                    children: items,
                }];
            }
        }
        
        Ok(true)
    }
}
