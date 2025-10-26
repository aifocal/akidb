# CLAUDE.md

此文件為 Claude Code (claude.ai/code) 提供專案指引。

---

## 📊 專案狀態

```
Phase 1: Architecture & Setup           ████████████████████ 100% ✅
Phase 2: Core Implementation            ████████████████████ 100% ✅
Phase 3: Production Features            ████████████████████ 100% ✅
  ├─ M1: Benchmark Infrastructure       ████████████████████ 100% ✅
  ├─ M2: HNSW Index & Storage           ████████████████████ 100% ✅
  ├─ M3: hnsw_rs Migration Complete     ████████████████████ 100% ✅
  └─ M4: Production Monitoring          ░░░░░░░░░░░░░░░░░░░░   0% ⏳

Current Status: Phase 3 Complete - hnsw_rs Migration Successful
Tests: 159/159 passing (100%)
Library: hnsw_rs (2.86x faster than instant-distance on 100K vectors)
Performance: Expected P95 improvement from 165-173ms to 58-82ms @ 1M vectors
Next: Phase 4 - Production Deployment & Monitoring
```

**關鍵指標**:
- **程式碼**: ~2,500 行 (production) + ~1,500 行 (tests)
- **Crates**: 6 個 (4 core libraries + 2 services)
- **分支**: `feature/phase3-m2-hnsw-tuning`
- **HNSW Library**: hnsw_rs 0.3 (migrated from instant-distance)
- **HNSW 配置**: ef_search=200, ef_construction=400, M=16

---

## 🤖 AI 代理工作指南

### 編碼標準
- **語言**: Rust 2021, idiomatic patterns
- **錯誤處理**: 使用 `thiserror`, 生產環境禁用 `unwrap()`/`expect()`
- **非同步**: Tokio runtime, 所有 I/O 必須 async
- **測試**: 每個函數需要單元測試
- **提交**: 禁止提及 AI/Claude 輔助 (根據 global CLAUDE.md)

### 開發流程
1. **開始前**: 閱讀 `tmp/current-status-analysis.md` 了解最新狀態
2. **實作**: 編寫功能 + 測試
3. **驗證**: 執行 `./scripts/dev-test.sh`
4. **提交**: 使用 conventional commits (feat:, fix:, docs:, refactor:)

### 常見陷阱
- ❌ 不要使用 `FilterParser::parse_with_cache()` → 使用 `parse_with_collection()`
- ❌ 使用 `Arc<dyn Trait>` 時別忘記 import trait
- ✅ 始終為 trait objects 使用明確類型標註: `Arc<dyn MetadataStore>`
- ✅ 執行測試前先 `cargo check` 捕獲編譯錯誤

### 資料來源
- **實作狀態**: `tmp/current-status-analysis.md` (優先查看)
- **效能結果**: `tmp/phase3-m2-final-performance-report.md`
- **測試覆蓋**: `cargo test --workspace`
- **依賴**: `Cargo.toml` workspace section

### 快速參考

**關鍵檔案路徑**:
- Core types: `crates/akidb-core/src/collection.rs:5`
- Storage: `crates/akidb-storage/src/s3.rs:1`
- WAL: `crates/akidb-storage/src/wal.rs:1`
- HNSW index: `crates/akidb-index/src/hnsw.rs:1`
- Query engine: `crates/akidb-query/src/simple_engine.rs:19`
- REST API: `services/akidb-api/src/handlers/`
- Bootstrap: `services/akidb-api/src/bootstrap.rs`

**核心 Traits**:
- `StorageBackend`: `crates/akidb-storage/src/backend.rs:16`
- `IndexProvider`: `crates/akidb-index/src/provider.rs:10`

---

## 🎯 AkiDB - 分散式向量資料庫

Rust 編寫的分散式向量資料庫，使用 S3-compatible storage backend，專為高效能相似度搜尋設計。

### 當前功能 (Phase 3 完成)
- ✅ S3-native storage backend (create_collection, write_segment, manifest operations)
- ✅ HNSW index (L2, Cosine, Dot metrics) 使用 **hnsw_rs** (2.86x faster)
- ✅ WAL system (append, replay, crash recovery)
- ✅ SEGv1 binary format (Zstd compression + XXH3 checksums)
- ✅ Optimistic locking for concurrent manifest updates
- ✅ Full REST API (create, insert, search collections)
- ✅ Advanced filter pushdown (3-tier strategy based on selectivity)
- ✅ Batch query API with parallel execution
- ✅ 159/159 tests passing (100%)
- ✅ Production-ready code (zero warnings)

### 下一步 (Phase 4)
1. ⏳ Prometheus metrics & monitoring
2. ⏳ OpenTelemetry tracing
3. ⏳ Production deployment automation
4. ⏳ Performance benchmarking with hnsw_rs

---

## 🚀 快速開始

### 環境需求
- Rust 1.77+ (`rustup` recommended)
- Docker + Docker Compose
- Git

### 設定步驟

```bash
# 1. Clone 專案
git clone https://github.com/defai-digital/akidb.git
cd akidb
cp .env.example .env

# 2. 啟動開發環境 (MinIO + akidb-server)
./scripts/dev-init.sh

# 3. 執行測試驗證
./scripts/dev-test.sh
```

**服務端口**:
- MinIO S3 API: http://localhost:9000
- MinIO Console: http://localhost:9001 (akidb / akidbsecret)
- AkiDB API: http://localhost:8080

---

## 🏗️ 架構概覽

### Crate 結構

**核心函式庫 (`crates/`)**:
- `akidb-core` - 核心資料類型 (collections, segments, manifests)
- `akidb-storage` - 持久化層 (StorageBackend trait, S3, WAL, snapshots)
- `akidb-index` - ANN index providers (HNSW, brute-force)
- `akidb-query` - 查詢規劃與執行引擎
- `akidb-benchmarks` - Criterion.rs 效能測試

**服務 (`services/`)**:
- `akidb-api` - REST + gRPC API server (Axum + Tonic)
- `akidb-mcp` - 叢集管理 (membership, scheduler, balancer)

### 核心概念

- **Collection**: 向量資料集 (vector_dim, distance metric, payload_schema)
- **Segment**: 向量區塊 (Active → Sealed → Compacting → Archived)
- **Manifest**: Collection 元數據 (追蹤所有 segments 狀態)
- **StorageBackend**: 可插拔持久化層 (trait at `crates/akidb-storage/src/backend.rs:16`)
- **IndexProvider**: 可插拔 ANN index (trait at `crates/akidb-index/src/provider.rs:10`)

---

## 📝 開發命令

### 日常開發
```bash
./scripts/dev-test.sh              # 完整測試 + linting
cargo test --workspace             # 快速測試
cargo fmt --all                    # 格式化
cargo clippy --fix --workspace     # 自動修復警告
```

### 測試與除錯
```bash
# 單一測試
cargo test -p akidb-storage test_name

# 啟用日誌
RUST_LOG=debug cargo test test_name -- --nocapture

# 完整 backtrace
RUST_BACKTRACE=full cargo test test_name

# 檢查編譯
cargo check --workspace
```

### 效能測試
```bash
# 執行所有 benchmarks
cargo bench --package akidb-benchmarks

# 特定 benchmark
cargo bench --bench vector_search

# 查看結果
open target/criterion/report/index.html
```

### Docker 環境
```bash
./scripts/dev-init.sh              # 啟動環境
./scripts/dev-init.sh --force-recreate  # 強制重建
docker compose down -v             # 清除環境
docker compose logs -f akidb-server  # 查看日誌
```

### 環境變數

**必要設定** (`.env`):
```bash
# S3 Storage
AKIDB_S3_ENDPOINT=http://minio:9000
AKIDB_S3_BUCKET=akidb
AKIDB_S3_REGION=us-east-1
AKIDB_S3_ACCESS_KEY=akidb
AKIDB_S3_SECRET_KEY=akidbsecret

# API Server
AKIDB_BIND_ADDRESS=0.0.0.0:8080
AKIDB_PORT=8080

# Logging
RUST_LOG=info
```

---

## ⚙️ 重要實作細節

### Storage Layer
- **S3 Backend**: `crates/akidb-storage/src/s3.rs:1` (使用 `object_store` crate)
- **WAL**: `crates/akidb-storage/src/wal.rs:1` (append-only log)
- **Retry Logic**: 可配置的指數退避重試
- **Metadata Format**: Arrow IPC format
- **Bootstrap Recovery**: `services/akidb-api/src/bootstrap.rs`

### Data Types
- **Distance Metrics**: L2, Cosine, Dot (預設: Cosine)
- **Payload Types**: Boolean, Integer, Float, Text, Keyword, GeoPoint, Timestamp, Json
- **Segment States**: Active → Sealed → Compacting → Archived

### HNSW Configuration
- **Implementation**: hnsw_rs 0.3 library (`crates/akidb-index/src/hnsw.rs:1`)
- **Migration**: Switched from instant-distance to hnsw_rs (2.86x faster on 100K vectors)
- **Current Config**: ef_search=200, ef_construction=400, M=16
- **Expected Performance**: P95=58-82ms @ 1M vectors (50%+ improvement)
- **Filter Strategy**: 3-tier pushdown based on selectivity (<10%, 10-50%, >=50%)

### Query Execution
- **Flow**: QueryRequest → QueryPlanner → PhysicalPlan → ExecutionEngine → QueryResponse
- **Components**: `crates/akidb-query/src/`

---

## 📊 效能優化 (Phase 3)

### Benchmark Results

**Phase 2 Baseline (10K vectors)**:
- Cosine k=10: P50=0.69ms, P95=0.82ms, 1,450 QPS
- L2 k=10: P50=0.53ms, P95=0.57ms, 1,890 QPS

**Phase 3 M2 (1M vectors, k=50, instant-distance)**:
- L2: P50=166.8ms, P95=171.4ms, P99=~176ms, 5.9 QPS
- Cosine: P50=168.7ms, P95=173.5ms, P99=~180ms, 5.9 QPS
- Dot: P50=160.9ms, P95=165.6ms, P99=~185ms, 6.1 QPS

**Phase 3 M3 (hnsw_rs Migration Complete)**:
- ✅ Library: Migrated to hnsw_rs 0.3
- ✅ 100K PoC: 2.86x faster than instant-distance
- ✅ Expected @ 1M: P95=58-82ms (50%+ improvement)
- ✅ All 159 tests passing

### 待辦事項

**Phase 3 M3** (Complete):
- ✅ 100K PoC: hnsw_rs vs instant-distance (2.86x faster)
- ✅ Migration to hnsw_rs library
- ✅ All 159 tests passing
- ✅ Filter pushdown optimization
- ✅ Documentation updated

**Phase 4** (Next):
- ⏳ Performance benchmarking with hnsw_rs @ 1M vectors
- ⏳ Prometheus metrics & monitoring
- ⏳ OpenTelemetry tracing
- ⏳ Production deployment automation

---

## 🔧 疑難排解

### 編譯問題

**`Arc<dyn Trait>` 類型錯誤**:
```rust
// ❌ 錯誤 - 無法推斷類型
let metadata_store = Arc::new(MemoryMetadataStore::new());

// ✅ 正確 - 明確指定 trait object
let metadata_store: Arc<dyn MetadataStore> = Arc::new(MemoryMetadataStore::new());
```

**缺少 trait import**:
```rust
use akidb_storage::MetadataStore;  // 記得 import trait
```

### Docker 問題

**MinIO 連線失敗**:
```bash
docker compose ps                   # 檢查容器狀態
./scripts/dev-init.sh               # 重啟環境
curl http://localhost:9000/minio/health/live  # 驗證存取
```

### 測試失敗

**S3 錯誤**:
```bash
cp .env.example .env                # 確保環境變數正確
docker compose down -v              # 清除舊資料
./scripts/dev-init.sh               # 重新啟動
```

**啟用除錯日誌**:
```bash
RUST_LOG=debug cargo test -- --nocapture
RUST_BACKTRACE=full cargo test
```

---

## 🤝 AutomatosX 整合

此專案使用 [AutomatosX](https://github.com/defai-digital/automatosx) - AI agent 編排平台，具備持久記憶與多代理協作。

### 常用命令

```bash
# 列出可用 agents
ax list agents

# 執行 agent 任務
ax run backend "create a REST API"

# 搜尋記憶
ax memory search "keyword"

# 系統狀態
ax status
```

### 在 Claude Code 中使用

**自然語言 (推薦)**:
```
"請與 ax agent backend 協作實作使用者認證"
"請 ax security agent 審查程式碼漏洞"
```

**Slash 命令**:
```
/ax-agent backend, create a REST API
/ax-agent security, audit authentication
```

### 可用 Agents

- **backend** - Rust/Go/Python 後端開發
- **frontend** - React/Next.js 前端開發
- **security** - 安全稽核與威脅建模
- **quality** - QA 與測試
- **devops** - DevOps 與基礎設施
- **product** - 產品管理
- **cto** - 技術策略

完整列表: `ax list agents --format json`

### 核心功能

1. **持久記憶**: 自動保存所有對話與決策
2. **多代理協作**: Agents 自動委派任務
3. **跨 Provider 支援**: Claude, Gemini, OpenAI (自動 fallback)

### 配置

編輯 `automatosx.config.json`:
```json
{
  "providers": {
    "claude-code": {"enabled": true, "priority": 1},
    "gemini-cli": {"enabled": true, "priority": 2}
  },
  "execution": {
    "defaultTimeout": 1500000,
    "maxRetries": 3
  }
}
```

### 進階功能

```bash
# 平行執行
ax run product "Design auth system" --parallel

# 可恢復執行
ax run backend "Refactor codebase" --resumable

# 串流輸出
ax run backend "Explain codebase" --streaming
```

### 資源

- **文件**: https://github.com/defai-digital/automatosx
- **Agent 目錄**: `.automatosx/agents/`
- **Memory 資料庫**: `.automatosx/memory/memories.db`
- **Workspace**: `automatosx/PRD/`, `automatosx/tmp/`

---

## 📚 遷移指南

- **[Manifest V1 Migration](docs/migrations/manifest_v1.md)** - Atomic operations & optimistic locking
- **[Storage API Migration](docs/migration-guide.md)** - `write_segment_with_data` + SEGv1 format
- **[Index Providers Guide](docs/index-providers.md)** - Vector index implementation guide
