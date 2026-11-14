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

vim.fn.delete(root, "rf")

if failures > 0 then
  print(failures .. " check(s) failed")
  os.exit(1)
end
print("smoke ok")
