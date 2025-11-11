# Runbook: Circuit Breaker Open

**Alert**: CircuitBreakerOpen
**Severity**: CRITICAL
**Threshold**: Circuit breaker state = OPEN

## Immediate Actions

```bash
# Check circuit breaker state and error rate
curl -s http://localhost:8080/metrics | grep circuit_breaker

# View S3 error rate (likely cause)
curl -s http://localhost:8080/metrics | grep s3_operations_total

# Check DLQ size
curl -s http://localhost:8080/metrics | grep dlq_size
```

## What Circuit Breaker Open Means

**Impact**: All S3 operations are BLOCKED to prevent cascading failures
- Inserts still succeed (written to WAL only)
- Deletes still succeed (WAL only)
- Snapshots are paused
- S3 uploads are queued to DLQ

## Diagnosis

```bash
# Check S3 connectivity
aws s3 ls s3://your-bucket --endpoint-url http://minio:9000

# Test S3 write
echo "test" > /tmp/test.txt
aws s3 cp /tmp/test.txt s3://your-bucket/test.txt

# View recent S3 errors
docker logs akidb-rest | grep -i "s3 error"
```

## Mitigation

### If S3 is Healthy Now
```bash
# Reset circuit breaker (allows retry)
curl -X POST http://localhost:8080/admin/circuit-breaker/reset

# Monitor state transition
watch -n 2 'curl -s http://localhost:8080/metrics | grep circuit_breaker_state'
```

### If S3 Still Down
```bash
# Let circuit breaker remain OPEN (protects system)
# Monitor DLQ size
curl -s http://localhost:8080/metrics | grep dlq_size

# Once S3 recovers, circuit breaker will auto-transition to HALF_OPEN
# then CLOSED if operations succeed
```

## Circuit Breaker States

- **CLOSED** (0): Normal operation
- **HALF_OPEN** (1): Testing if issue resolved
- **OPEN** (2): Blocking all S3 operations

## Prevention

- Monitor S3 error rate proactively
- Set up S3 health checks
- Configure circuit breaker thresholds appropriately
- Test circuit breaker behavior in staging

## Escalation

If circuit breaker remains OPEN >30 minutes, escalate to infrastructure team
