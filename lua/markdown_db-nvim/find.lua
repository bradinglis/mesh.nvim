local bin = require "markdown_db-nvim.binary"
local util = require "markdown_db-nvim.util"
local pick = require "markdown_db-nvim.pick"
local config = require "markdown_db-nvim.config"


local M = {}

M.notes = function()
  util.try(function()
      return bin.get_notes(config.current_vault.dir, config.current_vault.db)
    end,
    pick.from_notes
  )
end

M.tags = function()
  util.try(function()
      return bin.get_tags(config.current_vault.dir, config.current_vault.db)
    end,
    pick.from_tags
  )
end

M.links = function()
  local current_buf = vim.api.nvim_get_current_buf()
  local note_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]

  util.try(function()
      return bin.get_backlinks(config.current_vault.dir, config.current_vault.db, note_id)
    end,
    pick.from_notes
  )
end

M.tag_notes = function(tag)
  return util.try(function()
      return bin.get_tag_notes(config.current_vault.dir, config.current_vault.db, tag)
    end,
    pick.from_notes
  )
end

return M
