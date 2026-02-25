use std::fs;
use std::collections::HashMap;
use mlua::prelude::*;
use crate::editor::{Editor, Focus, EditorColor};

impl Editor {
    pub fn init_lua(&self) -> LuaResult<()> {
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

        let state_clone = self.state.clone();
        let set_theme = self.lua.create_function(move |_, theme_table: LuaTable| {
            let mut s = state_clone.lock().unwrap();
            
            if let Ok(colors) = theme_table.get::<_, LuaTable>("colors") {
                if let Ok(text) = colors.get::<_, LuaTable>("text") {
                    s.theme.text_color = EditorColor {
                        r: text.get("r").unwrap_or(255),
                        g: text.get("g").unwrap_or(255),
                        b: text.get("b").unwrap_or(255),
                    };
                }
                if let Ok(bg) = colors.get::<_, LuaTable>("bg") {
                    s.theme.bg_color = EditorColor {
                        r: bg.get("r").unwrap_or(0),
                        g: bg.get("g").unwrap_or(0),
                        b: bg.get("b").unwrap_or(0),
                    };
                }
                if let Ok(dir) = colors.get::<_, LuaTable>("directory") {
                    s.theme.directory_color = EditorColor {
                        r: dir.get("r").unwrap_or(255),
                        g: dir.get("g").unwrap_or(255),
                        b: dir.get("b").unwrap_or(255),
                    };
                }
                if let Ok(dot_dir) = colors.get::<_, LuaTable>("dot_directory") {
                    s.theme.dot_directory_color = EditorColor {
                        r: dot_dir.get("r").unwrap_or(255),
                        g: dot_dir.get("g").unwrap_or(255),
                        b: dot_dir.get("b").unwrap_or(255),
                    };
                }
                if let Ok(file) = colors.get::<_, LuaTable>("file") {
                    s.theme.file_color = EditorColor {
                        r: file.get("r").unwrap_or(255),
                        g: file.get("g").unwrap_or(255),
                        b: file.get("b").unwrap_or(255),
                    };
                }
                if let Ok(extensions) = colors.get::<_, LuaTable>("extensions") {
                    let mut ext_colors = HashMap::new();
                    for pair in extensions.pairs::<String, LuaTable>() {
                        if let Ok((ext, color_table)) = pair {
                            let color = EditorColor {
                                r: color_table.get("r").unwrap_or(255),
                                g: color_table.get("g").unwrap_or(255),
                                b: color_table.get("b").unwrap_or(255),
                            };
                            ext_colors.insert(ext, color);
                        }
                    }
                    s.theme.file_extension_colors = ext_colors;
                }
            }
            
            if let Ok(font) = theme_table.get::<_, LuaTable>("font") {
                if let Ok(name) = font.get::<_, String>("name") {
                    s.theme.font_name = name;
                }
                if let Ok(path) = font.get::<_, String>("path") {
                    s.theme.font_path = path;
                }
            }
            
            Ok(())
        })?;
        editor_api.set("set_theme", set_theme)?;

        let state_clone = self.state.clone();
        let project_api = self.lua.create_table()?;
        
        let s_clone = state_clone.clone();
        project_api.set("get_root", self.lua.create_function(move |_, (): ()| {
            let s = s_clone.lock().unwrap();
            Ok(s.project.as_ref().map(|p| p.root.to_str().unwrap_or("").to_string()))
        })?)?;

        let s_clone = state_clone.clone();
        project_api.set("get_name", self.lua.create_function(move |_, (): ()| {
            let s = s_clone.lock().unwrap();
            Ok(s.project.as_ref().map(|p| p.name.clone()))
        })?)?;

        let explorer_api = self.lua.create_table()?;
        let s_clone = state_clone.clone();
        explorer_api.set("toggle", self.lua.create_function(move |_, (): ()| {
            let mut s = s_clone.lock().unwrap();
            s.is_explorer_visible = !s.is_explorer_visible;
            if s.is_explorer_visible {
                s.focus = Focus::Explorer;
                Editor::sync_explorer_selection(&mut s);
            } else if s.focus == Focus::Explorer {
                s.focus = Focus::Editor;
            }
            Ok(())
        })?)?;

        let s_clone = state_clone.clone();
        explorer_api.set("focus", self.lua.create_function(move |_, (): ()| {
            let mut s = s_clone.lock().unwrap();
            if s.focus == Focus::Explorer {
                s.focus = Focus::Editor;
            } else {
                s.focus = Focus::Explorer;
                s.is_explorer_visible = true;
                Editor::sync_explorer_selection(&mut s);
            }
            Ok(())
        })?)?;

        let s_clone = state_clone.clone();
        editor_api.set("get_recent_files", self.lua.create_function(move |_, (): ()| {
            let s = s_clone.lock().unwrap();
            let recent: Vec<String> = s.recent_files.iter()
                .map(|p| p.to_str().unwrap_or("").to_string())
                .collect();
            Ok(recent)
        })?)?;

        project_api.set("explorer", explorer_api)?;

        let type_api = self.lua.create_table()?;
        let s_clone_type = state_clone.clone();
        type_api.set("init", self.lua.create_function(move |_, (p_type, script): (String, String)| {
            let mut s = s_clone_type.lock().unwrap();
            s.project_type_inits.insert(p_type, script);
            Ok(())
        })?)?;
        project_api.set("type", type_api)?;
        
        project_api.set("create_file", self.lua.create_function(move |_, (rel_path, content): (String, String)| {
            let s = state_clone.lock().unwrap();
            if let Some(ref p) = s.project {
                let full_path = p.root.join(rel_path);
                if let Some(parent) = full_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::write(full_path, content);
            }
            Ok(())
        })?)?;

        let state_clone = self.state.clone();
        project_api.set("file_creation", self.lua.create_function(move |_, templates: HashMap<String, String>| {
            let mut s = state_clone.lock().unwrap();
            s.file_templates.extend(templates);
            Ok(())
        })?)?;

        let to_upper = self.lua.create_function(|_, s: String| {
            Ok(s.to_uppercase())
        })?;
        globals.set("to_upper", to_upper)?;

        editor_api.set("project", project_api)?;

        globals.set("editor", editor_api)?;

        if fs::metadata("init.lua").is_ok() {
            self.lua.load(fs::read_to_string("init.lua")?).exec()?;
        }

        Ok(())
    }
}
