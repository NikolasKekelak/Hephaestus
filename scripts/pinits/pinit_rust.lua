local project_name = editor.project.get_name() or "my_rust_project"

-- Create Cargo.toml
editor.project.create_file("Cargo.toml", [[
[package]
name = "]] .. project_name .. [["
version = "0.1.0"
edition = "2021"

[dependencies]
]])

-- Create src/main.rs
editor.project.create_file("src/main.rs", [[
fn main() {
    println!("Hello, world!");
}
]])

editor.print("Project " .. project_name .. " initialized as Rust project.")

-- Register Rust-specific templates
function rust_mod(directory, name)
    editor.project.create_file(directory .. name .. ".rs", "pub fn " .. name .. "() {\n    \n}\n")
end

editor.project.file_creation({
    ["New Rust Module"] = "rust_mod"
})
