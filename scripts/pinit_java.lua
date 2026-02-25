local project_name = editor.project.get_name() or "my_project"

-- Basic Java project structure
editor.project.create_file("src/Main.java", [[
public class Main {
    public static void main(String[] args) {
        System.out.println("Hello, World!");
    }
}
]])

editor.print("Project " .. project_name .. " initialized as Java project.")

function java_class(directory, name)
    editor.project.create_file(directory .. name .. ".java",
    [[
public class ]] .. name .. [[ {

}
]]
    )
end

function java_interface(directory, name)
    editor.project.create_file(directory .. name .. ".java",
    [[
public interface ]] .. name .. [[ {

}
]]
    )
end

editor.project.file_creation({
    ["New Java Class"] = "java_class",
    ["New Java Interface"] = "java_interface"
})
