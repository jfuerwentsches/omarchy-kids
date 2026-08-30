# First test infrastructure for control/ — a plain GTest binary against
# omarchy-kids-control-core (Qt-free), run via `ctest`. Nothing in gui/ or
# tui/ is covered yet: they're Qt Widgets/SSH-network-dependent and have so
# far only been verified end-to-end against the dev VM (see root
# CLAUDE.md's "Status" tracking entries) — a real gap, but a separate,
# larger effort than adding this.
find_package(GTest REQUIRED)
include(GoogleTest)

add_executable(omarchy-kids-control-core-tests
    status_cache_test.cpp
    host_registry_test.cpp
)

target_link_libraries(omarchy-kids-control-core-tests PRIVATE
    omarchy-kids-control-core
    GTest::gtest_main
)

gtest_discover_tests(omarchy-kids-control-core-tests)
