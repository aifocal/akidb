//! Load test client for executing operations

use akidb_core::ids::CollectionId;
use std::time::Duration;

/// Operation types for load testing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationType {
    Search,
    Insert,
    Update,
    Delete,
    Metadata,
}

/// Load test client for executing operations
pub struct LoadTestClient {
    collection_id: CollectionId,
    dimension: usize,
}

impl LoadTestClient {
    /// Create new load test client
    pub fn new(collection_id: CollectionId, dimension: usize) -> Self {
        Self {
            collection_id,
            dimension,
        }
    }

    /// Execute an operation and return duration
    pub async fn execute(&self, op_type: OperationType) -> Result<Duration, String> {
        let start = std::time::Instant::now();

        match op_type {
            OperationType::Search => self.do_search().await?,
            OperationType::Insert => self.do_insert().await?,
            OperationType::Update => self.do_update().await?,
            OperationType::Delete => self.do_delete().await?,
            OperationType::Metadata => self.do_metadata().await?,
        }

        Ok(start.elapsed())
    }

    /// Simulate search operation
    async fn do_search(&self) -> Result<(), String> {
        // Simulate vector search with random query
        tokio::time::sleep(Duration::from_micros(100)).await;
        Ok(())
    }

    /// Simulate insert operation
    async fn do_insert(&self) -> Result<(), String> {
        // Create random vector
        let _vector: Vec<f32> = (0..self.dimension).map(|_| rand::random::<f32>()).collect();

        // Simulate insert
        tokio::time::sleep(Duration::from_micros(50)).await;
        Ok(())
    }

    /// Simulate update operation
    async fn do_update(&self) -> Result<(), String> {
        // Simulate update
        tokio::time::sleep(Duration::from_micros(75)).await;
        Ok(())
    }

    /// Simulate delete operation
    async fn do_delete(&self) -> Result<(), String> {
        // Simulate delete
        tokio::time::sleep(Duration::from_micros(30)).await;
        Ok(())
    }

    /// Simulate metadata operation (list, get info)
    async fn do_metadata(&self) -> Result<(), String> {
        // Simulate metadata fetch
        tokio::time::sleep(Duration::from_micros(200)).await;
        Ok(())
    }

    /// Choose operation type based on workload percentages
    pub fn choose_operation(
        search_pct: f32,
        insert_pct: f32,
        update_pct: f32,
        delete_pct: f32,
        _metadata_pct: f32,
    ) -> OperationType {
        let roll: f32 = rand::random();

        if roll < search_pct {
            OperationType::Search
        } else if roll < search_pct + insert_pct {
            OperationType::Insert
        } else if roll < search_pct + insert_pct + update_pct {
            OperationType::Update
        } else if roll < search_pct + insert_pct + update_pct + delete_pct {
            OperationType::Delete
        } else {
            OperationType::Metadata
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_operations() {
        let client = LoadTestClient::new(CollectionId::new(), 128);

        // Test each operation type
        assert!(client.execute(OperationType::Search).await.is_ok());
        assert!(client.execute(OperationType::Insert).await.is_ok());
        assert!(client.execute(OperationType::Update).await.is_ok());
        assert!(client.execute(OperationType::Delete).await.is_ok());
        assert!(client.execute(OperationType::Metadata).await.is_ok());
    }

    #[test]
    fn test_operation_selection() {
        // Test that we can select operations based on percentages
        let mut counts = [0; 5];

        for _ in 0..1000 {
            let op = LoadTestClient::choose_operation(0.7, 0.2, 0.05, 0.03, 0.02);
            match op {
                OperationType::Search => counts[0] += 1,
                OperationType::Insert => counts[1] += 1,
                OperationType::Update => counts[2] += 1,
                OperationType::Delete => counts[3] += 1,
                OperationType::Metadata => counts[4] += 1,
            }
        }

        // Search should be ~70% (allow 10% variance)
        assert!(counts[0] > 600 && counts[0] < 800);
        // Insert should be ~20%
        assert!(counts[1] > 150 && counts[1] < 250);
    }
}
