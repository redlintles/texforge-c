# Consumer project used to test installed packages (CMake install, Conan, vcpkg).
cmake_minimum_required(VERSION 3.21)
project(textforge_c_consumer LANGUAGES C)

find_package(textforge-c CONFIG REQUIRED)
add_executable(quickstart ../../examples/c/quickstart.c)
target_link_libraries(quickstart PRIVATE textforge-c::textforge-c)
set_target_properties(quickstart PROPERTIES C_STANDARD 11 C_STANDARD_REQUIRED ON)
if(MSVC)
    target_compile_options(quickstart PRIVATE /utf-8)  # the example contains UTF-8 literals
endif()
