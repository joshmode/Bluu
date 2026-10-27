cmake_minimum_required(VERSION 3.28)
project(openwinblue VERSION 0.4.0 LANGUAGES C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# GoogleTest via FetchContent (used in tests/)
include(FetchContent)
option(OWB_BUILD_TESTS "Build native host tests" ON)
if(OWB_BUILD_TESTS)
  FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG        v1.14.0
  )
  FetchContent_MakeAvailable(googletest)
  enable_testing()
endif()

if(WIN32)
  add_subdirectory(service)
  if(OWB_BUILD_TESTS)
    add_subdirectory(tests/service)
  endif()
endif()

# The driver itself is built separately via WDK (driver/owb_a2dp.vcxproj);
# driver/CMakeLists.txt only builds its WDK-free AVDTP packet module for host tests.
add_subdirectory(driver)
if(OWB_BUILD_TESTS)
  add_subdirectory(tests/driver)
endif()
