local M = {}

local Source = {}
Source.__index = Source

function Source:get_trigger_characters()
  return { "#" }
end

function Source:enabled()
  return vim.bo.filetype == "markdown"
end

function Source:get_completions(ctx, callback)
  return self._get_completions(ctx, callback)
end

function Source:execute(_, _, callback, default_implementation)
  default_implementation()
  callback()
end

--- Wrap a `get_completions` function in the module shape blink.cmp expects.
---@param get_completions fun(ctx: table, callback: fun(response: table)): (fun()|nil)
---@return { new: fun(): table }
function M.module(get_completions)
  local source = setmetatable({ _get_completions = get_completions }, Source)
  return {
    new = function()
      return source
    end,
  }
end

--- Finish a completion request with the standard mesh flags.
---@param callback fun(response: table)
---@param items lsp.CompletionItem[]
---@return fun()
function M.respond(callback, items)
  callback({
    items = items,
    is_incomplete_backward = true,
    is_incomplete_forward = true,
  })
  return function() end
end

M.providers = {
  { name = "md_tags", module = "mesh-nvim.completion.tags" },
  { name = "md_links", module = "mesh-nvim.completion.links" },
  { name = "md_anchors", module = "mesh-nvim.completion.anchors" },
  { name = "md_frontmatter", module = "mesh-nvim.completion.frontmatter_refs" },
}

local function add_provider(blink, provider_name, provider_module)
  local add_source_provider = blink.add_source_provider or blink.add_provider
  add_source_provider(provider_name, {
    name = provider_name,
    module = provider_module,
    async = true,
    opts = {},
    enabled = function()
      return vim.bo.filetype == "markdown"
        and vim.bo.buftype ~= "prompt"
        and vim.b.completion ~= false
    end,
  })
end

function M.register_providers()
  local blink = require "blink.cmp"
  for _, provider in ipairs(M.providers) do
    add_provider(blink, provider.name, provider.module)
  end
end

local function add_providers(list)
  for _, provider in ipairs(M.providers) do
    if not vim.tbl_contains(list, provider.name) then
      table.insert(list, 1, provider.name)
    end
  end
end

local function log_unexpected_type(path, actual, expected)
  vim.notify(
    ("blink.cmp's `%s` should be %s, but is '%s'. mesh.nvim will not modify it."):format(
      path,
      expected,
      actual
    ),
    vim.log.levels.ERROR
  )
end

--- Inject mesh providers into a blink source list, preserving user entries.
---@param container table
---@param key string
---@return boolean whether the value was handled
local function inject_into(container, key)
  local value = container[key]
  local value_type = type(value)

  if value == nil then
    return false
  elseif value_type == "function" then
    container[key] = function()
      local result = value()
      add_providers(result)
      return result
    end
    return true
  elseif value_type == "table" and vim.islist(value) then
    add_providers(value)
    return true
  end

  log_unexpected_type(("sources.%s"):format(key), value_type, "a list or a function")
  return true
end

-- blink.cmp only exposes global source configuration, so inject once and let
-- the provider's own `enabled` callback gate markdown buffers.
function M.inject_sources()
  if M.injected_once then
    return
  end
  M.injected_once = true

  local blink_config = require "blink.cmp.config"
  if not vim.deep_equal(blink_config.sources.per_filetype, {}) then
    if inject_into(blink_config.sources.per_filetype, "markdown") then
      return
    end
  end

  inject_into(blink_config.sources, "default")
end

return M
