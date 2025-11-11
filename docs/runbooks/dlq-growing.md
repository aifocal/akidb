# Runbook: DLQ Growing

**Alert**: DLQGrowing
**Severity**: WARNING
**Threshold**: >50 entries and increasing

## Immediate Actions

```bash
# Check DLQ size
curl -s http://localhost:8080/metrics | grep akidb_dlq_size

# View S3 error rate
curl -s http://localhost:8080/metrics | grep s3_operations_total

# Check circuit breaker state
curl -s http://localhost:8080/metrics | grep circuit_breaker_state
```

## Common Causes

1. **S3 Service Degradation**: Intermittent failures
2. **Network Issues**: Packet loss, timeouts
3. **Rate Limiting**: Exceeding S3 rate limits
4. **Credentials Invalid**: AWS/MinIO auth failures

## Mitigation

```bash
# Retry DLQ entries (may succeed if transient issue resolved)
curl -X POST http://localhost:8080/admin/collections/{id}/dlq/retry

# If persistent S3 issues, clear DLQ and switch to memory-only
curl -X DELETE http://localhost:8080/admin/collections/{id}/dlq
kubectl set env deployment/akidb AKIDB_STORAGE_POLICY=Memory

# Monitor DLQ after mitigation
watch -n 5 'curl -s http://localhost:8080/metrics | grep dlq_size'
```

## Prevention

- Monitor S3 health proactively
- Increase retry limits for transient failures
- Set DLQ size alerts at 25 entries (earlier warning)
- Review S3 operation logs for patterns

## Escalation

If DLQ >100 entries, escalate to investigate S3 infrastructure
