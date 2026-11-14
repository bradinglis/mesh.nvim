local util = require "mesh-nvim.util"
local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"

local M = {}

---@return string
local function current_note_id()
  return vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(0)), "%.")[1]
end

---@return table?
local function visual_selection()
  if vim.endswith(vim.fn.mode():lower(), "v") then
    return util.get_visual_selection()
  end
end

---@param opts { type: string, parent?: string, viz?: table }
local function create(opts)
  local content = opts.viz and vim.split(opts.viz.selection, "\n", { plain = true }) or nil

  vim.ui.input({ prompt = ("Enter %s id: "):format(opts.type) }, function(long_name)
    if not long_name or long_name == "" then
      vim.notify("Aborted")
      return
    end

    local suggested_id = util.try(function()
      return bin.id_gen(long_name)
    end)

    vim.ui.input(
      { prompt = ("Enter %s id: "):format(opts.type), default = suggested_id },
      function(input)
        if not input or input == "" then
          vim.notify("Aborted")
          return
        end

        require("mesh-nvim.rfun").try(function()
          return bin.create_new(
            config.current_vault.dir,
            config.current_vault.db,
            opts.type,
            long_name,
            input,
            opts.parent,
            content
          )
        end, true, opts.viz)

        util.try(function()
          return bin.update_index(config.current_vault.dir, config.current_vault.db)
        end)
      end
    )
  end)
end

function M.source()
  create({ type = "source", parent = current_note_id(), viz = visual_selection() })
end

function M.author()
  create({ type = "author" })
end

function M.journal()
  create({ type = "journal" })
end

function M.note()
  create({ type = "note", parent = current_note_id(), viz = visual_selection() })
end

return M
