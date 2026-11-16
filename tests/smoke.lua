-- Headless smoke test for the mesh Rust endpoints. Run from the repository root:
--   nix develop --command nvim --headless -u NONE --cmd 'set rtp+=.' -c 'luafile tests/smoke.lua' -c qa
-- Exits non-zero if any check fails.

local root = vim.fn.tempname() .. "-mesh-smoke"
local notes = root .. "/notes"
local db = root .. "/mesh.duckdb"
vim.fn.mkdir(notes, "p")

local alpha = {
  "---",
  "id: Alpha",
  "aliases: Alpha",
  "type: note",
  "tags: [topic]",
  "---",
  "# Alpha",
  "See [[Beta]] and #topic.",
}
local beta = {
  "---",
  "id: Beta",
  "aliases: Beta",
  "type: note",
  "tags: []",
  "---",
  "# Beta",
  "Back to [[Alpha]].",
}
vim.fn.writefile(alpha, notes .. "/alpha.md")
vim.fn.writefile(beta, notes .. "/beta.md")

local bin = require "mesh-nvim.binary"
local failures = 0

local function check(name, fn)
  local ok, err = pcall(fn)
  if ok then
    print("ok    " .. name)
  else
    failures = failures + 1
    print("FAIL  " .. name .. ": " .. tostring(err))
  end
end

check("update_index returns actions", function()
  local actions = bin.update_index(notes, db)
  assert(type(actions) == "table" and actions[1] and actions[1].run, "no actions")
end)

check("thin notes lists both notes", function()
  assert(#bin.get_thin_notes(notes, db) == 2, "expected 2 notes")
end)

check("tags are indexed", function()
  assert(#bin.get_tags(notes, db) >= 1, "expected a tag")
end)

check("backlinks resolve", function()
  assert(type(bin.get_backlinks(notes, db, "Beta")) == "table", "backlinks not a table")
end)

check("cursor on [[Beta]] opens Beta at its heading row", function()
  -- Row is 1-based (nvim_win_get_cursor), col is 0-based; '[[' starts at byte 4 of line 8.
  local actions = bin.cursor_action(notes, db, alpha, 8, 5)
  assert(actions[1] and actions[1].run == "open", "expected an open action")
  assert(actions[1].opts.path:match("beta%.md$"), "wrong path: " .. tostring(actions[1].opts.path))
  assert(actions[1].opts.row == 7, "expected row 7 (# Beta), got " .. tostring(actions[1].opts.row))
end)

check("highlights cover the link and tag", function()
  local actions = bin.get_highlights(alpha)
  assert(actions[1] and actions[1].run == "highlight", "expected a highlight action")
  assert(#actions[1].opts >= 1, "expected at least one highlight")
end)

check("semantic items carry locations and kinds", function()
  local items = require("mesh-nvim.semantic").items({
    { note_id = "Alpha", anchor_id = "Alpha", title = "Alpha", heading = "Alpha", path = "/v/alpha.md", line = 8, start_col = 0, score = 0.5, snippet = "text", tags = { "marx" } },
    { note_id = "Beta", anchor_id = nil, title = "Beta", heading = "", path = "/v/beta.md", line = 1, start_col = 0, score = 0.2, snippet = "more", tags = {} },
  })
  assert(items[1].row == 8 and items[1].file == "/v/alpha.md" and items[1].kind == "section")
  assert(items[1].heading == "Alpha" and items[1].text:find("marx"), "heading or tags missing")
  assert(items[2].kind == "note" and items[2].heading == nil and items[2].row == 1)
end)

check("semantic picker opens the note at the hit line", function()
  local captured
  package.preload["snacks.picker"] = function()
    return { pick = function(options) captured = options end }
  end
  package.loaded["mesh-nvim.pick"] = nil
  local items = require("mesh-nvim.semantic").items({
    { note_id = "Alpha", anchor_id = "Alpha", title = "Alpha", heading = "Alpha", path = notes .. "/alpha.md", line = 8, start_col = 0, score = 0.5, snippet = "x", tags = {} },
  })
  require("mesh-nvim.pick").from_semantic(items, "Test")
  assert(captured and captured.title == "Test" and #captured.items == 1, "picker not called")
  assert(#captured.format(items[1], nil) >= 2, "format produced no chunks")
  local chunks = captured.format({
    kind = "section",
    title = string.rep("W", 60),
    heading = string.rep("h", 60),
    score = 0.5,
    snippet = "s",
  }, nil)
  assert(chunks[3][1]:match("…"), "long title not truncated with an ellipsis")
  assert(
    vim.fn.strdisplaywidth(chunks[3][1]) == 37,
    "title column is " .. vim.fn.strdisplaywidth(chunks[3][1]) .. " wide"
  )
  assert(
    vim.fn.strdisplaywidth(chunks[4][1]) == 29,
    "heading column is " .. vim.fn.strdisplaywidth(chunks[4][1]) .. " wide"
  )
  captured.confirm({ close = function() end }, items[1])
  assert(vim.api.nvim_buf_get_name(0):match("alpha%.md$"), "wrong buffer: " .. vim.api.nvim_buf_get_name(0))
  assert(vim.api.nvim_win_get_cursor(0)[1] == 8, "cursor not on line 8")
end)

-- Real model: downloads bge-small on first run (~130 MB). Opt in with MESH_SMOKE_MODEL=1.
if vim.env.MESH_SMOKE_MODEL == "1" then
  check("embeddings are written, then nothing changes", function()
    local calls = {}
    local first = bin.update_embeddings(notes, db, function(done, total)
      calls[#calls + 1] = { done, total }
    end)
    assert(first[1] and first[1].run == "log", "refresh failed: " .. vim.inspect(first))
    assert(first[1].opts:match("Embeddings: %d+ written"), first[1].opts)
    local last = calls[#calls]
    assert(last and last[2] > 0 and last[1] == last[2], "no progress: " .. vim.inspect(calls))
    local second = bin.update_embeddings(notes, db)
    assert(second[1].opts:match("Embeddings: 0 written, 0 removed"), second[1].opts)
  end)
  check("prompt search returns located hits", function()
    local hits = bin.semantic_search_prompt(notes, db, "pasta with garlic", nil, {})
    assert(#hits >= 1, "no hits")
    assert(hits[1].path and hits[1].line >= 1, "missing location")
    assert(hits[1].score > 0, "score not positive")
  end)
  check("note search excludes the source note", function()
    local hits = bin.semantic_search_note(notes, db, "Alpha", {})
    for _, hit in ipairs(hits) do
      assert(hit.note_id ~= "Alpha", "source note returned")
    end
  end)
end

vim.fn.delete(root, "rf")

if failures > 0 then
  print(failures .. " check(s) failed")
  os.exit(1)
end
print("smoke ok")
