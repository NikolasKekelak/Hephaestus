editor.print("Welcome to Hephaestus! Plugin loaded.")

function undo()
    editor.undo()
end

function redo()
    editor.redo()
end

function save()
    editor.save()
end

editor.set_keymap("C-z", "undo")
editor.set_keymap("C-y", "redo")
editor.set_keymap("C-s", "save")