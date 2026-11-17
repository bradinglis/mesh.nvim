local util = require("mesh-nvim.util")

-- Nix builds write runtime.lua with the pinned ONNX Runtime path. Elsewhere,
-- set ORT_DYLIB_PATH yourself before loading the plugin.
local has_runtime, runtime = pcall(require, "mesh-nvim.runtime")
if has_runtime and runtime.onnxruntime and vim.env.ORT_DYLIB_PATH == nil then
  vim.env.ORT_DYLIB_PATH = runtime.onnxruntime
end

local release_cpath = util.get_paths().release_dir .. "/lib?.so"
if not package.cpath:find(release_cpath, 1, true) then
  package.cpath = release_cpath .. ";" .. package.cpath
end

return require("mesh")
