# Runbook: Memory Pressure

**Alert**: MemoryPressure
**Severity**: WARNING
**Threshold**: Hot tier >85% of 8GB limit

## Immediate Actions

```bash
# Check memory usage by component
curl -s http://localhost:8080/metrics | grep akidb_memory_usage_bytes

# List all collections
curl http://localhost:8080/api/v1/collections

# Check tier distribution
curl http://localhost:8080/api/v1/metrics/tiers
```

## Diagnosis

```bash
# Get collection sizes
for id in $(curl -s http://localhost:8080/api/v1/collections | jq -r '.collections[].collection_id'); do
  echo "Collection $id:"
  curl -s http://localhost:8080/api/v1/collections/$id | jq '.collection.document_count'
done

# Check for memory leaks (restart count)
kubectl get pods -l app=akidb -o json | jq '.items[].status.containerStatuses[].restartCount'
```

## Mitigation

```bash
# Demote large collections to warm tier
curl -X POST http://localhost:8080/api/v1/collections/{id}/tier -d '{"tier": "warm"}'

# Force compaction to reduce WAL size
curl -X POST http://localhost:8080/admin/compact-all

# If critical, scale up memory
kubectl set resources deployment/akidb --limits=memory=16Gi
```

## Prevention

- Set up automatic tiering policies
- Monitor collection growth trends
- Configure memory quotas per collection
- Implement auto-scaling based on memory usage

## Escalation

If OOMKilled events occur, immediate escalation required
