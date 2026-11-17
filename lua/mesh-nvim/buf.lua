local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"

local M = {}

M.highlights = function()
  local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)

  require("mesh-nvim.rfun").try(function()
    return bin.get_highlights(lines)
  end)
end

---@param opts { path?: string, row?: integer, col?: integer, replace?: string }
---@param viz? { csrow: integer, cscol: integer, cerow: integer, cecol: integer }
M.open = function(opts, viz)
  if opts.replace then
    if viz and (viz.cecol == 999 or viz.cscol == 999) then
      vim.api.nvim_buf_set_lines(0, viz.csrow - 1, viz.cerow, false, { opts.replace })
    elseif viz then
      vim.api.nvim_buf_set_text(
        0,
        viz.csrow - 1,
        viz.cscol - 1,
        viz.cerow - 1,
        viz.cecol,
        { opts.replace }
      )
    else
      local cur_row = vim.api.nvim_win_get_cursor(0)[1]
      vim.api.nvim_buf_set_lines(0, cur_row - 1, cur_row - 1, false, { opts.replace })
    end
  end

  if not opts.path then
    return
  end

  local win = vim.api.nvim_get_current_win()
  local target = vim.fn.bufadd(opts.path)
  vim.api.nvim_win_set_buf(win, target)

  if opts.row then
    vim.api.nvim_win_set_cursor(win, { opts.row, opts.col or 0 })
  end
  vim.cmd("normal! zzzv")
end

M.get_cursor = function()
  local lines = vim.api.nvim_buf_get_lines(0, 0, -1, false)
  local row, col = unpack(vim.api.nvim_win_get_cursor(0))

  require("mesh-nvim.rfun").try(function()
    return bin.cursor_action(config.current_vault.dir, config.current_vault.db, lines, row, col)
  end)
end

return M
