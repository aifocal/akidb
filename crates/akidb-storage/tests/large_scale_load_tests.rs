//! Large-Scale Load Tests for AkiDB 2.0
//!
//! This test suite pushes the system to its limits to discover:
//! - Maximum sustainable QPS
//! - Memory limits with large datasets
//! - Race conditions under extreme concurrency
//! - Memory leaks during long-running tests
//! - Error handling under failure scenarios
//!
//! **WARNING:** These tests are resource-intensive and may take hours to complete.
//! Run with: `cargo test --release --test large_scale_load_tests -- --nocapture --ignored --test-threads=1`

use akidb_core::ids::DocumentId;
use akidb_core::traits::VectorIndex;
use akidb_core::vector::VectorDocument;
use akidb_core::collection::DistanceMetric;
use akidb_index::BruteForceIndex;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::sleep;

/// Helper: Generate random vector
fn random_vector(dim: usize) -> Vec<f32> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..dim).map(|_| rng.gen_range(-1.0..1.0)).collect()
}

/// Helper: Create test index
async fn create_test_index(dimension: usize) -> Arc<BruteForceIndex> {
    Arc::new(BruteForceIndex::new(dimension, DistanceMetric::Cosine))
}

/// Helper: Bulk insert vectors
async fn bulk_insert(index: &BruteForceIndex, count: usize, dimension: usize) -> Vec<DocumentId> {
    let mut doc_ids = Vec::with_capacity(count);

    println!("📥 Inserting {} vectors...", count);
    let start = Instant::now();

    for i in 0..count {
        let doc_id = DocumentId::new();
        let vector = random_vector(dimension);
        let doc = VectorDocument::new(doc_id, vector);

        index.insert(doc).await.expect("Insert failed");
        doc_ids.push(doc_id);

        if (i + 1) % 10000 == 0 {
            println!("  ✓ Inserted {} vectors ({:.1}%)", i + 1, (i + 1) as f64 / count as f64 * 100.0);
        }
    }

    let elapsed = start.elapsed();
    println!("✅ Bulk insert complete: {} vectors in {:.2}s ({:.0} ops/sec)",
        count, elapsed.as_secs_f64(), count as f64 / elapsed.as_secs_f64());

    doc_ids
}

/// Helper: Measure P50, P95, P99 latencies
struct LatencyStats {
    p50: Duration,
    p95: Duration,
    p99: Duration,
    mean: Duration,
}

fn calculate_latencies(mut durations: Vec<Duration>) -> LatencyStats {
    if durations.is_empty() {
        return LatencyStats {
            p50: Duration::ZERO,
            p95: Duration::ZERO,
            p99: Duration::ZERO,
            mean: Duration::ZERO,
        };
    }

    durations.sort();
    let len = durations.len();

    let p50 = durations[len * 50 / 100];
    let p95 = durations[len * 95 / 100];
    let p99 = durations[len * 99 / 100];

    let sum: Duration = durations.iter().sum();
    let mean = sum / len as u32;

    LatencyStats { p50, p95, p99, mean }
}

// ============================================================================
// Part A: Throughput Stress Tests
// ============================================================================

/// Test A1: Linear QPS Ramp - Find maximum QPS before degradation
///
/// This test gradually increases QPS from 100 to 5000 to find the breaking point.
/// Expected: System should handle 500-1500 QPS gracefully, then degrade beyond that.
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release test_a1_linear_qps_ramp -- --ignored --nocapture
async fn test_a1_linear_qps_ramp() {
    println!("\n{}", "=".repeat(80));
    println!("Test A1: Linear QPS Ramp (30 minutes)");
    println!("Goal: Find maximum sustainable QPS before P95 > 25ms");
    println!("{}\n", "=".repeat(80));

    // Setup
    let dimension = 512;
    let index = create_test_index(dimension).await;

    // Pre-populate with 50k vectors
    println!("📦 Pre-populating index with 50,000 vectors...");
    bulk_insert(&*index, 50_000, dimension).await;

    // Test phases
    let phases = vec![
        (100, Duration::from_secs(5 * 60)),   // 5 min @ 100 QPS
        (500, Duration::from_secs(5 * 60)),   // 5 min @ 500 QPS
        (1000, Duration::from_secs(5 * 60)),  // 5 min @ 1000 QPS
        (2000, Duration::from_secs(5 * 60)),  // 5 min @ 2000 QPS
        (3000, Duration::from_secs(5 * 60)),  // 5 min @ 3000 QPS
        (5000, Duration::from_secs(5 * 60)),  // 5 min @ 5000 QPS
    ];

    let mut degradation_point = None;

    for (qps, duration) in phases {
        println!("\n🚀 Phase: {} QPS for {:.0} minutes", qps, duration.as_secs_f64() / 60.0);

        let interval = Duration::from_secs_f64(1.0 / qps as f64);
        let start = Instant::now();
        let mut latencies = Vec::new();
        let mut error_count = 0;
        let mut request_count = 0;

        while start.elapsed() < duration {
            let query_vector = random_vector(dimension);

            let query_start = Instant::now();
            let result = index.search(&query_vector, 10, None).await;
            let query_latency = query_start.elapsed();

            match result {
                Ok(_) => latencies.push(query_latency),
                Err(_) => error_count += 1,
            }

            request_count += 1;

            // Progress report every 10 seconds
            if request_count % (qps * 10) == 0 {
                let stats = calculate_latencies(latencies.clone());
                println!("  [{:.0}s] Requests: {}, P95: {:.2}ms, Errors: {:.2}%",
                    start.elapsed().as_secs_f64(),
                    request_count,
                    stats.p95.as_secs_f64() * 1000.0,
                    error_count as f64 / request_count as f64 * 100.0
                );
            }

            sleep(interval).await;
        }

        // Calculate final stats
        let stats = calculate_latencies(latencies);
        let error_rate = error_count as f64 / request_count as f64 * 100.0;

        println!("\n📊 Phase Results:");
        println!("  Total requests: {}", request_count);
        println!("  Errors: {} ({:.4}%)", error_count, error_rate);
        println!("  P50: {:.2}ms", stats.p50.as_secs_f64() * 1000.0);
        println!("  P95: {:.2}ms", stats.p95.as_secs_f64() * 1000.0);
        println!("  P99: {:.2}ms", stats.p99.as_secs_f64() * 1000.0);

        // Check for degradation
        if stats.p95.as_millis() > 25 && degradation_point.is_none() {
            degradation_point = Some((qps, stats.p95));
            println!("\n⚠️  DEGRADATION DETECTED: P95 exceeded 25ms at {} QPS", qps);
        }

        if error_rate > 0.1 {
            println!("\n⚠️  ERROR THRESHOLD EXCEEDED: {:.2}% error rate at {} QPS", error_rate, qps);
            break;
        }
    }

    // Summary
    println!("\n{}", "=".repeat(80));
    println!("Test A1: Summary");
    println!("{}", "=".repeat(80));

    if let Some((qps, p95)) = degradation_point {
        println!("✅ Maximum sustainable QPS: ~{} QPS", qps - 100);
        println!("⚠️  Degradation started at {} QPS (P95: {:.2}ms)", qps, p95.as_secs_f64() * 1000.0);
    } else {
        println!("✅ System handled all QPS levels without degradation!");
        println!("🎉 Exceeded expectations: Can handle >5000 QPS");
    }
}

/// Test A2: Sustained Peak Load - Run at 80% of max QPS for 60 minutes
#[tokio::test]
#[ignore]
async fn test_a2_sustained_peak_load() {
    println!("\n{}", "=".repeat(80));
    println!("Test A2: Sustained Peak Load (60 minutes)");
    println!("Goal: Validate stability at 80% of max QPS");
    println!("{}\n", "=".repeat(80));

    let dimension = 512;
    let index = create_test_index(dimension).await;

    // Pre-populate with 100k vectors
    println!("📦 Pre-populating index with 100,000 vectors...");
    bulk_insert(&*index, 100_000, dimension).await;

    // Assume max QPS from Test A1 is ~1500 QPS, so 80% = 1200 QPS
    // If you ran Test A1 and found different max, adjust this
    let target_qps = 1200;
    let duration = Duration::from_secs(60 * 60); // 60 minutes

    println!("🚀 Starting sustained load: {} QPS for 60 minutes", target_qps);
    println!("Monitoring for memory leaks and performance degradation...\n");

    let interval = Duration::from_secs_f64(1.0 / target_qps as f64);
    let start = Instant::now();
    let mut latencies_per_minute: Vec<Vec<Duration>> = vec![Vec::new()];
    let mut current_minute = 0;
    let mut error_count = 0;
    let mut request_count = 0;

    while start.elapsed() < duration {
        let query_vector = random_vector(dimension);

        let query_start = Instant::now();
        let result = index.search(&query_vector, 10, None).await;
        let query_latency = query_start.elapsed();

        match result {
            Ok(_) => {
                let minute = start.elapsed().as_secs() / 60;
                if minute as usize >= latencies_per_minute.len() {
                    latencies_per_minute.push(Vec::new());
                }
                latencies_per_minute[minute as usize].push(query_latency);
            }
            Err(_) => error_count += 1,
        }

        request_count += 1;

        // Report every minute
        let minute = start.elapsed().as_secs() / 60;
        if minute > current_minute {
            current_minute = minute;

            if let Some(latencies) = latencies_per_minute.get(current_minute as usize - 1) {
                let stats = calculate_latencies(latencies.clone());
                println!("[Minute {}] Requests: {}, P95: {:.2}ms, Errors: {}",
                    current_minute,
                    request_count,
                    stats.p95.as_secs_f64() * 1000.0,
                    error_count
                );
            }
        }

        sleep(interval).await;
    }

    // Analyze for degradation over time
    println!("\n📊 Performance Over Time Analysis:");
    println!("{:<10} {:<10} {:<10} {:<10}", "Minute", "P50", "P95", "P99");
    println!("{}", "-".repeat(40));

    let first_10_min_p95: Vec<Duration> = latencies_per_minute.iter()
        .take(10)
        .filter_map(|latencies| {
            if !latencies.is_empty() {
                Some(calculate_latencies(latencies.clone()).p95)
            } else {
                None
            }
        })
        .collect();

    let last_10_min_p95: Vec<Duration> = latencies_per_minute.iter()
        .rev()
        .take(10)
        .filter_map(|latencies| {
            if !latencies.is_empty() {
                Some(calculate_latencies(latencies.clone()).p95)
            } else {
                None
            }
        })
        .collect();

    for (i, latencies) in latencies_per_minute.iter().enumerate() {
        if latencies.is_empty() {
            continue;
        }

        let stats = calculate_latencies(latencies.clone());
        println!("{:<10} {:<10.2} {:<10.2} {:<10.2}",
            i + 1,
            stats.p50.as_secs_f64() * 1000.0,
            stats.p95.as_secs_f64() * 1000.0,
            stats.p99.as_secs_f64() * 1000.0
        );
    }

    // Check for performance degradation
    if !first_10_min_p95.is_empty() && !last_10_min_p95.is_empty() {
        let first_avg: Duration = first_10_min_p95.iter().sum::<Duration>() / first_10_min_p95.len() as u32;
        let last_avg: Duration = last_10_min_p95.iter().sum::<Duration>() / last_10_min_p95.len() as u32;

        let degradation_pct = ((last_avg.as_secs_f64() - first_avg.as_secs_f64()) / first_avg.as_secs_f64()) * 100.0;

        println!("\n📈 Degradation Analysis:");
        println!("  First 10 min avg P95: {:.2}ms", first_avg.as_secs_f64() * 1000.0);
        println!("  Last 10 min avg P95: {:.2}ms", last_avg.as_secs_f64() * 1000.0);
        println!("  Degradation: {:.1}%", degradation_pct);

        if degradation_pct < 10.0 {
            println!("  ✅ PASS: Degradation < 10%");
        } else {
            println!("  ⚠️  WARN: Degradation > 10% (possible memory leak)");
        }
    }

    println!("\n✅ Test A2 Complete");
}

/// Test A3: Burst Storm - Extreme bursts with recovery periods
#[tokio::test]
#[ignore]
async fn test_a3_burst_storm() {
    println!("\n{}", "=".repeat(80));
    println!("Test A3: Burst Storm (15 minutes)");
    println!("Goal: Validate burst handling and recovery");
    println!("{}\n", "=".repeat(80));

    let dimension = 512;
    let index = create_test_index(dimension).await;

    // Pre-populate with 50k vectors
    bulk_insert(&*index, 50_000, dimension).await;

    // Burst pattern: baseline → burst → recovery (repeat 3x)
    let baseline_qps = 100;
    let burst_qps = 10_000;
    let burst_clients = 1000;

    for burst_num in 1..=3 {
        println!("\n🔥 Burst #{} Starting...", burst_num);

        // Baseline phase (3 minutes)
        println!("  📊 Baseline: {} QPS for 3 minutes", baseline_qps);
        let baseline_start = Instant::now();
        let mut baseline_latencies = Vec::new();

        while baseline_start.elapsed() < Duration::from_secs(3 * 60) {
            let query_vector = random_vector(dimension);
            let start = Instant::now();
            let _ = index.search(&query_vector, 10, None).await;
            baseline_latencies.push(start.elapsed());

            sleep(Duration::from_secs_f64(1.0 / baseline_qps as f64)).await;
        }

        let baseline_stats = calculate_latencies(baseline_latencies);
        println!("    Baseline P95: {:.2}ms", baseline_stats.p95.as_secs_f64() * 1000.0);

        // Burst phase (1 minute, extreme load)
        println!("  ⚡ BURST: {} QPS for 1 minute ({}x spike!)", burst_qps, burst_qps / baseline_qps);
        let burst_start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(burst_clients));
        let index_arc: Arc<BruteForceIndex> = Arc::clone(&index);
        let mut burst_tasks = vec![];
        let burst_latencies = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let burst_errors = Arc::new(tokio::sync::Mutex::new(0u64));

        let total_requests = (burst_qps * 60) as usize;
        for _ in 0..total_requests {
            let permit = Arc::clone(&semaphore).acquire_owned().await.unwrap();
            let index_clone: Arc<BruteForceIndex> = Arc::clone(&index_arc);
            let latencies_clone = Arc::clone(&burst_latencies);
            let errors_clone = Arc::clone(&burst_errors);
            let query_vector = random_vector(dimension);

            let task = tokio::spawn(async move {
                let start = Instant::now();
                let result = index_clone.search(&query_vector, 10, None).await;
                let latency = start.elapsed();

                match result {
                    Ok(_) => {
                        let mut latencies = latencies_clone.lock().await;
                        latencies.push(latency);
                    }
                    Err(_) => {
                        let mut errors = errors_clone.lock().await;
                        *errors += 1;
                    }
                }

                drop(permit);
            });

            burst_tasks.push(task);

            // Control QPS during burst
            if burst_tasks.len() % 1000 == 0 {
                sleep(Duration::from_millis(100)).await;
            }
        }

        // Wait for burst to complete
        for task in burst_tasks {
            let _ = task.await;
        }

        let burst_elapsed = burst_start.elapsed();
        let burst_latencies_vec = burst_latencies.lock().await.clone();
        let burst_errors_count = *burst_errors.lock().await;
        let burst_stats = calculate_latencies(burst_latencies_vec.clone());
        let error_rate = burst_errors_count as f64 / total_requests as f64 * 100.0;

        println!("    Burst P95: {:.2}ms", burst_stats.p95.as_secs_f64() * 1000.0);
        println!("    Burst errors: {} ({:.2}%)", burst_errors_count, error_rate);
        println!("    Burst duration: {:.1}s (target: 60s)", burst_elapsed.as_secs_f64());

        // Recovery phase (3 minutes)
        println!("  🔄 Recovery: {} QPS for 3 minutes", baseline_qps);
        let recovery_start = Instant::now();
        let mut recovery_latencies = Vec::new();

        while recovery_start.elapsed() < Duration::from_secs(3 * 60) {
            let query_vector = random_vector(dimension);
            let start = Instant::now();
            let _ = index.search(&query_vector, 10, None).await;
            recovery_latencies.push(start.elapsed());

            sleep(Duration::from_secs_f64(1.0 / baseline_qps as f64)).await;
        }

        let recovery_stats = calculate_latencies(recovery_latencies);
        println!("    Recovery P95: {:.2}ms", recovery_stats.p95.as_secs_f64() * 1000.0);

        // Check recovery
        let recovery_degradation = ((recovery_stats.p95.as_secs_f64() - baseline_stats.p95.as_secs_f64())
            / baseline_stats.p95.as_secs_f64()) * 100.0;

        if recovery_degradation < 20.0 {
            println!("    ✅ Recovery successful (degradation: {:.1}%)", recovery_degradation);
        } else {
            println!("    ⚠️  Slow recovery (degradation: {:.1}%)", recovery_degradation);
        }
    }

    println!("\n✅ Test A3 Complete");
}

// ============================================================================
// Part B: Dataset Size Stress Tests
// ============================================================================

/// Test B1: Large Dataset Ladder - Test with increasingly large datasets
#[tokio::test]
#[ignore]
async fn test_b1_large_dataset_ladder() {
    println!("\n{}", "=".repeat(80));
    println!("Test B1: Large Dataset Ladder (45 minutes)");
    println!("Goal: Find maximum dataset size before OOM or severe degradation");
    println!("{}\n", "=".repeat(80));

    let dimension = 512;
    let dataset_sizes = vec![100_000, 500_000, 1_000_000, 2_000_000];

    for size in dataset_sizes {
        println!("\n📦 Testing with {} vectors...", size);

        let index = create_test_index(dimension).await;

        // Insert phase
        let insert_start = Instant::now();
        bulk_insert(&*index, size, dimension).await;
        let insert_duration = insert_start.elapsed();

        println!("  Insert time: {:.1}s ({:.0} ops/sec)",
            insert_duration.as_secs_f64(),
            size as f64 / insert_duration.as_secs_f64()
        );

        // Query phase (100 QPS for 5 minutes)
        println!("  🔍 Running queries (100 QPS for 5 minutes)...");
        let query_duration = Duration::from_secs(5 * 60);
        let qps = 100;
        let interval = Duration::from_secs_f64(1.0 / qps as f64);

        let start = Instant::now();
        let mut latencies = Vec::new();

        while start.elapsed() < query_duration {
            let query_vector = random_vector(dimension);
            let query_start = Instant::now();
            let _ = index.search(&query_vector, 10, None).await;
            latencies.push(query_start.elapsed());

            sleep(interval).await;
        }

        let stats = calculate_latencies(latencies);

        println!("\n  📊 Results for {} vectors:", size);
        println!("    P50: {:.2}ms", stats.p50.as_secs_f64() * 1000.0);
        println!("    P95: {:.2}ms", stats.p95.as_secs_f64() * 1000.0);
        println!("    P99: {:.2}ms", stats.p99.as_secs_f64() * 1000.0);

        // Check if we should continue
        if stats.p95.as_millis() > 100 {
            println!("  ⚠️  P95 exceeded 100ms, stopping dataset ladder");
            break;
        }
    }

    println!("\n✅ Test B1 Complete");
}

/// Test B2: High-Dimensional Vectors
#[tokio::test]
#[ignore]
async fn test_b2_high_dimensional_vectors() {
    println!("\n{}", "=".repeat(80));
    println!("Test B2: High-Dimensional Vectors (30 minutes)");
    println!("Goal: Validate support for max dimensions (4096)");
    println!("{}\n", "=".repeat(80));

    let test_cases = vec![
        (50_000, 2048), // 4x normal
        (50_000, 4096), // 8x normal (max)
        (10_000, 4096), // Reduced count at max dim
    ];

    for (count, dimension) in test_cases {
        println!("\n📦 Testing: {} vectors @ {} dimensions", count, dimension);

        let index = create_test_index(dimension).await;

        // Insert
        let insert_start = Instant::now();
        bulk_insert(&*index, count, dimension).await;
        let insert_duration = insert_start.elapsed();

        println!("  Insert time: {:.1}s", insert_duration.as_secs_f64());

        // Query
        println!("  🔍 Running queries (100 QPS for 5 minutes)...");
        let query_duration = Duration::from_secs(5 * 60);
        let qps = 100;
        let interval = Duration::from_secs_f64(1.0 / qps as f64);

        let start = Instant::now();
        let mut latencies = Vec::new();

        while start.elapsed() < query_duration {
            let query_vector = random_vector(dimension);
            let query_start = Instant::now();
            let _ = index.search(&query_vector, 10, None).await;
            latencies.push(query_start.elapsed());

            sleep(interval).await;
        }

        let stats = calculate_latencies(latencies);

        println!("  📊 Results:");
        println!("    P95: {:.2}ms", stats.p95.as_secs_f64() * 1000.0);
    }

    println!("\n✅ Test B2 Complete");
}

// ============================================================================
// Part D: Concurrency Stress Tests
// ============================================================================

/// Test D1: Extreme Concurrency - 1000 concurrent clients
#[tokio::test]
#[ignore]
async fn test_d1_extreme_concurrency() {
    println!("\n{}", "=".repeat(80));
    println!("Test D1: Extreme Concurrency (20 minutes)");
    println!("Goal: Find race conditions and deadlocks");
    println!("{}\n", "=".repeat(80));

    let dimension = 512;
    let index = Arc::new(create_test_index(dimension).await);

    // Pre-populate
    bulk_insert(&**index, 10_000, dimension).await;

    println!("🚀 Launching 1000 concurrent clients...");

    let duration = Duration::from_secs(20 * 60);
    let num_clients = 1000;
    let qps_per_client = 0.1; // Each client: 1 request every 10 seconds

    let start = Instant::now();
    let mut tasks = vec![];

    for client_id in 0..num_clients {
        let index_clone: Arc<BruteForceIndex> = Arc::clone(&index);

        let task = tokio::spawn(async move {
            let mut client_requests = 0;
            let mut client_errors = 0;

            while start.elapsed() < duration {
                let query_vector = random_vector(dimension);

                match index_clone.search(&query_vector, 10, None).await {
                    Ok(_) => client_requests += 1,
                    Err(_) => client_errors += 1,
                }

                sleep(Duration::from_secs_f64(1.0 / qps_per_client)).await;
            }

            (client_id, client_requests, client_errors)
        });

        tasks.push(task);
    }

    println!("⏳ Running test for 20 minutes with {} clients...", num_clients);

    // Wait for all clients
    let mut total_requests = 0;
    let mut total_errors = 0;

    for task in tasks {
        if let Ok((client_id, requests, errors)) = task.await {
            total_requests += requests;
            total_errors += errors;

            if errors > 0 {
                println!("  ⚠️  Client {} had {} errors", client_id, errors);
            }
        }
    }

    println!("\n📊 Results:");
    println!("  Total requests: {}", total_requests);
    println!("  Total errors: {} ({:.4}%)",
        total_errors,
        total_errors as f64 / total_requests as f64 * 100.0
    );

    if total_errors == 0 {
        println!("  ✅ PASS: No race conditions or deadlocks detected");
    } else {
        println!("  ⚠️  WARN: Errors detected, review logs for race conditions");
    }

    println!("\n✅ Test D1 Complete");
}
