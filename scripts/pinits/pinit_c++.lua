local project_name = editor.project.get_name() or "my_project"

editor.project.create_file("main.cpp", [[
#include <iostream>

using namespace std;

int main() {
    cout << "Hello world" << endl;
    return 0;
}
]])

editor.project.create_file("CMakeLists.txt", [[
cmake_minimum_required(VERSION 3.10)
project(]] .. project_name .. [[)

set(CMAKE_CXX_STANDARD 17)

add_executable(]] .. project_name .. [[ main.cpp)
]])

editor.print("Project " .. project_name .. " initialized as C++ project.")

function cpp(directory, name)
    editor.project.create_file(directory..name..".cpp",
    [[
    ]]
    )
end

function class(directory, name)
    editor.project.create_file(directory..name..".h",
    [[
#ifndef ]]..to_upper(project_name)..[[_]]..to_upper(name)..[[_H
#define ]]..to_upper(project_name)..[[_]]..to_upper(name)..[[_H


class ]]..name..[[ {
};


#endif
]]
    )
    editor.project.create_file(directory..name..".cpp",
        [[
#include "]] ..name..[[.h"
]]
        )

end


editor.project.file_creation({
        ["New cpp file"] = "cpp",
        ["New class"] = "class"
})