# Runbook: Slow Snapshots

**Alert**: SlowSnapshots
**Severity**: WARNING
**Threshold**: P95 >30s for snapshots

## Immediate Actions

```bash
# Check snapshot metrics
curl -s http://localhost:8080/metrics | grep compactions_total

# View WAL size (large WAL = slow snapshot)
curl -s http://localhost:8080/metrics | grep wal_size_bytes

# Check S3 upload latency
curl -s http://localhost:8080/metrics | grep s3_operation_duration_seconds
```

## Common Causes

1. **Large WAL**: Accumulated writes not compacted
2. **Slow S3 Uploads**: Network or S3 performance issues
3. **Uncompressed Snapshots**: Large Parquet files
4. **CPU Constraint**: Serialization bottleneck
5. **Disk I/O**: Slow local disk for temporary files

## Mitigation

```bash
# Force immediate compaction to reset WAL
curl -X POST http://localhost:8080/admin/compact-all

# Enable snapshot compression
kubectl set env deployment/akidb AKIDB_SNAPSHOT_COMPRESSION=true

# Reduce snapshot size by increasing frequency (smaller deltas)
kubectl set env deployment/akidb AKIDB_SNAPSHOT_INTERVAL=1800

# Check available disk space
df -h /tmp /var/lib/akidb
```

## Prevention

- Monitor WAL size trends
- Tune compaction thresholds
- Enable compression by default
- Increase snapshot frequency for large collections

## Escalation

If snapshots timeout (>5 minutes), investigate immediately
