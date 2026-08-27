find_package(Qt6 REQUIRED COMPONENTS Widgets)

add_executable(omarchy-kids-control
    src/main.cpp
)

target_link_libraries(omarchy-kids-control PRIVATE
    omarchy-kids-control-core
    Qt6::Widgets
)
