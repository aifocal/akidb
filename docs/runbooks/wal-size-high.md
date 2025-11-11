# Runbook: WAL Size High

**Alert**: WALSizeHigh
**Severity**: WARNING
**Threshold**: WAL >100MB for 15+ minutes

## Immediate Actions

```bash
# Check WAL size
curl -s http://localhost:8080/metrics | grep wal_size_bytes

# Check compaction metrics
curl -s http://localhost:8080/metrics | grep compactions_total

# View last snapshot time
curl -s http://localhost:8080/metrics | grep last_snapshot_at
```

## Common Causes

1. **Compaction Disabled**: Auto-compaction not running
2. **S3 Unavailable**: Cannot upload snapshots
3. **High Write Volume**: Inserts faster than compaction
4. **Disk Space Low**: Cannot write snapshot
5. **Compaction Worker Failing**: Background worker errors

## Diagnosis

```bash
# Check compaction worker status
curl -s http://localhost:8080/metrics | grep 'worker_type="compaction"'

# Check S3 error rate
curl -s http://localhost:8080/metrics | grep s3_operations_total

# Check disk space
df -h /var/lib/akidb /tmp

# View recent compaction attempts
docker logs akidb-rest | grep -i "compaction"
```

## Mitigation

```bash
# Force immediate compaction
curl -X POST http://localhost:8080/admin/compact-all

# If S3 failing, temporarily reduce compaction threshold
kubectl set env deployment/akidb AKIDB_WAL_COMPACTION_THRESHOLD_MB=50

# Monitor WAL size after compaction
watch -n 5 'curl -s http://localhost:8080/metrics | grep wal_size_bytes'

# If disk space low, clear old snapshots
find /var/lib/akidb/snapshots -mtime +7 -delete
```

## WAL Growth Rate

**Normal**: 1-10MB/hour depending on write volume
**Warning**: >50MB/hour sustained
**Critical**: >100MB/hour

## Prevention

- Enable auto-compaction (default)
- Monitor compaction success rate
- Set up disk space alerts
- Tune compaction thresholds for workload

## Escalation

If WAL >500MB, investigate immediately (potential disk space exhaustion)
