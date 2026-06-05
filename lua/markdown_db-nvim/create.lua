local util = require "mesh-nvim.util"
local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"

local M = {}

local function create(opts)

  local content = nil
  if opts.viz then
    content = vim.split(opts.viz.selection, "\n", { plain = true })
  end

  vim.ui.input({ prompt = string.format("Enter %s id: ", opts.type) }, function(longName)
    if not longName or longName == "" then
      vim.notify("Aborted")
      return
    end

    local id = util.try(function()
      return bin.id_gen(longName)
    end)

    vim.ui.input({ prompt = string.format("Enter %s id: ", opts.type), default = id }, function(input)
      if not id or id == "" then
        vim.notify("Aborted")
        return
      end
      require("mesh-nvim.rfun").try(function()
        return bin.create_new(config.current_vault.dir, config.current_vault.db, opts.type, longName, input, opts.parent, content)
      end, true, opts.viz)
      util.try(function()
          return bin.update_index(config.current_vault.dir, config.current_vault.db)
        end,
        function(v)
          vim.notify(v, vim.log.levels.INFO)
        end
      )
    end)
  end)

end

function M.source()
  local current_buf = vim.api.nvim_get_current_buf()
  local parent_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]

  local viz
  if vim.endswith(vim.fn.mode():lower(), "v") then
    viz = util.get_visual_selection()
  end

  create({ type = "source", parent = parent_id, viz = viz })
end

function M.author()
  -- local current_buf = vim.api.nvim_get_current_buf()
  create({ type = "author" })
end

function M.journal()
  -- local current_buf = vim.api.nvim_get_current_buf()
  create({ type = "journal" })
end

function M.note()
  local current_buf = vim.api.nvim_get_current_buf()
  local parent_id = vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(current_buf)), "%.")[1]

  local viz
  if vim.endswith(vim.fn.mode():lower(), "v") then
    viz = util.get_visual_selection()
  end

  create({ type = "note", parent = parent_id, viz = viz })
end

return M
