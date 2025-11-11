# Runbook: S3 High Latency

**Alert**: S3HighLatency
**Severity**: WARNING
**Threshold**: PUT P95 >2s for 10+ minutes

## Immediate Actions

```bash
# Check S3 operation latency
curl -s http://localhost:8080/metrics | grep s3_operation_duration_seconds

# Test direct S3 access
time aws s3 cp /tmp/test.txt s3://your-bucket/test.txt --endpoint-url http://minio:9000

# Check network latency to S3
ping minio-host
traceroute minio-host
```

## Common Causes

1. **Network Congestion**: High latency to S3 endpoint
2. **S3 Service Degradation**: Provider-side performance issues
3. **Large Object Sizes**: Snapshots >100MB
4. **Bandwidth Saturation**: Uploading too fast
5. **MinIO Disk I/O**: Slow disk on MinIO server

## Mitigation

```bash
# Enable compression for snapshots (reduce upload size)
kubectl set env deployment/akidb AKIDB_SNAPSHOT_COMPRESSION=true

# Reduce snapshot frequency temporarily
kubectl set env deployment/akidb AKIDB_SNAPSHOT_INTERVAL=3600

# Use parallel uploads if supported
kubectl set env deployment/akidb AKIDB_S3_MAX_CONCURRENT=10
```

## Prevention

- Monitor S3 endpoint latency
- Optimize snapshot sizes with compression
- Consider using S3 Transfer Acceleration
- Deploy MinIO closer to compute (same AZ/region)

## Escalation

If latency >10s consistently, investigate S3 provider status
