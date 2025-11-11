# Runbook: S3 Errors

**Alert**: S3ErrorRateHigh
**Severity**: CRITICAL
**Threshold**: >10% S3 errors for 5+ minutes

## Immediate Actions

```bash
# Check S3 connectivity
aws s3 ls s3://your-bucket --endpoint-url http://minio:9000

# Check DLQ size (failed S3 operations)
curl -s http://localhost:8080/metrics | grep akidb_dlq_size

# View S3 error breakdown
curl -s http://localhost:8080/metrics | grep akidb_s3_operations_total
```

## Common Causes

1. **MinIO/S3 Down**: Service unreachable
2. **Network Issues**: Intermittent connectivity
3. **Credentials Expired**: AWS IAM or MinIO access keys invalid
4. **Bucket Permissions**: Insufficient permissions for operations
5. **Rate Limiting**: Exceeding S3 rate limits

## Mitigation

```bash
# Check circuit breaker state
curl -s http://localhost:8080/metrics | grep circuit_breaker_state

# Retry failed DLQ entries
curl -X POST http://localhost:8080/admin/collections/{id}/dlq/retry

# If persistent, disable S3 temporarily
kubectl set env deployment/akidb AKIDB_STORAGE_POLICY=Memory
kubectl rollout restart deployment/akidb
```

## Prevention

- Monitor S3 service health
- Set up S3 access logs
- Configure retry policies with exponential backoff
- Ensure adequate S3 quotas

## Escalation

Contact cloud provider support if S3 service degradation confirmed
