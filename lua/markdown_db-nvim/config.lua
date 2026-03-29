local util = require "markdown_db-nvim.util"


local M = {}
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

function M.set_opts(opts)
  M.config = vim.tbl_extend("force", M.config, opts)

  local db_root = util.get_paths().data_dir
  vim.fn.mkdir(db_root, "p")

  if type(M.config.vaults.default) == "string" then
    M.current_vault.name = M.config.vaults[M.config.vaults.default].name
    M.current_vault.dir = vim.fs.abspath(M.config.vaults[M.config.vaults.default].dir)
  elseif type(M.config.vaults.default) == "table" then
    M.current_vault.name = M.config.vaults.default.name
    M.current_vault.dir = vim.fs.abspath(M.config.vaults.default.dir)
  end
  M.current_vault.db = db_root .. "/" .. M.current_vault.name .. ".duckdb"
end

return M
