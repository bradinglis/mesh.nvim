local find = require("markdown_db-nvim.find")
local util = require("markdown_db-nvim.util")
local p = require "markdown_db-nvim"

local M = {}

local funs = {
  tag = find.tag_notes,
  open = require("markdown_db-nvim.buf").open,
  log = function(to_log)
    vim.notify(to_log)
  end,
  annotation = function(to_log)
    dd(to_log)
  end,
  append = function(opts)
    local list = opts.contents
    local win = vim.api.nvim_get_current_win()
    local buf = vim.fn.bufadd(opts.path)
    vim.cmd(("buffer %d"):format(buf))
    local lines = vim.api.nvim_buf_line_count(buf)
    vim.cmd("norm! zzzv")
    vim.api.nvim_buf_set_lines(buf, -1, -1, false, list)
    vim.api.nvim_win_set_cursor(win, { lines + 1, 0 })
  end,
  err = function(to_log)
    dd(to_log)
  end,
  highlight = function(highlights)
    local ns_id = vim.api.nvim_create_namespace("markdown_db")
    vim.api.nvim_buf_clear_namespace(0, ns_id, 0, -1)

    for _, v in ipairs(highlights) do
      vim.api.nvim_buf_set_extmark(
        0,
        ns_id,
        v.line - 1,
        v.start_col,
        {
          end_row = v.line - 1, -- End line (same as start for single line)
          end_col = v.end_col,  -- End column (highlights characters from col 0 to 4)
          hl_group = v.hl,      -- The highlight group to use
        })
    end
  end

}

local function run(cmd, opts, update, viz)
  if update then
    p.update()
  end

  funs[cmd](opts, viz)
end

function M.try(f, update, viz)
  util.try(f,
    function(x)
      for _, value in ipairs(x) do
        run(value.run, value.opts, update, viz)
      end
    end
  )
end

return M
