# AkiDB Autoscaling Architecture

AkiDB 的自動伸縮架構設計，整合 MinIO Server Pool 機制，實現彈性部署。

---

## 設計原則

1. **AkiDB 無狀態**: 可自動水平伸縮，節點可動態加入/移除
2. **MinIO 受控伸縮**: 透過 Server Pool 機制擴容，不能動態調整既有 pool 節點數
3. **NATS 高可用**: JetStream 至少 3 節點 (RAFT quorum)
4. **Consul 服務發現**: 強一致性的成員管理與健康檢查

---

## 部署模式

### 單機模式 (min=1)

**用途**: 開發環境、邊緣部署

**組態**:
- **MinIO**: 單機 + 4 個目錄/磁碟啟用 EC
  ```bash
  minio server /data{1..4}
  ```
- **NATS**: 單節點
- **AkiDB**: 1 節點
- **Consul**: 可選 (僅 agent 模式)
- **Auto-scale Controller**: 停用或手動模式

### 高可用模式 (推薦 ≥3)

**用途**: 生產環境

**基線配置 (3 節點)**:
- **NATS JetStream**: 3 節點叢集 (quorum)
- **MinIO**: 1 個 pool, 3 servers × 2 dirs = 6 drives (EC parity 2)
- **AkiDB**: 3 節點 (behind L4 load balancer)
- **Consul**: 3 節點 server quorum
- **Auto-scale Controller**: 啟用 (僅算力擴展)

**彈性配置 (4+ 節點)**:
- **NATS**: 保持 3 節點 quorum (或升級到 5)
- **MinIO**:
  - Pool 1: 原始 3-4 servers
  - Pool 2+: 每次新增 4 servers (當使用率 >75%)
- **AkiDB**: 隨 pool 數量擴展 (例如 6 AkiDB pods 對應 2 pools)
- **Consul**: 3-5 節點 server quorum
- **Auto-scale Controller**: 全功能 (算力 + 容量擴展)

---

## 架構組件

### 1. Service Discovery & Routing

#### Consul 作為成員管理中心

```
┌─────────────────────────────────────────────────────────────┐
│                    Consul Server Cluster                    │
│                      (3 or 5 nodes)                         │
│                                                             │
│  ├─ Service Catalog (AkiDB nodes, health checks)           │
│  ├─ KV Store (Rendezvous ring seeds, MinIO pool manifest)  │
│  └─ DNS/SRV (service discovery)                            │
└─────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
┌───────▼────────┐   ┌───────▼────────┐   ┌───────▼────────┐
│  AkiDB Node 1  │   │  AkiDB Node 2  │   │  AkiDB Node 3  │
│  + Consul Agent│   │  + Consul Agent│   │  + Consul Agent│
└────────────────┘   └────────────────┘   └────────────────┘
```

**為何選擇 Consul**:
- ✅ 強一致性 (Raft) - 路由表不會漂移
- ✅ 內建健康檢查 - 自動故障檢測
- ✅ KV store - 存儲 pool manifest 和 ring seeds
- ✅ DNS/SRV - 原生服務發現
- ✅ ACL 支援 - 符合安全要求

#### Rendezvous Hashing 實現

```rust
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub weight: u64,
}

pub struct RendezvousRing {
    nodes: Vec<Node>,
}

impl RendezvousRing {
    pub fn new(nodes: Vec<Node>) -> Self {
        assert!(!nodes.is_empty(), "ring requires at least one node");
        Self { nodes }
    }

    /// Pick the highest-scoring node for the given key
    pub fn pick<'a>(&'a self, key: &[u8]) -> &'a Node {
        let mut scores: BTreeMap<u128, usize> = BTreeMap::new();

        for (idx, node) in self.nodes.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(node.id.as_bytes());
            hasher.update(key);
            let digest = hasher.finalize();

            // Use first 16 bytes as u128
            let value = u128::from_be_bytes(digest[0..16].try_into().unwrap());

            // XOR with weighted score
            let score = value ^ ((node.weight as u128) << 64);
            scores.insert(score, idx);
        }

        let highest = scores.iter().next_back().expect("non-empty ring");
        &self.nodes[*highest.1]
    }

    /// Pick top-k nodes for redundancy
    pub fn pick_k<'a>(&'a self, key: &[u8], k: usize) -> Vec<&'a Node> {
        let mut scores: BTreeMap<u128, usize> = BTreeMap::new();

        for (idx, node) in self.nodes.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(node.id.as_bytes());
            hasher.update(key);
            let digest = hasher.finalize();
            let value = u128::from_be_bytes(digest[0..16].try_into().unwrap());
            let score = value ^ ((node.weight as u128) << 64);
            scores.insert(score, idx);
        }

        scores
            .iter()
            .rev()
            .take(k.min(self.nodes.len()))
            .map(|(_, idx)| &self.nodes[*idx])
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_is_deterministic() {
        let nodes = vec![
            Node { id: "aki-1".into(), weight: 10 },
            Node { id: "aki-2".into(), weight: 10 },
            Node { id: "aki-3".into(), weight: 10 },
        ];
        let ring = RendezvousRing::new(nodes);
        let key = b"customer:42";

        let x = ring.pick(key);
        let y = ring.pick(key);
        assert_eq!(x.id, y.id, "pick should be deterministic");
    }

    #[test]
    fn pick_k_returns_top_nodes() {
        let nodes = vec![
            Node { id: "aki-1".into(), weight: 10 },
            Node { id: "aki-2".into(), weight: 10 },
            Node { id: "aki-3".into(), weight: 10 },
        ];
        let ring = RendezvousRing::new(nodes);
        let key = b"customer:42";

        let top3 = ring.pick_k(key, 3);
        assert_eq!(top3.len(), 3);

        let top2 = ring.pick_k(key, 2);
        assert_eq!(top2.len(), 2);
    }

    #[test]
    fn weight_affects_selection() {
        let nodes = vec![
            Node { id: "aki-heavy".into(), weight: 100 },
            Node { id: "aki-light".into(), weight: 1 },
        ];
        let ring = RendezvousRing::new(nodes);

        // Heavy node should be selected more often
        let mut heavy_count = 0;
        for i in 0..1000 {
            let key = format!("key:{}", i);
            let node = ring.pick(key.as_bytes());
            if node.id == "aki-heavy" {
                heavy_count += 1;
            }
        }

        // Should be significantly more than 50%
        assert!(heavy_count > 700, "heavy node selected {} times", heavy_count);
    }
}
```

### 2. MinIO Pool Management

#### Pool Lifecycle States

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PoolState {
    Provisioning,  // 正在建立新 pool
    Active,        // 接受讀寫
    Draining,      // 停止新寫入，準備退役
    Retiring,      // 資料遷移中
    Decommissioned // 已移除
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinioPoolManifest {
    pub pool_id: String,
    pub endpoint: String,
    pub servers: Vec<String>,
    pub drives_per_server: usize,
    pub parity: usize,
    pub state: PoolState,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub total_capacity_bytes: u64,
    pub used_capacity_bytes: u64,
}
```

#### Pool Expansion Flow

```rust
pub async fn expand_minio_pool(
    consul: &ConsulClient,
    minio_admin: &MinioAdminClient,
    new_servers: Vec<String>,
    drives_per_server: usize,
) -> Result<String, Error> {
    let pool_id = uuid::Uuid::new_v4().to_string();

    // 1. Create manifest entry in Consul
    let manifest = MinioPoolManifest {
        pool_id: pool_id.clone(),
        endpoint: minio_admin.endpoint().to_string(),
        servers: new_servers.clone(),
        drives_per_server,
        parity: 2, // EC parity
        state: PoolState::Provisioning,
        created_at: chrono::Utc::now(),
        total_capacity_bytes: 0,
        used_capacity_bytes: 0,
    };

    consul.kv_put(
        &format!("minio/pools/{}", pool_id),
        &serde_json::to_string(&manifest)?
    ).await?;

    // 2. Invoke MinIO Admin API to add server pool
    minio_admin.add_server_pool(&new_servers, drives_per_server).await?;

    // 3. Wait for background healing
    loop {
        let healing_status = minio_admin.healing_status().await?;
        if healing_status.pending_objects == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }

    // 4. Update state to Active
    let mut manifest = manifest;
    manifest.state = PoolState::Active;
    consul.kv_put(
        &format!("minio/pools/{}", pool_id),
        &serde_json::to_string(&manifest)?
    ).await?;

    // 5. Broadcast pool addition event
    nats.publish("minio.pool.added", pool_id.as_bytes()).await?;

    Ok(pool_id)
}
```

#### Pool Decommission Flow

```rust
pub async fn decommission_pool(
    consul: &ConsulClient,
    minio_admin: &MinioAdminClient,
    nats: &NatsClient,
    pool_id: &str,
) -> Result<(), Error> {
    // 1. Mark pool as Draining
    let mut manifest: MinioPoolManifest = consul.kv_get(&format!("minio/pools/{}", pool_id)).await?;
    manifest.state = PoolState::Draining;
    consul.kv_put(&format!("minio/pools/{}", pool_id), &serde_json::to_string(&manifest)?).await?;

    // 2. Notify AkiDB nodes to stop new writes to this pool
    nats.publish("minio.pool.draining", pool_id.as_bytes()).await?;

    // 3. Start decommission process
    minio_admin.decommission_start(pool_id).await?;
    manifest.state = PoolState::Retiring;
    consul.kv_put(&format!("minio/pools/{}", pool_id), &serde_json::to_string(&manifest)?).await?;

    // 4. Monitor decommission progress
    loop {
        let status = minio_admin.decommission_status(pool_id).await?;
        if status.complete {
            break;
        }
        tokio::time::sleep(Duration::from_secs(60)).await;
    }

    // 5. Mark as Decommissioned
    manifest.state = PoolState::Decommissioned;
    consul.kv_put(&format!("minio/pools/{}", pool_id), &serde_json::to_string(&manifest)?).await?;

    Ok(())
}
```

### 3. Auto-scale Controller

#### Metrics & Decision Tree

```rust
pub struct AutoscaleMetrics {
    pub latency_p95: f64,           // ms
    pub cache_hit_ratio: f64,       // 0.0-1.0
    pub minio_utilization: f64,     // 0.0-1.0 (used / total)
    pub nats_backlog: u64,          // message count
}

pub struct AutoscaleConfig {
    pub latency_slo_ms: f64,
    pub latency_shrink_target_ms: f64,
    pub cache_hit_floor: f64,
    pub capacity_threshold: f64,
    pub backlog_threshold: u64,
    pub idle_threshold: u64,
    pub cooldown_period: Duration,
    pub min_aki_nodes: usize,
    pub max_aki_nodes: usize,
}

pub enum ScaleDecision {
    AddAkiDBNodes(usize),
    AddMinIOPool { servers: Vec<String>, drives_per_server: usize },
    RemoveAkiDBNodes(Vec<String>),
    NoAction,
}

pub fn decide_scale_action(
    metrics: &AutoscaleMetrics,
    config: &AutoscaleConfig,
    current_aki_nodes: usize,
) -> ScaleDecision {
    // Decision Tree:

    // 1. Need more compute (good cache, low capacity usage, high latency)
    if metrics.latency_p95 > config.latency_slo_ms
        && metrics.cache_hit_ratio >= config.cache_hit_floor
        && metrics.minio_utilization < config.capacity_threshold
    {
        let target_nodes = (current_aki_nodes as f64 * 1.5).ceil() as usize;
        let add_count = (target_nodes - current_aki_nodes).min(config.max_aki_nodes - current_aki_nodes);
        if add_count > 0 {
            return ScaleDecision::AddAkiDBNodes(add_count);
        }
    }

    // 2. Need more capacity (high utilization OR low cache hit)
    if metrics.minio_utilization >= config.capacity_threshold
        || metrics.cache_hit_ratio < config.cache_hit_floor
    {
        // Add new MinIO pool + proportional AkiDB nodes
        let servers = vec![]; // Populated by infrastructure automation
        return ScaleDecision::AddMinIOPool {
            servers,
            drives_per_server: 4,
        };
    }

    // 3. High backlog - rebalance
    if metrics.nats_backlog > config.backlog_threshold {
        if metrics.minio_utilization < config.capacity_threshold {
            // Pure compute issue
            return ScaleDecision::AddAkiDBNodes(2);
        } else {
            // Need both compute and capacity
            return ScaleDecision::AddMinIOPool {
                servers: vec![],
                drives_per_server: 4,
            };
        }
    }

    // 4. Scale-in guard (only if underutilized for cooldown period)
    if metrics.latency_p95 < config.latency_shrink_target_ms
        && metrics.nats_backlog < config.idle_threshold
        && current_aki_nodes > config.min_aki_nodes
    {
        // Would need cooldown tracking here
        // return ScaleDecision::RemoveAkiDBNodes(vec![]);
    }

    ScaleDecision::NoAction
}
```

#### Controller Loop

```rust
pub async fn autoscale_controller_loop(
    prometheus: &PrometheusClient,
    consul: &ConsulClient,
    nats: &NatsClient,
    config: &AutoscaleConfig,
) -> Result<(), Error> {
    let mut last_scale_time = Instant::now();

    loop {
        // 1. Gather metrics
        let metrics = AutoscaleMetrics {
            latency_p95: prometheus.query("aki_request_latency_p95").await?,
            cache_hit_ratio: prometheus.query("aki_cache_hit_ratio").await?,
            minio_utilization: prometheus.query("minio_cluster_used_bytes / minio_cluster_total_bytes").await?,
            nats_backlog: prometheus.query("nats_stream_consumer_backlog").await? as u64,
        };

        // 2. Check cooldown
        if last_scale_time.elapsed() < config.cooldown_period {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        }

        // 3. Get current state
        let current_nodes = consul.catalog_service("aki-db").await?.len();

        // 4. Make decision
        let decision = decide_scale_action(&metrics, config, current_nodes);

        // 5. Execute decision
        match decision {
            ScaleDecision::AddAkiDBNodes(count) => {
                tracing::info!("Scaling out: adding {} AkiDB nodes", count);
                consul.kv_put("autoscale/desired/aki_nodes", &(current_nodes + count).to_string()).await?;
                nats.publish("autoscale.events", b"scale_out_aki").await?;
                last_scale_time = Instant::now();
            }
            ScaleDecision::AddMinIOPool { servers, drives_per_server } => {
                tracing::info!("Scaling out: adding MinIO pool with {} servers", servers.len());
                // Trigger infrastructure automation
                consul.kv_put("autoscale/desired/minio_pools", &(current_pools + 1).to_string()).await?;
                nats.publish("autoscale.events", b"scale_out_minio").await?;
                last_scale_time = Instant::now();
            }
            ScaleDecision::RemoveAkiDBNodes(node_ids) => {
                tracing::info!("Scaling in: removing {} AkiDB nodes", node_ids.len());
                // Trigger graceful drain
                for node_id in &node_ids {
                    nats.publish(&format!("aki.{}.drain", node_id), b"drain").await?;
                }
                last_scale_time = Instant::now();
            }
            ScaleDecision::NoAction => {
                // No action needed
            }
        }

        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
```

### 4. Node Join/Leave Flow

#### Join Flow

```
┌───────────────────┐
│  Provision Node   │
└─────────┬─────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Install:                         │
│  - AkiDB binary                   │
│  - Consul agent                   │
│  - NATS credentials               │
│  - Prometheus node exporter       │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Consul: Register Service         │
│  - service_name: "aki-db"         │
│  - health_check: HTTP /health     │
│  - tags: [version, region, pool]  │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Start reporting metrics          │
│  - Prometheus endpoint :9090      │
│  - Custom AkiDB metrics            │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Controller validates:            │
│  - Health check passing           │
│  - Metrics available              │
│  - (Optional) Cache warm-up       │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Add to Rendezvous Ring           │
│  - Update Consul KV               │
│  - Increment ring revision        │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Broadcast ring update            │
│  - NATS publish:                  │
│    "aki.ring.revisions"           │
│  - All nodes reload ring          │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Node accepts live traffic        │
└───────────────────────────────────┘
```

#### Leave Flow (Graceful)

```
┌───────────────────────────────────┐
│  Controller selects node          │
│  - Lowest load                    │
│  - Oldest node (rolling upgrade)  │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Consul: Set maintenance mode     │
│  - Health check fails             │
│  - Stop receiving new requests    │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  NATS: Publish drain request      │
│  - Subject: "aki.<node_id>.drain" │
│  - Node completes inflight ops    │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Update Rendezvous Ring           │
│  - Remove node from ring          │
│  - Update Consul KV               │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Confirm zero load                │
│  - Check metrics                  │
│  - Verify no active connections   │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Deregister from Consul           │
│  - Remove service entry           │
└─────────┬─────────────────────────┘
          │
          ▼
┌───────────────────────────────────┐
│  Tear down instance               │
│  - Stop AkiDB process             │
│  - Stop Consul agent              │
│  - (If pool member) Decommission  │
└───────────────────────────────────┘
```

---

## Configuration

### TOML Schema

```toml
[aki]
service_name = "aki-db"
listen_addr = "0.0.0.0:8443"
cache_engine = "redis"
cache_hit_floor = 0.85
latency_slo_ms = 120

[discovery]
provider = "consul"
consul_http = "http://consul.service:8500"
consul_dc = "primary"
consul_token = "${CONSUL_HTTP_TOKEN}"
ring_seed_kv = "aki/rendezvous/seed"
nats_subject = "aki.ring.revisions"

[minio]
endpoint = "https://minio.service:9000"
access_key = "${MINIO_ACCESS_KEY}"
secret_key = "${MINIO_SECRET_KEY}"
pool_manifest_kv = "minio/pools"
capacity_threshold = 0.75
decommission_timeout = "24h"

[nats]
servers = [
    "nats://nats-1:4222",
    "nats://nats-2:4222",
    "nats://nats-3:4222"
]
jetstream_domain = "aki"
backlog_threshold = 10000

[autoscale]
enabled = true
cooldown = "10m"
max_aki_nodes = 50
min_aki_nodes = 1
metrics_endpoint = "http://prometheus:9090"
scale_window = "5m"
latency_slo_ms = 120
latency_shrink_target_ms = 60
cache_hit_floor = 0.85
capacity_threshold = 0.75
backlog_threshold = 10000
idle_threshold = 1000

[deployment]
mode = "ha"  # "single" or "ha"
min_minio_drives = 4
minio_parity = 2
```

---

## Deployment Topologies

### Single-Node (Development / Edge)

```yaml
# docker-compose.yml
version: '3.8'
services:
  minio:
    image: minio/minio:latest
    command: server /data{1..4}
    volumes:
      - /mnt/disk1:/data1
      - /mnt/disk2:/data2
      - /mnt/disk3:/data3
      - /mnt/disk4:/data4
    ports:
      - "9000:9000"
      - "9001:9001"
    environment:
      MINIO_ROOT_USER: akidb
      MINIO_ROOT_PASSWORD: ${MINIO_PASSWORD}

  nats:
    image: nats:latest
    command: -js
    ports:
      - "4222:4222"

  akidb:
    image: aifocal/akidb:latest
    ports:
      - "8443:8443"
    environment:
      AKIDB_MODE: single
      MINIO_ENDPOINT: http://minio:9000
      NATS_SERVERS: nats://nats:4222
    volumes:
      - ./config/akidb.toml:/etc/akidb/akidb.toml
```

### 3-Node HA Baseline

```yaml
# Kubernetes deployment example
---
apiVersion: v1
kind: Service
metadata:
  name: nats-cluster
spec:
  clusterIP: None
  selector:
    app: nats
  ports:
    - port: 4222
      name: client
    - port: 6222
      name: cluster
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: nats
spec:
  serviceName: nats-cluster
  replicas: 3
  selector:
    matchLabels:
      app: nats
  template:
    metadata:
      labels:
        app: nats
    spec:
      containers:
      - name: nats
        image: nats:latest
        command:
          - nats-server
          - --cluster_name=aki-cluster
          - --cluster=nats://0.0.0.0:6222
          - --routes=nats://nats-0.nats-cluster:6222,nats://nats-1.nats-cluster:6222,nats://nats-2.nats-cluster:6222
          - --js
        ports:
          - containerPort: 4222
            name: client
          - containerPort: 6222
            name: cluster
---
apiVersion: apps/v1
kind: StatefulSet
metadata:
  name: minio
spec:
  serviceName: minio
  replicas: 3
  selector:
    matchLabels:
      app: minio
  template:
    metadata:
      labels:
        app: minio
    spec:
      containers:
      - name: minio
        image: minio/minio:latest
        command:
          - minio
          - server
          - http://minio-{0...2}.minio:9000/data{1...2}
        volumeMounts:
          - name: data1
            mountPath: /data1
          - name: data2
            mountPath: /data2
  volumeClaimTemplates:
  - metadata:
      name: data1
    spec:
      accessModes: [ "ReadWriteOnce" ]
      resources:
        requests:
          storage: 100Gi
  - metadata:
      name: data2
    spec:
      accessModes: [ "ReadWriteOnce" ]
      resources:
        requests:
          storage: 100Gi
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: akidb
spec:
  replicas: 3
  selector:
    matchLabels:
      app: akidb
  template:
    metadata:
      labels:
        app: akidb
    spec:
      containers:
      - name: akidb
        image: aifocal/akidb:latest
        ports:
          - containerPort: 8443
        env:
          - name: AKIDB_MODE
            value: "ha"
          - name: CONSUL_HTTP_ADDR
            value: "consul:8500"
          - name: NATS_SERVERS
            value: "nats://nats-0.nats-cluster:4222,nats://nats-1.nats-cluster:4222,nats://nats-2.nats-cluster:4222"
        volumeMounts:
          - name: config
            mountPath: /etc/akidb
      volumes:
        - name: config
          configMap:
            name: akidb-config
```

---

## Implementation Schedule

### Phase 1: Foundation (M1 - 4 weeks)

**Week 1-2: Service Discovery**
- ✅ Implement Rendezvous Hashing library
- ✅ Consul integration for service registry
- ✅ Health check implementation
- ✅ Ring management in Consul KV

**Week 3-4: Basic Autoscaling**
- ✅ Metrics collection (Prometheus integration)
- ✅ Auto-scale controller skeleton
- ✅ AkiDB node add/remove logic
- ✅ Configuration system

**Deliverables**:
- `crates/akidb-discovery/` - Rendezvous hashing + Consul client
- `crates/akidb-autoscale/` - Controller logic
- `docs/deployment-single-node.md`
- `docs/deployment-ha.md`

### Phase 2: MinIO Pool Management (M2 - 3 weeks)

**Week 1: Pool Lifecycle**
- ✅ Pool manifest schema
- ✅ MinIO Admin API client
- ✅ Pool expansion flow

**Week 2: Pool Operations**
- ✅ Pool decommission flow
- ✅ Health monitoring
- ✅ NATS event integration

**Week 3: Testing**
- ✅ Integration tests with real MinIO cluster
- ✅ Pool failover scenarios
- ✅ Data migration validation

**Deliverables**:
- `crates/akidb-minio/` - Pool management
- Pool expansion/decommission automation
- Runbooks for operators

### Phase 3: Production Hardening (M3 - 3 weeks)

**Week 1: Observability**
- ✅ Autoscale decision metrics
- ✅ Pool health dashboards (Grafana)
- ✅ Alerting rules

**Week 2: Failure Scenarios**
- ✅ Consul partition handling
- ✅ NATS connection loss
- ✅ MinIO pool failure
- ✅ Circuit breakers

**Week 3: Performance Tuning**
- ✅ Ring rebalancing optimization
- ✅ Cache warm-up strategies
- ✅ Load testing (1K/10K/100K requests/sec)

**Deliverables**:
- Production-ready autoscaler
- Comprehensive monitoring
- Incident response playbooks

### Phase 4: Advanced Features (M4 - 4 weeks)

**Week 1-2: Multi-Region Support**
- ✅ Cross-region Consul federation
- ✅ MinIO site replication integration
- ✅ Region-aware routing

**Week 3: Intelligent Scheduling**
- ✅ Predictive autoscaling (ML-based)
- ✅ Cost optimization (spot instances)
- ✅ SLA-driven scaling

**Week 4: Operator Experience**
- ✅ CLI tools (`akidb-admin scale`, `akidb-admin pool`)
- ✅ Web UI for cluster management
- ✅ GitOps integration (Flux/ArgoCD)

**Deliverables**:
- Multi-region support
- Predictive autoscaling
- Operator tooling

---

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scale_decision_add_compute() {
        let metrics = AutoscaleMetrics {
            latency_p95: 150.0,  // Above SLO
            cache_hit_ratio: 0.90,  // Good
            minio_utilization: 0.50,  // Low
            nats_backlog: 100,
        };

        let config = AutoscaleConfig::default();
        let decision = decide_scale_action(&metrics, &config, 3);

        assert!(matches!(decision, ScaleDecision::AddAkiDBNodes(_)));
    }

    #[test]
    fn test_scale_decision_add_capacity() {
        let metrics = AutoscaleMetrics {
            latency_p95: 100.0,
            cache_hit_ratio: 0.60,  // Low (below floor)
            minio_utilization: 0.80,  // High
            nats_backlog: 100,
        };

        let config = AutoscaleConfig::default();
        let decision = decide_scale_action(&metrics, &config, 3);

        assert!(matches!(decision, ScaleDecision::AddMinIOPool { .. }));
    }
}
```

### Integration Tests
- MinIO pool expansion with real cluster
- Consul ring updates propagation
- NATS event delivery
- End-to-end autoscale cycle

### Load Tests
- 1K QPS sustained load
- 10K QPS spike handling
- Node failure during high load
- Pool decommission under load

---

## Monitoring & Alerts

### Key Metrics

```promql
# Autoscale decisions
rate(akidb_autoscale_decisions_total[5m])

# Node join/leave events
increase(akidb_cluster_nodes_total[1h])

# Pool operations
minio_pool_state{pool_id="pool-1"}

# Ring consistency
akidb_ring_revision - akidb_ring_revision offset 1m

# Scale latency
histogram_quantile(0.95,
  rate(akidb_autoscale_duration_seconds_bucket[5m])
)
```

### Alerting Rules

```yaml
groups:
  - name: autoscale
    rules:
      - alert: AutoscaleFailure
        expr: rate(akidb_autoscale_errors_total[5m]) > 0
        for: 5m
        annotations:
          summary: "Autoscale controller failing"

      - alert: PoolStuckProvisioning
        expr: minio_pool_state{state="Provisioning"} > 0
        for: 30m
        annotations:
          summary: "Pool stuck in provisioning state"

      - alert: RingInconsistent
        expr: stddev(akidb_ring_revision) > 0
        for: 5m
        annotations:
          summary: "Ring revision mismatch across nodes"
```

---

## References

- [Rendezvous Hashing](https://randorithms.com/2020/12/26/rendezvous-hashing.html)
- [MinIO Server Pools](https://docs.min.io/enterprise/aistor-object-store/operations/scaling/expansion/)
- [NATS JetStream Clustering](https://docs.nats.io/running-a-nats-service/configuration/clustering/jetstream_clustering)
- [Consul Service Discovery](https://www.consul.io/docs/discovery/services)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/naming/)
