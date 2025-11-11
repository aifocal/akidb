# Runbook: High Search Latency

**Alert**: HighSearchLatency
**Severity**: WARNING
**Threshold**: P95 >25ms for 10+ minutes

## Immediate Actions

```bash
# Check tier distribution
curl -s http://localhost:8080/api/v1/metrics/tiers

# Check search latency by tier
curl -s http://localhost:8080/metrics | grep vector_search_duration_seconds

# Identify slow collections
curl http://localhost:8080/api/v1/collections
```

## Common Causes

1. **Cold Tier Access**: Searches hitting S3-backed collections
2. **Large Result Sets**: top_k value too high
3. **Index Degradation**: HNSW index needs rebuild
4. **Memory Pressure**: Swapping to disk
5. **CPU Throttling**: Insufficient CPU quota

## Mitigation

```bash
# Promote frequently accessed collections to hot tier
curl -X POST http://localhost:8080/api/v1/collections/{id}/tier \
  -d '{"tier": "hot"}'

# Check collection sizes
curl http://localhost:8080/api/v1/collections/{id}

# Scale horizontally if needed
kubectl scale deployment/akidb --replicas=3
```

## Prevention

- Review access patterns and tier assignments
- Optimize HNSW parameters (m, ef_construction)
- Monitor collection growth
- Set up auto-scaling based on latency

## Escalation

If P95 >100ms for 30+ minutes, escalate to engineering team
