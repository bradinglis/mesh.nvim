# Semantic search evaluation (Phase 4)

Run: full pipeline (`semantic_search_prompt`), default weights 0.6 / 0.2 / 0.2, BGE-small via ONNX, `limit = 20`, on a copy of `~/.local/share/nvf/mesh/zettel.duckdb` against `~/zettel` (691 notes). The live database was not modified.

Rank is the position of the expected note among distinct notes in the hit list; `-` means not in the top 20. Queries are the 10 drafts approved in `docs/spike-results.md`.

| Rank | Latency | Query |
| --- | --- | --- |
| - | 173 ms | how did Marx's theory of value account for reification |
| 1 | 167 ms | relative surplus population and capitalist crisis |
| 1 | 165 ms | Foucault on care of the self and ethics |
| 1 | 166 ms | genealogy as a critique of origins |
| 2 | 186 ms | Hegel's account of sense certainty and the immediate |
| 14 | 157 ms | Hegel doctrine of essence and actuality |
| 1 | 162 ms | monetary inflation and the magic of money |
| 1 | 162 ms | mass movements and the rise of fascism |
| 1 | 150 ms | Derrida on writing and dissemination |
| 1 | 162 ms | expressionism in art and the Bloch debate |

**Recall@20 (notes): 9/10.** The semantic-only spike found 8/10 on the same queries at chunk level. Derrida is now found; the two misses are candidates for weight tuning.

Refresh: 3,975 chunks embedded in 420 s (gate: 10 min).

Known gaps, not yet addressed:

- Weights are the defaults and have not been tuned. Phase 6 should sweep them against these queries plus more.
- "Marx reification" misses entirely. The likely cause is that the note title and body use different vocabulary from the query; keyword matching on the query terms does not reach it.
- Line targeting (Stage 3) is keyword-only and was not scored in this run.

## Phase 6: tuning and profiling

Reproduce with `tests/semantic_eval.lua` against a copy of the vault database (see the header of that file). Set A is the 10 approved queries above. Set B is 12 held-out queries taken from note titles, each expecting its own note; the script picks every fourth non-author note by id with a title of at least 12 characters, so the set is reproducible per database and no note titles are stored in the repository. (An earlier run selected titles with `order by hash(note_id)`; that selection overlaps the current one in 1 note of 12, so numbers from the two are not comparable.)

### Chunk context fix (parent title)

The first tuning pass left one set A query outside the top 20: "how did Marx's theory of value account for reification" placed the target note at rank 22. The target is a chapter of *Essay on Marx's Theory of Value*, but its section chunks carried only `note title > heading`, and no chunk of the chapter contains the words "Marx", "theory", or "account". Keyword matching therefore put sibling chapters of the same book ahead of it, even though its semantic score was competitive.

Chunk text now includes the parent note title (from `refs.kind = 'parent'`) in the note-level header and in each section prefix. The effect:

| | Before | After |
| --- | --- | --- |
| Marx reification query, target rank | 22 | 1 |
| Set A recall / MRR | 9/10, 0.800 | 10/10, 0.863 |
| Set B recall / MRR | 12/12, 0.917 | 12/12, 0.917 |

The refresh re-embeds every chunk whose text changed; on this vault that was 225 s. The `text_hash` check makes this automatic.

The numbers above and in the sweep below predate the result caps (`max_per_note` / `max_per_author`). Re-measured with the caps, set A is 10/10 at MRR 0.867 and set B is 12/12 at 0.917, i.e. unchanged within one rank step. The caps matter for list quality rather than these metrics: the Marx query now returns two chapters of the target's book plus the book note, instead of a run of sibling chapters from the same author.

### Weight sweep after the chunking fix

| Setting | A recall / MRR | B recall / MRR |
| --- | --- | --- |
| sem 0.8, kw 0.2, gr 0.0 | 10/10, 0.864 | 12/12, 0.917 |
| sem 0.6, kw 0.2, gr 0.0 (chosen) | 10/10, 0.863 | 12/12, 0.917 |
| sem 0.6, kw 0.2, gr 0.2 (previous default) | 10/10, 0.861 | 12/12, 0.958 |
| sem 0.6, kw 0.2, gr 0.4 | 9/10, 0.850 | 12/12, 0.958 |
| sem 0.6, kw 0.0, gr 0.0 | 10/10, 0.856 | 12/12, 0.917 |
| sem 0.6, kw 0.4, gr 0.0 | 10/10, 0.796 | 12/12, 0.917 |

Findings:

- **The parent title in chunk context was the largest quality fix so far.** It fixed the only remaining set A miss without changing set B.
- **Graph is no longer clearly harmful.** After re-chunking, graph weight 0.2 keeps 10/10 on A and 12/12 on B, with A MRR 0.861 versus 0.863 and B MRR 0.958 versus 0.917. That is a difference of one rank step on A and one query's rank on B, i.e. within the noise of 22 queries. Prompt search keeps graph weight 0 because pseudo-seeds remain the least predictable part of the score; note search keeps it because links are its main relationship signal. A larger golden set would be needed to justify re-enabling it for prompts.
- **Keyword 0.2 is still the best setting.** Raising it to 0.4 lowers set A MRR from 0.863 to 0.796.
- **Semantic and keyword ratio:** 0.8/0.2 and 0.6/0.2 are equivalent after normalisation (0.864 versus 0.863).
- **Note search is not measured.** Its default (semantic 0.6, keyword 0, graph 0.4) relies on links, which the query sets above do not exercise. It needs its own evaluation before it is trusted.

Latency and profile (copy of the zettel database, 3,975 chunks, warm):

| Component | Time |
| --- | --- |
| database open | ~20 ms |
| query embedding (ONNX) | ~10 ms |
| corpus load: cosine scan over all chunks, keyword matching, notes/tags/refs/anchors | ~35 ms without terms, ~60 ms with terms |
| scoring | ~1 ms |
| per-hit snippet and line refinement (20 hits) | ~40 ms |
| total, prompt search | ~160-230 ms |

The corpus scan is linear in the number of chunks. This is fine at this vault size, but a medium vault (around ten times larger) would make each search roughly a second. The likely fixes are caching vectors in memory with invalidation, or an ANN index. This is deliberately deferred: there is no larger vault to measure against yet. Revisit it before advertising the feature for medium or large vaults.
