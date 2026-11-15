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
