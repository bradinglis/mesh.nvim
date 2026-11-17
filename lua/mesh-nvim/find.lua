local bin = require "mesh-nvim.binary"
local util = require "mesh-nvim.util"
local pick = require "mesh-nvim.pick"
local config = require "mesh-nvim.config"

local M = {}

--- Run a binary query against the active vault and feed the result to a
--- picker handler.
---@param query fun(): any
---@param handler? fun(result: any)
local function find(query, handler)
  util.try(query, handler or pick.from_notes)
end

function M.all()
  find(function()
    return bin.get_notes(config.current_vault.dir, config.current_vault.db)
  end)
end

function M.grep()
  find(function()
    return bin.get_lines(config.current_vault.dir, config.current_vault.db)
  end)
end

---@param note_type "note"|"author"|"source"|"journal"
---@param handler? fun(result: any)
local function find_type(note_type, handler)
  find(function()
    return bin.get_type(config.current_vault.dir, config.current_vault.db, note_type)
  end, handler)
end

function M.notes()
  find_type("note")
end

function M.authors()
  find_type("author")
end

function M.sources()
  find_type("source")
end

function M.tags()
  find(function()
    return bin.get_tags(config.current_vault.dir, config.current_vault.db)
  end, pick.from_tags)
end

function M.links()
  local current_buf = vim.api.nvim_get_current_buf()
  local note_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]

  find(function()
    return bin.get_backlinks(config.current_vault.dir, config.current_vault.db, note_id)
  end)
end

---@param tag string
function M.tag_notes(tag)
  return find(function()
    return bin.get_tag_notes(config.current_vault.dir, config.current_vault.db, tag)
  end)
end

return M
