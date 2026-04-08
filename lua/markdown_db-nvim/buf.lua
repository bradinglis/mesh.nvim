local util = require "markdown_db-nvim.util"
local bin = require "markdown_db-nvim.binary"
local config = require "markdown_db-nvim.config"

local M = {}

M.highlights = function()
  local current_buf = vim.api.nvim_get_current_buf()
  local lines = vim.api.nvim_buf_get_lines(current_buf, 0, -1, false)

  require("markdown_db-nvim.rfun").try(function()
    -- return bin.cursor_action(config.current_vault.dir, config.current_vault.db, lines, row, col)
    return bin.get_highlights(lines)
  end)

  -- util.try(function()
  --     return bin.get_highlights(lines)
  --   end,
  --   function(highlights)
  --     dd(highlights)
  --     local ns_id = vim.api.nvim_create_namespace("markdown_db")
  --     vim.api.nvim_buf_clear_namespace(current_buf, ns_id, 0, -1)
  --
  --     for _, v in ipairs(highlights) do
  --       vim.api.nvim_buf_set_extmark(
  --         current_buf,
  --         ns_id,
  --         v.line - 1,
  --         v.start_col,
  --         {
  --           end_row = v.line - 1, -- End line (same as start for single line)
  --           end_col = v.end_col,  -- End column (highlights characters from col 0 to 4)
  --           hl_group = v.hl,      -- The highlight group to use
  --         })
  --     end
  --   end
  -- )
end

M.open = function(opts, viz)
  if opts.replace then
    if viz then
      if viz.cecol == 999 or viz.cscol == 999 then
        vim.api.nvim_buf_set_lines(0, viz.csrow - 1, viz.cerow, false, { opts.replace })
      else
        vim.api.nvim_buf_set_text(0, viz.csrow - 1, viz.cscol - 1, viz.cerow - 1, viz.cecol, { opts.replace })
      end
    else
      local cur_row, _ = unpack(vim.api.nvim_win_get_cursor(0))
      vim.api.nvim_buf_set_lines(0, cur_row - 1, cur_row - 1, false, { opts.replace })
    end
  end

  if opts.path then
    local buf = vim.fn.bufadd(opts.path)
    vim.cmd(("buffer %d"):format(buf))
    local win = vim.api.nvim_get_current_win()
    if opts.row then
      vim.api.nvim_win_set_cursor(win, { opts.row or 1, opts.col or 0 })
    end
    vim.cmd("norm! zzzv")
  end
end


M.get_cursor = function()
  local current_buf = vim.api.nvim_get_current_buf()
  local current_win = vim.api.nvim_get_current_win()
  local lines = vim.api.nvim_buf_get_lines(current_buf, 0, -1, false)
  local row, col = unpack(vim.api.nvim_win_get_cursor(current_win))

  require("markdown_db-nvim.rfun").try(function()
    return bin.cursor_action(config.current_vault.dir, config.current_vault.db, lines, row, col)
  end)
end

return M
