local util = require("markdown_db-nvim.util")

local function get_platform()
    local os_name = jit.os:lower()
    local arch = jit.arch:lower()

    if os_name == "osx" then
        local triple = arch == "arm64" and "aarch64-apple-darwin" or "x86_64-apple-darwin"
        return triple, "libdifftastic_nvim.dylib"
    elseif os_name == "linux" then
        local triple = arch == "arm64" and "aarch64-unknown-linux-gnu" or "x86_64-unknown-linux-gnu"
        return triple, "libdifftastic_nvim.so"
    elseif os_name == "windows" then
        return "x86_64-pc-windows-msvc", "difftastic_nvim.dll"
    end

    return nil, nil
end

--- Get the library filename for a given extension.
--- @param ext string|nil Library extension
--- @return string
local function get_lib_name(ext)
    if ext == ".dylib" then
        return "libdifftastic_nvim.dylib"
    elseif ext == ".dll" then
        return "difftastic_nvim.dll"
    else
        return "libdifftastic_nvim.so"
    end
end

local get = function()
    local platform, ext = get_platform()
    local paths = util.get_paths()
    package.cpath = paths.release_dir .. "/?.so;"
    return require("markdown_db")
end

return get()
