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

-- Keibinds

editor.set_keymap("C-z", "undo")
editor.set_keymap("C-y", "redo")
editor.set_keymap("C-s", "save")
editor.set_keymap("C-p", "toggle_explorer")
editor.set_keymap("A-q", "focus_explorer")

-- Theme
editor.set_theme({
    colors = {
        text = { r = 220, g = 220, b = 220 },
        bg = { r = 30, g = 30, b = 30 },
        directory = { r = 200, g = 200, b = 100 },      -- Pale Yellow
        dot_directory = { r = 150, g = 150, b = 150 },  -- Grey
        file = { r = 255, g = 255, b = 255 },           -- White
        extensions = {
            [".h"] = { r = 255, g = 165, b = 0 },     -- Orange
            [".c"] = { r = 100, g = 149, b = 237 },   -- Cornflower Blue
            [".cpp"] = { r = 100, g = 149, b = 237 }, -- Cornflower Blue
            [".py"] = { r = 50, g = 205, b = 50 },    -- Lime Green
            [".lua"] = { r = 0, g = 0, b = 255 },     -- Blue
            [".java"] = { r = 255, g = 255, b = 255 }, -- White (Default for Java)
            ["class"] = { r = 255, g = 165, b = 0 },   -- Orange
            ["interface"] = { r = 0, g = 255, b = 0 }, -- Green
        }
    },
    font = {
        name = "Fira Code",
        path = "/usr/share/fonts/TTF/FiraCode-Regular.ttf"
    }
})


--Project types
editor.project.type.init("C", "./scripts/pinit_c.lua")
editor.project.type.init("C++", "./scripts/pinit_c++.lua")
editor.project.type.init("Java", "./scripts/pinit_java.lua")

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
