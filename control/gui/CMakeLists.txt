find_package(Qt6 REQUIRED COMPONENTS Widgets)

add_executable(omarchy-kids-control
    src/main.cpp
    src/main_window.cpp
    src/pairing_dialog.cpp
)

target_link_libraries(omarchy-kids-control PRIVATE
    omarchy-kids-control-core
    Qt6::Widgets
)
