local util = require "markdown_db-nvim.util"
local create = require "markdown_db-nvim.create"
local buf = require "markdown_db-nvim.buf"
local bin = require "markdown_db-nvim.binary"
local find = require "markdown_db-nvim.find"
local config = require "markdown_db-nvim.config"
local comp = require "markdown_db-nvim.completion"

local M = {}

M.setup = function(opts)
  config.set_opts(opts)

  M.update()
  comp.register_providers()

  vim.api.nvim_create_autocmd("BufEnter", {
    pattern = "*.md",
    callback = function()
      comp.inject_sources()
      M.buf.highlights()
    end,
  })

  vim.api.nvim_create_autocmd("InsertLeave", {
    pattern = "*.md",
    callback = function()
      M.buf.highlights()
    end,
  })

  vim.api.nvim_create_autocmd("BufWritePost", {
    pattern = "*.md",
    callback = function()
      M.update()
    end,
  })
end

M.update = function()
  util.try(function()
      return bin.update_index(config.current_vault.dir, config.current_vault.db)
    end,
    function(v)
      vim.notify(v, vim.log.levels.INFO)
    end
  )
end

M.create = create
M.find = find
M.buf = buf
M.open_asset = function()
  local asset_path = config.current_vault.dir .. "/annotated/" .. vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(0)), "%.")[1]

  if vim.fn.filereadable(asset_path .. ".pdf") == 1 then
    asset_path = asset_path .. ".pdf"
  elseif vim.fn.filereadable(asset_path .. ".epub") == 1 then
    asset_path = asset_path .. ".epub"
  else
    vim.notify("no file")
  end

  vim.fn.jobstart({"lektra", asset_path})


end

return M
