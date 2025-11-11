# Runbook: Background Worker Errors

**Alert**: BackgroundWorkerErrors
**Severity**: WARNING
**Threshold**: >10% worker error rate

## Immediate Actions

```bash
# Check worker error rate by type
curl -s http://localhost:8080/metrics | grep background_worker_runs_total

# View worker logs
docker logs akidb-rest | grep -i "background worker"

# Check specific worker types
curl -s http://localhost:8080/metrics | grep 'worker_type="tiering"'
curl -s http://localhost:8080/metrics | grep 'worker_type="dlq_cleanup"'
curl -s http://localhost:8080/metrics | grep 'worker_type="compaction"'
```

## Common Worker Types & Failures

### 1. Tiering Worker
- **Purpose**: Promote/demote collections between tiers
- **Common Failures**: S3 access errors, metadata update failures
- **Fix**: Check S3 connectivity, verify tier policies

### 2. DLQ Cleanup Worker
- **Purpose**: Remove expired DLQ entries
- **Common Failures**: Database lock contention
- **Fix**: Reduce cleanup frequency, check DB health

### 3. Compaction Worker
- **Purpose**: Create snapshots from WAL
- **Common Failures**: Disk space exhausted, S3 upload failures
- **Fix**: Free disk space, check S3 quotas

## Mitigation

```bash
# Disable problematic worker temporarily
kubectl set env deployment/akidb AKIDB_WORKER_TIERING_ENABLED=false

# Restart service to reset worker state
kubectl rollout restart deployment/akidb

# Check disk space if compaction failing
df -h /var/lib/akidb
```

## Prevention

- Monitor worker success rates proactively
- Set up disk space alerts
- Review worker logs weekly
- Test worker error handling in staging

## Escalation

If multiple worker types failing simultaneously, escalate immediately
