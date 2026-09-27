cmake_minimum_required(VERSION 3.22.1)
project(carbon_graphics LANGUAGES CXX)

add_library(
    carbon_graphics
    SHARED
    carbon_graphics_jni.cpp
    gpu_probe.cpp
    presentation.cpp
)

target_compile_features(carbon_graphics PRIVATE cxx_std_20)
target_compile_options(carbon_graphics PRIVATE -Wall -Wextra -Wpedantic)
target_link_libraries(carbon_graphics PRIVATE android dl)
