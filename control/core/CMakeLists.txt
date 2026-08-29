find_package(tomlplusplus REQUIRED)

add_library(omarchy-kids-control-core STATIC
    src/host_registry.cpp
)

target_include_directories(omarchy-kids-control-core PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

target_link_libraries(omarchy-kids-control-core PRIVATE
    tomlplusplus::tomlplusplus
)
