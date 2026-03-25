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

  vim.api.nvim_create_autocmd("BufEnter", {
    pattern = "*.md",
    callback = function()
      M.highlights()
    end,
  })

  vim.api.nvim_create_autocmd("InsertLeave", {
    pattern = "*.md",
    callback = function()
      M.highlights()
    end,
  })

  vim.api.nvim_create_autocmd("BufWritePost", {
    pattern = "*.md",
    callback = function()
      M.update()
    end,
  })
end

local function run_bin(f, callback)
  local v1, v2, v3 = pcall(f)

  if not v1 then
    vim.notify(v2, vim.log.levels.ERROR)
    return
  else
    if callback then
      callback(v2, v3)
    else
      return v2, v3
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

M.highlights = function()
  local current_buf = vim.api.nvim_get_current_buf()
  local lines = vim.api.nvim_buf_get_lines(current_buf, 0, -1, false)

  run_bin(function()
      return bin.get_highlights(lines)
    end,
    function(highlights)
      local ns_id = vim.api.nvim_create_namespace("markdown_db")
      vim.api.nvim_buf_clear_namespace(current_buf, ns_id, 0, -1)

      for i, v in ipairs(highlights) do
        vim.api.nvim_buf_set_extmark(
          current_buf,
          ns_id,
          v.line - 1,
          v.start_col,
          {
            end_row = v.line - 1, -- End line (same as start for single line)
            end_col = v.end_col, -- End column (highlights characters from col 0 to 4)
            hl_group = v.hl,     -- The highlight group to use
          })
      end
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

M.find_tag_notes = function(tag)
  return run_bin(function()
      return bin.get_tag_notes(M.current_vault.dir, M.current_vault.db, tag)
    end,
    function(notes)
      pick.from_notes(notes)
    end
  )
end

M.open = function(opts, viz)
  local first_win = vim.api.nvim_get_current_win()
  local row, col = unpack(vim.api.nvim_win_get_cursor(first_win))

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

local funs = {
  tag = M.find_tag_notes,
  note = M.open,
}

M.get_cursor = function()
  local current_buf = vim.api.nvim_get_current_buf()
  local current_win = vim.api.nvim_get_current_win()
  -- local note_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]
  local lines = vim.api.nvim_buf_get_lines(current_buf, 0, -1, false)
  local row, col = unpack(vim.api.nvim_win_get_cursor(current_win))

  run_bin(function()
      return bin.cursor_action(M.current_vault.dir, M.current_vault.db, lines, row, col)
    end,
    function(x, y)
      funs[x](y)
    end
  )
end

M.create_source = function()
  local current_buf = vim.api.nvim_get_current_buf()

  local viz
  if vim.endswith(vim.fn.mode():lower(), "v") then
    viz = util.get_visual_selection()
  end

  local content = nil

  if viz then
    content = vim.split(viz.selection, "\n", { plain = true })
  end

  vim.ui.input({ prompt = "Enter source name: " }, function(longName)
    if not longName or longName == "" then
      vim.notify("Aborted")
      return
    end

    local id = run_bin(function()
      return bin.id_gen(longName)
    end)

    vim.ui.input({ prompt = "Enter source id: ", default = id }, function(input)
      local parent_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]
      run_bin(function()
          return bin.create_new(M.current_vault.dir, M.current_vault.db, longName, input, parent_id, content)
        end,
        function(x, opts)
          M.update()
          funs[x](opts, viz)
        end)
    end)
  end)

  run_bin(function()
      return bin.update_index(M.current_vault.dir, M.current_vault.db)
    end,
    function(v)
      vim.notify(v, vim.log.levels.INFO)
    end
  )
end

return M
