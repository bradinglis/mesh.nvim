local util = require("mesh-nvim.util")

local release_cpath = util.get_paths().release_dir .. "/lib?.so"
if not package.cpath:find(release_cpath, 1, true) then
  package.cpath = release_cpath .. ";" .. package.cpath
end

return require("mesh")
