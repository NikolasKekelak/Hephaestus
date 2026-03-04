editor.print("Welcome to Hephaestus, the forge of Gods!")

-- Functions defintions
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

function open_context_menu()
    editor.project.explorer.open_context_menu()
end

-- Keibinds

editor.set_keymap("C-z", "undo")
editor.set_keymap("C-y", "redo")
editor.set_keymap("C-s", "save")
editor.set_keymap("C-p", "toggle_explorer")
editor.set_keymap("A-q", "focus_explorer")
editor.set_keymap("e", "open_context_menu")

-- Theme
editor.set_theme({
    colors = {
        text = { r = 220, g = 220, b = 220 },
        bg = { r = 3, g = 10, b = 10 },
        directory = { r = 200, g = 200, b = 100 },
        dot_directory = { r = 150, g = 150, b = 150 },
        file = { r = 255, g = 255, b = 255 },
        extensions = {
            [".h"] = { r = 255, g = 165, b = 0 },
            [".c"] = { r = 100, g = 149, b = 237 },
            [".cpp"] = { r = 100, g = 149, b = 237 },
            [".py"] = { r = 50, g = 205, b = 50 },
            [".lua"] = { r = 50, g = 50, b = 255 },
            [".java"] = { r = 255, g = 255, b = 255 },
            ["class"] = { r = 255, g = 165, b = 0 },
            ["interface"] = { r = 0, g = 255, b = 0 },
            [".rs"] = { r = 183, g = 65, b = 14 },
        }
    },
    font = {
        name = "Fira Code",
        path = "/usr/share/fonts/TTF/FiraCode-Regular.ttf"
    }
})


--Project types
editor.project.type.init_folder("scripts/pinits", "pinits.lua")

-- Global File Templates
function plain_text(directory, name)
    editor.project.create_file(directory .. name .. ".txt", "")
end

function markdown(directory, name)
    editor.project.create_file(directory .. name .. ".md", "# " .. name .. "\n")
end

editor.project.file_creation({
    ["Text File"] = "plain_text",
    ["Markdown File"] = "markdown"
})