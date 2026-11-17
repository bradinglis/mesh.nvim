-- Semantic search front end: prompt/note entry points and the picker data.
-- Nothing at the top level needs snacks.nvim, so this module loads headless.
local bin = require "mesh-nvim.binary"
local config = require "mesh-nvim.config"
local util = require "mesh-nvim.util"

local M = {}

local SNIPPET_CHARS = 90

---@param text string?
---@return string
local function truncate(text, limit)
  text = text or ""
  if vim.fn.strchars(text) <= limit then
    return text
  end
  return vim.fn.strcharpart(text, 0, limit) .. "…"
end

--- Convert Rust hits into picker items. Pure data transform.
---@param hits table[]
---@return table[]
function M.items(hits)
  local items = {}
  for index, hit in ipairs(hits) do
    local heading = hit.heading ~= "" and hit.heading or nil
    local label = heading and (hit.title .. " > " .. heading) or hit.title
    local tags = hit.tags or {}
    items[#items + 1] = {
      idx = index,
      text = label .. " " .. table.concat(tags, " ") .. " " .. (hit.snippet or ""),
      score = hit.score or 0,
      title = hit.title,
      heading = heading,
      kind = hit.anchor_id and "section" or "note",
      snippet = truncate(hit.snippet, SNIPPET_CHARS),
      tags = tags,
      note_id = hit.note_id,
      anchor_id = hit.anchor_id,
      file = hit.path,
      path = hit.path,
      row = hit.line,
      col = hit.start_col or 0,
      pos = { hit.line, hit.start_col or 0 },
      semantic = hit.semantic,
      keyword = hit.keyword,
      graph = hit.graph,
    }
  end
  return items
end

---@param opts? table
---@return table
local function options(opts)
  local semantic = config.config.semantic or {}
  return vim.tbl_deep_extend("force", {
    limit = semantic.limit,
    group = semantic.group,
    refine_line = semantic.refine_line,
    graph_seeds = semantic.graph_seeds,
    weights = semantic.weights,
    max_per_note = semantic.max_per_note,
    max_per_author = semantic.max_per_author,
    high_score = semantic.high_score,
  }, opts or {})
end

local function enabled()
  local semantic = config.config.semantic or {}
  if semantic.enabled == false then
    vim.notify("mesh: semantic search is disabled in setup()", vim.log.levels.WARN)
    return false
  end
  return true
end

--- The note id of the current buffer (file basename without extension).
---@return string
local function current_note_id()
  return vim.split(vim.fs.basename(vim.api.nvim_buf_get_name(0)), "%.")[1]
end

---@param hits table[]
---@param title string
local function show(hits, title)
  if #hits == 0 then
    vim.notify("mesh: no matches", vim.log.levels.INFO)
    return
  end
  require("mesh-nvim.pick").from_semantic(M.items(hits), title)
end

---@param prompt string?
---@param opts? table  `context_note = true` uses the current buffer's note
function M.search(prompt, opts)
  if not enabled() then
    return
  end
  opts = options(opts)
  if opts.context_note == true then
    opts.context_note = current_note_id()
  end
  local context = opts.context_note
  opts.context_note = nil

  local function run(text)
    local dir, db = config.current_vault.dir, config.current_vault.db
    util.try(function()
      return bin.semantic_search_prompt(dir, db, text, context, opts)
    end, function(hits)
      show(hits, "Semantic: " .. text)
    end)
  end

  if prompt and prompt ~= "" then
    run(prompt)
    return
  end
  vim.ui.input({ prompt = "Semantic search: " }, function(input)
    if input and input ~= "" then
      run(input)
    end
  end)
end

---@param note_id? string defaults to the current buffer's note
---@param opts? table
function M.search_note(note_id, opts)
  if not enabled() then
    return
  end
  note_id = note_id or current_note_id()
  opts = options(opts)
  local dir, db = config.current_vault.dir, config.current_vault.db
  util.try(function()
    return bin.semantic_search_note(dir, db, note_id, opts)
  end, function(hits)
    show(hits, "Related to " .. note_id)
  end)
end

return M
