cmake_minimum_required(VERSION 3.21)
project(omarchy-kids-control LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

find_package(Qt6 REQUIRED COMPONENTS Widgets)

option(OMARCHY_KIDS_BUILD_TUI "Build the control TUI stub" ON)

enable_testing()

add_subdirectory(core)
add_subdirectory(gui)

if(OMARCHY_KIDS_BUILD_TUI)
    add_subdirectory(tui)
endif()

add_subdirectory(tests)
