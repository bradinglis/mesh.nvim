-- TODO: copy fff rust handling 
-- https://github.com/dmtrKovalenko/fff.nvim/blob/main/lua/fff/rust/init.lua
-- https://github.com/dmtrKovalenko/fff.nvim/blob/main/lua/fff/download.lua
local util = require("mesh-nvim.util")

local bin = {}

local get = function(path)
  if not vim.tbl_isempty(bin) then
    return bin
  end

  local paths = util.get_paths()
  package.cpath = paths.release_dir .. "/?.so;"
  bin = require("mesh")
  return bin
end

return get()
