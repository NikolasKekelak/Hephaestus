local project_name = editor.project.get_name() or "my_python_project"

-- Create main.py
editor.project.create_file("main.py", [[
def main():
    print("Hello, world!")

if __name__ == "__main__":
    main()
]])

-- Create requirements.txt
editor.project.create_file("requirements.txt", "")

editor.print("Project " .. project_name .. " initialized as Python project.")

-- Register Python-specific templates
function python_script(directory, name)
    editor.project.create_file(directory .. name .. ".py", [[
def main():
    pass

if __name__ == "__main__":
    main()
]])
end

editor.project.file_creation({
    ["New Python Script"] = "python_script"
})
