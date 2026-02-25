local project_name = editor.project.get_name() or "my_project"

editor.project.create_file("main.c", [[
#include <stdio.h>

int main() {
    printf("Hello world\n");
    return 0;
}
]])

editor.project.create_file("CMakeLists.txt", [[
cmake_minimum_required(VERSION 3.10)
project(]] .. project_name .. [[)

set(CMAKE_C_STANDARD 11)

add_executable(]] .. project_name .. [[ main.c)
]])

editor.print("Project " .. project_name .. " initialized as C project.")
