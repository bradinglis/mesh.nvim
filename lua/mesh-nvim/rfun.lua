local find = require("mesh-nvim.find")
local util = require("mesh-nvim.util")

local M = {}

---@param message any
local function notify(message)
  vim.notify(message)
end

local funs = {
  tag = find.tag_notes,
  open = require("mesh-nvim.buf").open,
  log = notify,
  annotation = notify,
  err = notify,
  ---@param opts { path: string, contents: string[] }
  append = function(opts)
    local list = opts.contents
    local win = vim.api.nvim_get_current_win()
    local buf = vim.fn.bufadd(opts.path)
    vim.api.nvim_win_set_buf(win, buf)

    local lines = vim.api.nvim_buf_line_count(buf)
    vim.api.nvim_buf_set_lines(buf, -1, -1, false, list)
    vim.api.nvim_win_set_cursor(win, { lines + 1, 0 })
    vim.cmd("normal! zzzv")
  end,
  ---@param highlights { line: integer, start_col: integer, end_col: integer, hl: string }[]
  highlight = function(highlights)
    local ns_id = vim.api.nvim_create_namespace("mesh")
    vim.api.nvim_buf_clear_namespace(0, ns_id, 0, -1)

    for _, highlight in ipairs(highlights) do
      vim.api.nvim_buf_set_extmark(0, ns_id, highlight.line - 1, highlight.start_col, {
        end_row = highlight.line - 1,
        end_col = highlight.end_col,
        hl_group = highlight.hl,
      })
    end
  end,
}

---@param cmd string
---@param opts any
---@param update? boolean
---@param viz? any
function M.run(cmd, opts, update, viz)
  if update then
    require "mesh-nvim".update()
  end

  local run = funs[cmd]
  if not run then
    vim.notify(("mesh: unknown action '%s'"):format(cmd), vim.log.levels.ERROR)
    return
  end

  run(opts, viz)
end

---@param f fun(): any
---@param update? boolean
---@param viz? any
function M.try(f, update, viz)
  util.try(f, function(results)
    for _, value in ipairs(results) do
      M.run(value.run, value.opts, update, viz)
    end
  end)
end

return M
