# AkiDB Performance Guide

**Last Updated**: 2025-10-23  
**Phase**: Phase 3 Week 0 (Milestone M1)  
**Status**: ✅ Benchmarking Foundation Complete

---

## 📖 概述

AkiDB 使用 **Criterion.rs** 進行性能測試,主要目標是:

- ✅ **捕獲基準**: 在優化前記錄 Phase 2 性能
- ✅ **測量改進**: 追蹤 Phase 3 優化效果
- ✅ **檢測退化**: CI/CD 中自動發現性能下降
- ✅ **指導決策**: 用數據驅動的洞察來指導調優

---

## 🚀 快速開始

### 運行所有 Benchmarks

```bash
# 運行完整 benchmark suite
./scripts/capture-baseline.sh

# 結果保存在: target/criterion/
# 查看 HTML 報告: open target/criterion/report/index.html
```

### 運行特定 Benchmark

```bash
# Vector search benchmarks
cargo bench --package akidb-benchmarks --bench vector_search

# Index build benchmarks
cargo bench --package akidb-benchmarks --bench index_build

# Metadata operations
cargo bench --package akidb-benchmarks --bench metadata_ops
```

---

## 📊 理解結果

### Criterion 輸出格式

```
vector_search/10k/k=10/cosine_with_filter/vector_search
    time:   [688.91 µs 693.20 µs 698.33 µs]
             ↑ 下界    ↑ 估計值  ↑ 上界
```

### 自定義指標

我們的 benchmarks 還會輸出:

```
vector_search/10k/k=10/cosine_with_filter =>
  p50=0.693ms p95=0.821ms p99=0.943ms
  throughput=1,450 QPS
  rss=57.8 GB
```

- **P50 (median)**: 50% 的查詢在此時間內完成
- **P95**: 95% 的查詢在此時間內完成
- **P99**: 99% 的查詢在此時間內完成 (tail latency)
- **Throughput**: 每秒查詢數 (QPS)
- **RSS**: 峰值記憶體使用量

---

## 📈 Phase 2 Baseline Metrics

**捕獲時間**: 2025-10-23  
**環境**: macOS ARM64 (darwin 25.0.0)  
**完整報告**: `tmp/PHASE2-BASELINE-METRICS.md`

### Vector Search (10K Vectors, 128-dim)

#### Cosine Distance with Metadata Filter

| Top-K | P50 Latency | P95 Latency | P99 Latency | Throughput | Peak RSS |
|-------|-------------|-------------|-------------|------------|----------|
| 10    | 0.69 ms     | 0.82 ms     | 0.94 ms     | 1,450 QPS  | 57.8 GB  |
| 50    | 0.94 ms     | 1.09 ms     | 1.19 ms     | 1,060 QPS  | 70.4 GB  |
| 100   | 1.28 ms     | 1.45 ms     | 1.54 ms     | 785 QPS    | 72.5 GB  |

#### L2 Distance with Metadata Filter

| Top-K | P50 Latency | P95 Latency | P99 Latency | Throughput | Peak RSS |
|-------|-------------|-------------|-------------|------------|----------|
| 10    | 0.53 ms     | 0.57 ms     | 0.62 ms     | 1,890 QPS  | 73.1 GB  |

### 關鍵洞察

- ✅ **L2 快 23%**: L2 比 Cosine 快 (0.53ms vs 0.69ms)
- ✅ **Sub-millisecond P50**: 所有 k=10 場景都在 1ms 以下
- ✅ **可預測的 tail latency**: P99 < 1ms
- ⚠️ **記憶體使用高**: 70+ GB for 10K vectors (需調查)

---

## 🎯 Phase 3 優化目標

### M1: Benchmarking Foundation ✅ 完成
- ✅ Criterion harness 實現
- ✅ Phase 2 baseline 捕獲
- ✅ Performance guide (本文檔)

### M2: HNSW Index Tuning (Week 1-2) ⏳ 下一步

**目標改進**:
- **P95 latency**: ≤150ms (1M vectors, k=50)
- **P99 latency**: ≤250ms (1M vectors, k=50)
- **Throughput**: +20% vs Phase 2
- **Index rebuild**: -15% 時間

**如何追蹤**:
```bash
# M2 優化前後比較
cargo bench --package akidb-benchmarks --bench vector_search -- 1m/k=50
```

---

## 📝 Troubleshooting

### 常見問題

**Q: Benchmarks 報 "out of memory"**  
A: 減少 dataset size 或增加系統記憶體。1M vectors (128-dim) 至少需要 ~2GB RAM。

**Q: 結果變異性很高**  
A: 關閉背景程序,禁用 CPU 頻率調整:
```bash
# macOS
sudo systemsetup -setcomputersleep Never
```

---

## 📚 參考資料

- **Criterion.rs**: https://bheisler.github.io/criterion.rs/book/
- **Phase 3 PRD**: `tmp/PHASE-3-PRD.md`
- **Baseline Report**: `tmp/PHASE2-BASELINE-METRICS.md`
- **Benchmark 代碼**: `crates/akidb-benchmarks/`

### Migration Guides

- **[Manifest V1 Migration](migrations/manifest_v1.md)** - Atomic manifest operations and optimistic locking for concurrent writes
- **[Storage API Migration](migration-guide.md)** - Migrating from `write_segment` to `write_segment_with_data` with SEGv1 format
- **[Index Providers Guide](index-providers.md)** - Vector index implementation guide and contract testing

---

**下一步**:
1. 查看 Phase 2 baselines (`tmp/PHASE2-BASELINE-METRICS.md`)
2. 確定 M2 (HNSW tuning) 的優化目標
3. 運行 focused benchmarks 驗證假設
4. 對比 Phase 3 success metrics
