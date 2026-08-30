cmake_minimum_required(VERSION 3.21)
project(omarchy-kids-control LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

enable_testing()

add_subdirectory(core)
add_subdirectory(gui)
add_subdirectory(tui)
add_subdirectory(tests)
