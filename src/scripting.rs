use std::fs;
use mlua::prelude::*;
use crate::editor::Editor;

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
                s.focus_on_explorer = true;
                Editor::sync_explorer_selection(&mut s);
            } else {
                s.focus_on_explorer = false;
            }
            Ok(())
        })?)?;

        let s_clone = state_clone.clone();
        explorer_api.set("focus", self.lua.create_function(move |_, (): ()| {
            let mut s = s_clone.lock().unwrap();
            s.focus_on_explorer = !s.focus_on_explorer;
            if s.focus_on_explorer {
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
        editor_api.set("project", project_api)?;

        globals.set("editor", editor_api)?;

        if fs::metadata("init.lua").is_ok() {
            self.lua.load(fs::read_to_string("init.lua")?).exec()?;
        }

        Ok(())
    }
}
