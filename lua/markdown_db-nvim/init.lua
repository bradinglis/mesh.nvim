local util = require "markdown_db-nvim.util"
local pick = require "markdown_db-nvim.pick"

local M = {}

local bin = {}

---@class vault_config
---@field name string
---@field dir string
---@field db string?

---@class opts
---@field vaults table<string, string|vault_config>

---@type opts
M.config = {
  vaults = {
    default = "none",
  }
}

---@type vault_config
M.current_vault = {
  name = "none",
  dir = "",
  db = ""
}

M.setup = function(opts)
  M.config = vim.tbl_extend("force", M.config, opts)

  bin = require("markdown_db-nvim.binary")

  local db_root = util.get_paths().data_dir
  vim.fn.mkdir(db_root, "p")

  if type(M.config.vaults.default) == "string" then
    M.current_vault.name = M.config.vaults[M.config.vaults.default].name
    M.current_vault.dir = M.config.vaults[M.config.vaults.default].dir
  elseif type(M.config.vaults.default) == "table" then
    M.current_vault.name = M.config.vaults.default.name
    M.current_vault.dir = M.config.vaults.default.dir
  end

  M.current_vault.db = db_root .. "/" .. M.current_vault.name .. ".duckdb"

  M.update()

  vim.api.nvim_create_autocmd("BufWritePost", {
    pattern = "*.md",
    callback = function()
      M.update()
    end,
  })
end

local function run_bin(f, callback)
  local v1, v2, v3, v4 = pcall(f)


  if not v1 then
    vim.notify(v2, vim.log.levels.ERROR)
    return
  else
    if callback then
      callback(v2, v3, v4)
    else
      return v2, v3, v4
    end
  end
end

M.update = function()
  run_bin(function()
      return bin.update_index(M.current_vault.dir, M.current_vault.db)
    end,
    function(v)
      vim.notify(v, vim.log.levels.INFO)
    end
  )
end


M.find_notes = function()
  run_bin(function()
      return bin.get_notes(M.current_vault.dir, M.current_vault.db)
    end,
    function(notes)
      pick.from_notes(notes)
    end
  )
end

M.find_tags = function()
  run_bin(function()
      return bin.get_tags(M.current_vault.dir, M.current_vault.db)
    end,
    function(tags)
      pick.from_tags(tags)
    end
  )
end

M.find_links = function()
  local current_buf = vim.api.nvim_get_current_buf()
  local note_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]

  run_bin(function()
      return bin.get_backlinks(M.current_vault.dir, M.current_vault.db, note_id)
    end,
    function(notes)
      pick.from_notes(notes)
    end
  )
end

M.open = function(path, row, col)
  local buf = vim.fn.bufadd(path)
  vim.cmd(("buffer %d"):format(buf))

  local win = vim.api.nvim_get_current_win()

  if row then
    dd(row)
    vim.api.nvim_win_set_cursor(win, { row or 1, col or 0 })
  end
  vim.cmd("norm! zzzv")

end

M.get_cursor = function()

  local funs = {
    tag = M.find_tag_notes,
    note = M.open,
  }
  local current_buf = vim.api.nvim_get_current_buf()
  local current_win = vim.api.nvim_get_current_win()
  -- local note_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]
  local lines = vim.api.nvim_buf_get_lines(current_buf, 0, -1, false)
  local row, col = unpack(vim.api.nvim_win_get_cursor(current_win))

  run_bin(function()
      return bin.cursor_action(M.current_vault.dir, M.current_vault.db, lines, row, col)
    end,
    function(x, ...)
      funs[x](...)
    end
  )
end

M.find_tag_notes = function(tag)
  return run_bin(function()
      return bin.get_tag_notes(M.current_vault.dir, M.current_vault.db, tag)
    end,
    function(notes)
      pick.from_notes(notes)
    end
  )
end


return M
