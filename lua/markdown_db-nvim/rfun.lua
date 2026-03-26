local find = require("markdown_db-nvim.find")
local util = require("markdown_db-nvim.util")
local p = require "markdown_db-nvim"

local M = {}

local funs = {
  tag = find.find_tag_notes,
  note = require("markdown_db-nvim.buf").open,
}

local function run(cmd, opts, update, viz)
  if update then
    p.update()
  end

  funs[cmd](opts, viz)
end

function M.try(f, update, viz)
  util.try(f,
    function(x, y)
      if x then 
        run(x, y, update, viz)
      end
    end
  )
end

return M
