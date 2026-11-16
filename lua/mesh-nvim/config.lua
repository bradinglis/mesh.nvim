local util = require "mesh-nvim.util"

local M = {}

---@class vault_config
---@field name string
---@field dir string
---@field db? string

---@class opts
---@field vaults table<string, string|vault_config>

---@type opts
M.config = {
  vaults = {
    default = "none",
  },
  semantic = {
    enabled = true,
    limit = 20,
    group = "mixed",
    refine_line = true,
    graph_seeds = true,
    weights = nil,
    max_per_note = 2,
    max_per_author = 2,
    high_score = 0.85,
  },
}

---@type vault_config
M.current_vault = {
  name = "none",
  dir = "",
  db = "",
}

---@param opts? opts
function M.set_opts(opts)
  local defaults = M.config.semantic
  M.config = vim.tbl_extend("force", M.config, opts or {})
  M.config.semantic = vim.tbl_deep_extend("force", defaults, (opts and opts.semantic) or {})

  local db_root = util.get_paths().data_dir
  vim.fn.mkdir(db_root, "p")

  local default = M.config.vaults.default
  if type(default) == "string" then
    local vault = M.config.vaults[default]
    if type(vault) == "table" then
      M.set_vault(vault.name or default, vault.dir)
    elseif type(vault) == "string" then
      M.set_vault(default, vault)
    else
      vim.notify(
        ("mesh.nvim: unknown default vault '%s'"):format(default),
        vim.log.levels.ERROR
      )
    end
  elseif type(default) == "table" then
    M.set_vault(default.name, default.dir)
  end
end

---@param name string
---@param dir string
function M.set_vault(name, dir)
  local db_root = util.get_paths().data_dir
  vim.fn.mkdir(db_root, "p")

  M.current_vault.name = name
  M.current_vault.dir = vim.fs.abspath(dir)
  M.current_vault.db = vim.fs.joinpath(db_root, name .. ".duckdb")
end

return M
