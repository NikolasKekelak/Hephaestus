editor.print("Welcome to heph, the forge of Gods!")

function undo()
    editor.undo()
end

function redo()
    editor.redo()
end

function save()
    editor.save()
end

function toggle_explorer()
    editor.project.explorer.toggle()
end

function focus_explorer()
    editor.project.explorer.focus()
end

editor.set_keymap("C-z", "undo")
editor.set_keymap("C-y", "redo")
editor.set_keymap("C-s", "save")
editor.set_keymap("C-p", "toggle_explorer")
editor.set_keymap("A-q", "focus_explorer")