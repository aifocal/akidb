# Runbook: Tier Imbalance

**Alert**: TierImbalance
**Severity**: WARNING
**Threshold**: Hot tier >50% of total collections

## Immediate Actions

```bash
# Check tier distribution
curl http://localhost:8080/api/v1/metrics/tiers

# List all collections with tiers
curl http://localhost:8080/api/v1/collections | jq '.collections[] | {id, name, tier}'

# Check memory usage
curl -s http://localhost:8080/metrics | grep memory_usage_bytes
```

## Why Tier Balance Matters

- **Hot Tier**: In-memory, fast but limited capacity (8GB target)
- **Warm Tier**: Memory + S3 backup, moderate latency
- **Cold Tier**: S3-only, high latency but unlimited capacity

**Target Distribution**: 20% hot, 30% warm, 50% cold

## Diagnosis

```bash
# Identify large collections in hot tier
for id in $(curl -s http://localhost:8080/api/v1/collections | jq -r '.collections[] | select(.tier=="hot") | .collection_id'); do
  echo "Collection $id:"
  curl -s http://localhost:8080/api/v1/collections/$id | jq '{name, document_count, tier}'
done

# Check access patterns (if tiering manager enabled)
curl http://localhost:8080/api/v1/metrics/access-patterns
```

## Mitigation

```bash
# Demote infrequently accessed collections
curl -X POST http://localhost:8080/api/v1/collections/{id}/tier \
  -H "Content-Type: application/json" \
  -d '{"tier": "warm"}'

# Auto-rebalance (if tiering manager enabled)
curl -X POST http://localhost:8080/admin/tiering/rebalance

# Check result
curl http://localhost:8080/api/v1/metrics/tiers
```

## Demotion Criteria

Demote to warm tier if:
- Collection size >1GB
- Access frequency <10 queries/hour
- Not accessed in last 24 hours

Demote to cold tier if:
- Not accessed in last 7 days
- Archive/compliance use case
- Batch processing only

## Prevention

- Enable automatic tiering policies
- Monitor access patterns
- Set collection-level tier hints at creation
- Review tier assignments monthly

## Escalation

If hot tier approaching memory limit (>90%), immediate action required
