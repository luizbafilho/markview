# Search v2 launch plan

We are replacing the old **keyword search** with *hybrid ranking*: BM25 for exact matches, embeddings for everything else. The new index ships behind the `search_v2` flag and rolls out to 5%, 25%, then 100% of traffic. Questions go to [#search-v2](https://example.com).

## Milestones

| Milestone         | Owner  | Status         |  p95 latency | Target     |
|:------------------|:-------|:--------------:|-------------:|:-----------|
| Index builder     | Ana    | ✅ shipped     |       41 ms  | 2026-09-01 |
| Hybrid ranker     | Bruno  | ✅ shipped     |       68 ms  | 2026-09-15 |
| Query rewriting   | Chen   | 🚧 in review   |       74 ms  | 2026-10-01 |
| Rollout to 100%   | Dana   | ⏳ waiting     |          —   | 2026-10-20 |

- [x] Backfill 12M documents into the new index
- [x] Shadow traffic for two weeks, no regressions
- [ ] Dashboards for `ndcg@10` and zero-result rate
- [ ] Delete the legacy `KeywordSearch` service

> **Rollback is one flag.** Turning `search_v2` off sends every query back to the old index within a minute.

## How a query runs

```rust
pub async fn search(query: &str, index: &Index) -> Result<Vec<Hit>, SearchError> {
    let rewritten = rewrite(query).await?;
    let (exact, semantic) = tokio::join!(
        index.bm25(&rewritten, 50),
        index.knn(&embed(&rewritten).await?, 50),
    );
    // Reciprocal rank fusion: k = 60 keeps either list from dominating.
    Ok(fuse(exact?, semantic?, 60).into_iter().take(10).collect())
}
```

```typescript
export async function track(hit: Hit, position: number): Promise<void> {
  await fetch("/events", {
    method: "POST",
    body: JSON.stringify({ id: hit.id, position, ts: Date.now() }),
  });
}
```

```bash
# Flip the flag for 5% of traffic
flagctl set search_v2 --rollout 5 --env production
```

## Rollout

```mermaid
flowchart TD
    A[Deploy behind flag] --> B{Error rate under 0.1%?}
    B -->|yes| C[Raise rollout]
    B -->|no| D[Turn flag off]
    C --> E[100% of traffic]
```

```mermaid
sequenceDiagram
    participant Client
    participant Gateway
    participant Ranker
    participant Index
    Client->>Gateway: GET /search
    Gateway->>Ranker: query
    Ranker->>Index: BM25 + kNN
    Index-->>Ranker: 100 hits
    Ranker-->>Gateway: top 10
    Gateway-->>Client: 68 ms
```

## Schedule

```mermaid
gantt
    title Search v2
    dateFormat YYYY-MM-DD
    section Build
    Index builder     :done,    a1, 2026-08-18, 14d
    Hybrid ranker     :done,    a2, after a1, 14d
    section Launch
    Query rewriting   :active,  a3, after a2, 10d
    Staged rollout    :         a4, after a3, 14d
    section Cleanup
    Remove legacy     :crit,    a5, after a4, 5d
```

```mermaid
pie title Where query time goes
    "Embedding" : 38
    "kNN lookup" : 27
    "BM25" : 20
    "Rerank" : 15
```

## Offsite

The team celebrated the index backfill with a hike. Images render inline, at full resolution, in terminals that speak the kitty graphics protocol.

![Fjord from the trail](assets/photo.jpg)

---

*Last updated by the search team on 2026-09-23.*
