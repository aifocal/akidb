# Runbook: High Error Rate

**Alert**: HighErrorRate
**Severity**: CRITICAL
**Threshold**: >5% errors for 5+ minutes
**Component**: API

## Immediate Actions (5 min)

1. **Check system health**:
   ```bash
   curl http://localhost:8080/health
   curl http://localhost:8080/admin/health
   ```

2. **View recent errors**:
   ```bash
   # Docker logs (last 10 minutes)
   docker logs akidb-rest --since 10m | grep ERROR

   # Or kubectl logs
   kubectl logs -l app=akidb --since=10m | grep ERROR
   ```

3. **Check Grafana Errors dashboard**:
   - Navigate to: http://localhost:3000/d/akidb-errors
   - Review error rate by endpoint
   - Check 4xx vs 5xx distribution

## Diagnosis (10 min)

### Common Causes

#### 1. S3 Unavailable
- **Check**: `akidb_s3_operations_total{status="error"}`
- **Symptoms**: High error rate on `/insert` and `/delete` endpoints
- **Action**: Verify S3 connectivity

```bash
# Test S3 connection
aws s3 ls s3://your-bucket --endpoint-url http://your-minio:9000

# Check S3 metrics
curl -s http://localhost:8080/metrics | grep akidb_s3_operations_total
```

#### 2. Database Connection Pool Exhausted
- **Check**: Logs for "connection refused" or "pool timeout"
- **Symptoms**: All endpoints returning 500 errors
- **Action**: Restart service to reset connection pool

```bash
# Check active connections (if using PostgreSQL for metadata)
docker exec akidb-db psql -U akidb -c "SELECT count(*) FROM pg_stat_activity;"

# Restart service
docker restart akidb-rest
# OR
kubectl rollout restart deployment/akidb
```

#### 3. Memory Pressure
- **Check**: `akidb_memory_usage_bytes`
- **Symptoms**: Slow responses, eventual crashes
- **Action**: Manually demote collections to warm/cold tier

```bash
# Check memory usage
curl -s http://localhost:8080/metrics | grep akidb_memory_usage_bytes

# Force collection demotion (if tiering enabled)
curl -X POST http://localhost:8080/api/v1/collections/{id}/tier \
  -H "Content-Type: application/json" \
  -d '{"tier": "warm"}'
```

#### 4. Index Corruption
- **Check**: Logs for HNSW errors or panic messages
- **Symptoms**: Search queries failing consistently
- **Action**: Rebuild affected collection indexes

```bash
# Delete and recreate collection (WARNING: data loss if no backup)
curl -X DELETE http://localhost:8080/api/v1/collections/{id}

# Restore from S3 snapshot (if available)
# This requires admin access to storage backend
```

## Mitigation

### If S3 Down
```bash
# Disable S3 temporarily (requires config change + restart)
kubectl set env deployment/akidb S3_ENABLED=false
kubectl rollout restart deployment/akidb
```

### If Memory Issues
```bash
# Force garbage collection + compaction
curl -X POST http://localhost:8080/admin/compact-all

# Or manually evict LRU collections
curl -X POST http://localhost:8080/admin/evict-lru
```

### If Circuit Breaker Open
```bash
# Reset circuit breaker (emergency only)
curl -X POST http://localhost:8080/admin/circuit-breaker/reset
```

## Prevention

- **Horizontal Scaling**: Add more replicas if sustained high load
- **Review Recent Deploys**: Check if error rate correlates with recent deployment
- **Check External Dependencies**: Verify S3, database, network health
- **Review Resource Quotas**: Ensure adequate CPU/memory allocated

## Escalation

**If unresolved after 30 minutes**:
- Page on-call: `@oncall-akidb`
- Create incident ticket
- Notify #akidb-alerts Slack channel

## Post-Incident

1. Document root cause in incident report
2. Add test case to prevent recurrence
3. Update this runbook if new mitigation steps discovered
4. Review error budget and adjust SLOs if needed
