#ifndef KSA64_VIEWER_HARNESS_PLATFORM_HPP
#define KSA64_VIEWER_HARNESS_PLATFORM_HPP

#include <cstring>
#include <stdexcept>
#include <string>
#include <type_traits>

#if defined(_WIN32)
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#else
#include <dlfcn.h>
#endif

namespace ksa64::native {

#if defined(_WIN32)
using LibraryHandle = HMODULE;
using RawSymbol = FARPROC;
inline constexpr const char* kDefaultBridgePath =
    "..\\..\\target\\viewer\\ksa64_viewer_bridge.dll";

inline LibraryHandle open_library(const char* path) {
    return LoadLibraryA(path);
}

inline RawSymbol find_symbol(LibraryHandle library, const char* name) {
    return GetProcAddress(library, name);
}

inline void close_library(LibraryHandle library) {
    if (library != nullptr) {
        FreeLibrary(library);
    }
}

inline std::string loader_error() {
    return "Win32 dynamic-loader error " + std::to_string(GetLastError());
}
#elif defined(__APPLE__)
using LibraryHandle = void*;
using RawSymbol = void*;
inline constexpr const char* kDefaultBridgePath =
    "../../target/viewer/libksa64_viewer_bridge.dylib";

inline LibraryHandle open_library(const char* path) {
    dlerror();
    return dlopen(path, RTLD_NOW | RTLD_LOCAL);
}

inline void* find_symbol(LibraryHandle library, const char* name) {
    dlerror();
    return dlsym(library, name);
}

inline void close_library(LibraryHandle library) {
    if (library != nullptr) {
        dlclose(library);
    }
}

inline std::string loader_error() {
    const char* error = dlerror();
    return error == nullptr ? "unknown dynamic-loader error" : std::string(error);
}
#else
using LibraryHandle = void*;
using RawSymbol = void*;
inline constexpr const char* kDefaultBridgePath =
    "../../target/viewer/libksa64_viewer_bridge.so";

inline LibraryHandle open_library(const char* path) {
    dlerror();
    return dlopen(path, RTLD_NOW | RTLD_LOCAL);
}

inline void* find_symbol(LibraryHandle library, const char* name) {
    dlerror();
    return dlsym(library, name);
}

inline void close_library(LibraryHandle library) {
    if (library != nullptr) {
        dlclose(library);
    }
}

inline std::string loader_error() {
    const char* error = dlerror();
    return error == nullptr ? "unknown dynamic-loader error" : std::string(error);
}
#endif

template <class T>
T cast_symbol(RawSymbol raw) {
    static_assert(std::is_pointer_v<T>, "dynamic symbol type must be a pointer");
    static_assert(sizeof(T) == sizeof(raw), "dynamic symbol pointer size mismatch");
    T result = nullptr;
    std::memcpy(&result, &raw, sizeof(result));
    return result;
}

template <class T>
T required_symbol(LibraryHandle library, const char* name) {
    RawSymbol raw = find_symbol(library, name);
    if (raw == nullptr) {
        throw std::runtime_error(
            std::string("missing required ABI-v1 symbol ") + name + ": " +
            loader_error());
    }
    return cast_symbol<T>(raw);
}

template <class T>
T optional_symbol(LibraryHandle library, const char* name) {
    RawSymbol raw = find_symbol(library, name);
    return raw == nullptr ? nullptr : cast_symbol<T>(raw);
}

}  // namespace ksa64::native

#endif
