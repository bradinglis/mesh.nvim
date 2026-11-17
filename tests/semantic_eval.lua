-- Semantic search evaluation. Not part of the default smoke run.
--
--   MESH_EVAL_NOTES=~/zettel MESH_EVAL_DB=/path/to/copy.duckdb \
--     nix develop --command nvim --headless -u NONE --cmd 'set rtp+=.' \
--     -c 'luafile tests/semantic_eval.lua' -c qa
--
-- Run it against a COPY of the vault database: searching needs embeddings, so
-- run a refresh first (update_embeddings), which writes to the database.
--
-- Set A: hand-written queries with expected notes (docs/semantic-eval.md).
-- Set B: held-out queries taken from note titles, each expecting its own note.
-- Set MESH_EVAL_SWEEP=1 to also grid-search the weights (slow).

local bin = require "mesh-nvim.binary"

local notes = vim.env.MESH_EVAL_NOTES
local db = vim.env.MESH_EVAL_DB
if not notes or not db then
  print("set MESH_EVAL_NOTES and MESH_EVAL_DB (use a copy of the database)")
  os.exit(2)
end

local set_a = {
  { "how did Marx's theory of value account for reification", "s_rubin_essays-marxs-theory-value_reification-production-relations-among-people-personification-things" },
  { "relative surplus population and capitalist crisis", "s_mau_mute-compulsion_surplus-population-crisis" },
  { "Foucault on care of the self and ethics", "foucault-care-self" },
  { "genealogy as a critique of origins", "foucault-genealogy" },
  { "Hegel's account of sense certainty and the immediate", "s_hegel_phenomenology-spirit_consciousness_sense-certainty" },
  { "Hegel doctrine of essence and actuality", "s_hegel_science-logic_essence" },
  { "monetary inflation and the magic of money", "s_mattick_jr_return-inflation_from-great-inflation-magic-money" },
  { "mass movements and the rise of fascism", "s_rosenberg_fascism-mass-movement" },
  { "Derrida on writing and dissemination", "s_derrida_dissemination" },
  { "expressionism in art and the Bloch debate", "s_rose_marxist-modernism_bloch-expressionism" },
}

-- Titles of non-author notes, every fourth by id, capped at 12. Deterministic,
-- so the set is the same on every run against the same database.
local set_b = {}
local thin = bin.get_thin_notes(notes, db)
table.sort(thin, function(a, b) return a.id < b.id end)
for index, note in ipairs(thin) do
  if #set_b < 12 and index % 4 == 0 and note.type ~= "author" and note.title and #note.title >= 12 then
    set_b[#set_b + 1] = { note.title, note.id }
  end
end

local function rank_of(hits, expected)
  local seen, rank = {}, 0
  for _, hit in ipairs(hits) do
    if not seen[hit.note_id] then
      seen[hit.note_id] = true
      rank = rank + 1
      if hit.note_id == expected then
        return rank
      end
    end
  end
  return nil
end

local function evaluate(set, weights)
  local found, reciprocal, total_ms = 0, 0, 0
  for _, query in ipairs(set) do
    local started = vim.uv.hrtime()
    local hits = bin.semantic_search_prompt(notes, db, query[1], nil, { limit = 20, weights = weights })
    total_ms = total_ms + (vim.uv.hrtime() - started) / 1e6
    local rank = rank_of(hits, query[2])
    if rank then
      found = found + 1
      reciprocal = reciprocal + 1 / rank
    end
  end
  return found, reciprocal / #set, total_ms / #set
end

local found_a, mrr_a, ms_a = evaluate(set_a, nil)
local found_b, mrr_b, ms_b = evaluate(set_b, nil)
print(("A  recall@20 %d/%d  mrr %.3f  mean %.0f ms"):format(found_a, #set_a, mrr_a, ms_a))
print(("B  recall@20 %d/%d  mrr %.3f  mean %.0f ms"):format(found_b, #set_b, mrr_b, ms_b))

if vim.env.MESH_EVAL_SWEEP == "1" then
  for _, semantic in ipairs({ 0.4, 0.6, 0.8 }) do
    for _, keyword in ipairs({ 0.0, 0.2, 0.4 }) do
      for _, graph in ipairs({ 0.0, 0.2, 0.4 }) do
        local weights = { semantic = semantic, keyword = keyword, graph = graph }
        local fa, ra = evaluate(set_a, weights)
        local fb, rb = evaluate(set_b, weights)
        print(("SWEEP sem=%.1f kw=%.1f gr=%.1f  A %d/%d %.3f  B %d/%d %.3f"):format(
          semantic, keyword, graph, fa, #set_a, ra, fb, #set_b, rb))
      end
    end
  end
end
