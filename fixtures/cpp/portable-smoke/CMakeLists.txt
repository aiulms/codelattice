cmake_minimum_required(VERSION 3.14)
project(PortableSmokeCpp LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_executable(portable_smoke
    src/main.cpp
    src/math_utils.cpp
    src/logger.cpp
)
target_include_directories(portable_smoke PRIVATE include)
