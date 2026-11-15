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

Reproduce with `tests/semantic_eval.lua` against a copy of the vault database (see the header of that file). Set A is the 10 approved queries above. Set B is 12 held-out queries taken from note titles, each expecting its own note; set B exists to check that tuning on A has not overfit.

Weight sweep (semantic x keyword x graph, `limit = 20`):

| Setting | A recall / MRR | B recall / MRR |
| --- | --- | --- |
| sem 0.4, kw 0.0, gr 0.0 | 10/10, 0.806 | 12/12, 0.944 |
| sem 0.6, kw 0.2, gr 0.0 (chosen) | 9/10, 0.800 | 12/12, 0.917 |
| sem 0.6, kw 0.2, gr 0.2 (previous default) | 9/10, 0.757 | 12/12, 0.854 |
| sem 0.6, kw 0.2, gr 0.4 | 8/10, 0.750 | 12/12, 0.845 |

Findings:

- **The graph term lowers quality at every setting we tried.** Without a context note, prompt search uses the top three semantic hits as pseudo-seeds, and their neighbours displace correct results. Prompt search therefore defaults to graph weight 0.
- **Keyword weight helps modestly** (A MRR 0.757 to 0.800 from kw 0 to 0.2). Higher keyword weight did not help.
- **Semantic-only (sem 0.4, kw 0, gr 0) has the best set A MRR (0.806)** but also drops the keyword signal entirely; the chosen setting differs by one query on set A and keeps keyword for exact terms. The difference is within the noise of 10 queries; we kept the keyword signal.
- **Note search is not measured.** Its default (semantic 0.6, graph 0.4) relies on links, which the query sets above do not exercise. It needs its own evaluation before it is trusted.

Latency and profile (copy of the zettel database, 3,975 chunks, warm):

| Component | Time |
| --- | --- |
| database open | ~20 ms |
| query embedding (ONNX) | ~10 ms |
| corpus load: cosine scan over all chunks, keyword matching, notes/tags/refs/anchors | ~35 ms without terms, ~60 ms with terms |
| scoring | ~1 ms |
| per-hit snippet and line refinement (20 hits) | ~40 ms |
| total, prompt search | ~150–230 ms |

The corpus scan is linear in the number of chunks. This is fine at this vault size, but a medium vault (around ten times larger) would make each search roughly a second. The likely fixes are caching vectors in memory with invalidation, or an ANN index. Both need a decision before Phase 6 can be called complete for a medium vault.
