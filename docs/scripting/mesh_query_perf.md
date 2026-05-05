# Mesh Query Perf Snapshot

This page records mesh-query benchmark results.

## Big Takeaway

- In this benchmark run, a mesh with about `50,000,000` vertices queried in around `~5us` (`time_to_query_us`).
- Most projects will never query a single mesh this large.
- This shows the strength of Perro mesh queries on very large topology.

## What `time_to_query_us` Means

- Time for one mesh query call, in microseconds.
- It does not include full test runtime overhead (mesh build + repeated sampling).

## Test Output Shape

```text
running tests w/ vertices=<count>, triangles=<count>
surfaces,vertices,triangles,time_to_query_us
```

Surface lanes tested:

- `1`
- `4`
- `16`
- `64`
- `256`

## Re-run Command

```bash
cargo test -p perro_runtime mesh_query::tests::bench_mesh_query_fixed_vertex_count_latency -- --ignored --nocapture
```
