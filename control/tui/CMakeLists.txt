# TODO: link against ftxui once vendored/available (not currently in the
# official Arch repos — needs AUR or FetchContent). Plain stub for now so the
# monorepo build stays green.
add_executable(omarchy-kids-control-tui
    src/main.cpp
)

target_link_libraries(omarchy-kids-control-tui PRIVATE
    omarchy-kids-control-core
)
