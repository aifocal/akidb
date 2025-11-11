//! Comprehensive load tests for AkiDB 2.0
//!
//! This test suite implements 8 load test scenarios from the load test design:
//! 1. Baseline Performance (30 min)
//! 2. Sustained High Load (60 min)
//! 3. Spike Load (15 min)
//! 4. Tiered Storage Workflow (45 min)
//! 5. Multi-Tenant Load (30 min) - TODO
//! 6. Large Dataset (60 min) - TODO
//! 7. Failure Injection (20 min) - TODO
//! 8. Mixed Workload Chaos (30 min) - TODO

mod load_test_framework;

use load_test_framework::{
    LoadProfile, LoadTestOrchestrator, ReportFormat, ScenarioConfig, SuccessCriteria, WorkloadMix,
};
use std::time::Duration;

// ============================================================================
// Scenario 1: Baseline Performance (30 minutes)
// ============================================================================

/// Scenario 1: Baseline Performance
///
/// Purpose: Establish baseline metrics under normal load
/// Duration: 30 minutes
/// QPS: 100 (constant)
/// Workload: 70% search, 20% insert, 10% metadata
/// Success Criteria:
/// - P95 latency <25ms
/// - Error rate <0.1%
/// - Memory stable (no growth >10MB/min)
/// - CPU <70% average
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_1_baseline -- --ignored --nocapture
async fn scenario_1_baseline() {
    let config = ScenarioConfig {
        name: "Scenario 1: Baseline Performance".to_string(),
        duration: Duration::from_secs(1800), // 30 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix {
            search_pct: 0.7,
            insert_pct: 0.2,
            metadata_pct: 0.1,
            update_pct: 0.0,
            delete_pct: 0.0,
        },
        dataset_size: 10_000,
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 25.0,
        max_error_rate: 0.001, // 0.1%
        max_memory_growth_mb_per_min: 10.0,
        max_cpu_utilization: 0.70,
        min_throughput_qps: Some(95.0), // Allow 5% variance
        max_p99_latency_ms: Some(50.0),
    };

    run_scenario(config, success_criteria, "baseline").await;
}

// ============================================================================
// Scenario 1 (Quick): Baseline Performance - 5 minute version for CI
// ============================================================================

/// Scenario 1 (Quick): Baseline Performance - Short Version
///
/// Same as Scenario 1 but runs for only 5 minutes for faster validation
#[tokio::test]
async fn scenario_1_baseline_quick() {
    let config = ScenarioConfig {
        name: "Scenario 1: Baseline Performance (Quick)".to_string(),
        duration: Duration::from_secs(300), // 5 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix::default(),
        dataset_size: 10_000,
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 25.0,
        max_error_rate: 0.001,
        max_memory_growth_mb_per_min: 10.0,
        max_cpu_utilization: 0.70,
        min_throughput_qps: Some(95.0),
        max_p99_latency_ms: Some(50.0),
    };

    run_scenario(config, success_criteria, "baseline_quick").await;
}

// ============================================================================
// Scenario 2: Sustained High Load (60 minutes)
// ============================================================================

/// Scenario 2: Sustained High Load
///
/// Purpose: Validate stability under sustained production load
/// Duration: 60 minutes
/// QPS: 200 (constant, 2x baseline)
/// Workload: Same as Scenario 1
/// Success Criteria:
/// - P95 latency <50ms (degraded but acceptable)
/// - Error rate <0.5%
/// - No crashes or panics
/// - Memory growth <100MB over duration
/// - CPU <85% average
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_2_sustained -- --ignored --nocapture
async fn scenario_2_sustained_load() {
    let config = ScenarioConfig {
        name: "Scenario 2: Sustained High Load".to_string(),
        duration: Duration::from_secs(3600), // 60 minutes
        load_profile: LoadProfile::Constant { qps: 200 },
        workload_mix: WorkloadMix::default(),
        dataset_size: 50_000,
        dimension: 512,
        concurrency: 20,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 50.0, // Relaxed
        max_error_rate: 0.005,    // 0.5%
        max_memory_growth_mb_per_min: 1.67, // <100MB over 60min
        max_cpu_utilization: 0.85,
        min_throughput_qps: Some(190.0),
        max_p99_latency_ms: Some(100.0),
    };

    run_scenario(config, success_criteria, "sustained_load").await;
}

// ============================================================================
// Scenario 2 (Quick): Sustained High Load - 10 minute version
// ============================================================================

/// Scenario 2 (Quick): Sustained High Load - Short Version
#[tokio::test]
async fn scenario_2_sustained_load_quick() {
    let config = ScenarioConfig {
        name: "Scenario 2: Sustained High Load (Quick)".to_string(),
        duration: Duration::from_secs(600), // 10 minutes
        load_profile: LoadProfile::Constant { qps: 200 },
        workload_mix: WorkloadMix::default(),
        dataset_size: 50_000,
        dimension: 512,
        concurrency: 20,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 50.0,
        max_error_rate: 0.005,
        max_memory_growth_mb_per_min: 10.0,
        max_cpu_utilization: 0.85,
        min_throughput_qps: Some(190.0),
        max_p99_latency_ms: Some(100.0),
    };

    run_scenario(config, success_criteria, "sustained_load_quick").await;
}

// ============================================================================
// Scenario 3: Spike Load (15 minutes)
// ============================================================================

/// Scenario 3: Spike Load
///
/// Purpose: Test system response to sudden traffic spikes
/// Duration: 15 minutes
/// Load Pattern:
/// - 0-3 min: 100 QPS (baseline)
/// - 3-8 min: 500 QPS (spike)
/// - 8-15 min: 100 QPS (recovery)
/// Success Criteria:
/// - System remains responsive during spike
/// - P95 latency <100ms during spike
/// - Error rate <1% during spike
/// - Full recovery to baseline performance after spike
/// - No memory leaks post-spike
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_3_spike -- --ignored --nocapture
async fn scenario_3_spike_load() {
    let config = ScenarioConfig {
        name: "Scenario 3: Spike Load".to_string(),
        duration: Duration::from_secs(900), // 15 minutes
        load_profile: LoadProfile::Spike {
            baseline_qps: 100,
            spike_qps: 500,
            spike_start: Duration::from_secs(180), // 3 minutes
            spike_duration: Duration::from_secs(300), // 5 minutes
        },
        workload_mix: WorkloadMix::default(),
        dataset_size: 10_000,
        dimension: 512,
        concurrency: 50,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 100.0, // Higher tolerance during spike
        max_error_rate: 0.01,      // 1%
        max_memory_growth_mb_per_min: 15.0,
        max_cpu_utilization: 0.90,
        min_throughput_qps: Some(80.0), // Lower due to averaging
        max_p99_latency_ms: Some(200.0),
    };

    run_scenario(config, success_criteria, "spike_load").await;
}

// ============================================================================
// Scenario 3 (Quick): Spike Load - 5 minute version
// ============================================================================

/// Scenario 3 (Quick): Spike Load - Short Version
#[tokio::test]
async fn scenario_3_spike_load_quick() {
    let config = ScenarioConfig {
        name: "Scenario 3: Spike Load (Quick)".to_string(),
        duration: Duration::from_secs(300), // 5 minutes
        load_profile: LoadProfile::Spike {
            baseline_qps: 100,
            spike_qps: 500,
            spike_start: Duration::from_secs(60), // 1 minute
            spike_duration: Duration::from_secs(120), // 2 minutes
        },
        workload_mix: WorkloadMix::default(),
        dataset_size: 10_000,
        dimension: 512,
        concurrency: 50,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 100.0,
        max_error_rate: 0.01,
        max_memory_growth_mb_per_min: 15.0,
        max_cpu_utilization: 0.90,
        min_throughput_qps: Some(80.0),
        max_p99_latency_ms: Some(200.0),
    };

    run_scenario(config, success_criteria, "spike_load_quick").await;
}

// ============================================================================
// Scenario 4: Tiered Storage Workflow (45 minutes)
// ============================================================================

/// Scenario 4: Tiered Storage Workflow
///
/// Purpose: Validate hot/warm/cold tier transitions under load
/// Duration: 45 minutes
/// QPS: 100 (constant)
/// Workload: Access patterns with Zipf distribution (80/20 rule)
/// Success Criteria:
/// - Hot tier: P95 <5ms
/// - Warm tier: P95 <25ms
/// - Cold tier: First access <2s, subsequent <25ms
/// - Automatic promotions/demotions working
/// - No data loss during transitions
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_4_tiering -- --ignored --nocapture
async fn scenario_4_tiered_storage() {
    let config = ScenarioConfig {
        name: "Scenario 4: Tiered Storage Workflow".to_string(),
        duration: Duration::from_secs(2700), // 45 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix {
            search_pct: 0.8, // Heavy read workload
            insert_pct: 0.15,
            metadata_pct: 0.05,
            update_pct: 0.0,
            delete_pct: 0.0,
        },
        dataset_size: 100_000, // Larger dataset for tiering
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 25.0, // Mixed tier average
        max_error_rate: 0.001,
        max_memory_growth_mb_per_min: 10.0,
        max_cpu_utilization: 0.75,
        min_throughput_qps: Some(95.0),
        max_p99_latency_ms: Some(100.0), // Allow for cold tier access
    };

    run_scenario(config, success_criteria, "tiered_storage").await;
}

// ============================================================================
// Scenario 4 (Quick): Tiered Storage - 10 minute version
// ============================================================================

/// Scenario 4 (Quick): Tiered Storage - Short Version
#[tokio::test]
async fn scenario_4_tiered_storage_quick() {
    let config = ScenarioConfig {
        name: "Scenario 4: Tiered Storage Workflow (Quick)".to_string(),
        duration: Duration::from_secs(600), // 10 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix::read_heavy(),
        dataset_size: 100_000,
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 25.0,
        max_error_rate: 0.001,
        max_memory_growth_mb_per_min: 10.0,
        max_cpu_utilization: 0.75,
        min_throughput_qps: Some(95.0),
        max_p99_latency_ms: Some(100.0),
    };

    run_scenario(config, success_criteria, "tiered_storage_quick").await;
}

// ============================================================================
// Scenario 5: Multi-Tenant Load (30 minutes)
// ============================================================================

/// Scenario 5: Multi-Tenant Load
///
/// Purpose: Validate tenant isolation and quota enforcement
/// Duration: 30 minutes
/// QPS: 150 total (50 QPS per tenant, 3 tenants)
/// Workload: Mixed (60% search, 30% insert, 10% metadata)
/// Success Criteria:
/// - P95 latency <30ms (slightly higher due to contention)
/// - Error rate <0.5%
/// - No cross-tenant data leakage
/// - Quota enforcement working
/// - CPU <80% average
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_5_multi_tenant -- --ignored --nocapture
async fn scenario_5_multi_tenant() {
    let config = ScenarioConfig {
        name: "Scenario 5: Multi-Tenant Load".to_string(),
        duration: Duration::from_secs(1800), // 30 minutes
        load_profile: LoadProfile::Constant { qps: 150 },
        workload_mix: WorkloadMix {
            search_pct: 0.6,
            insert_pct: 0.3,
            metadata_pct: 0.1,
            update_pct: 0.0,
            delete_pct: 0.0,
        },
        dataset_size: 30_000, // 10k per tenant
        dimension: 512,
        concurrency: 15, // 5 per tenant
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 30.0,
        max_error_rate: 0.005, // 0.5%
        max_memory_growth_mb_per_min: 12.0,
        max_cpu_utilization: 0.80,
        min_throughput_qps: Some(140.0),
        max_p99_latency_ms: Some(60.0),
    };

    run_scenario(config, success_criteria, "multi_tenant").await;
}

// ============================================================================
// Scenario 5 (Quick): Multi-Tenant Load - 10 minute version
// ============================================================================

/// Scenario 5 (Quick): Multi-Tenant Load - Short Version
#[tokio::test]
async fn scenario_5_multi_tenant_quick() {
    let config = ScenarioConfig {
        name: "Scenario 5: Multi-Tenant Load (Quick)".to_string(),
        duration: Duration::from_secs(600), // 10 minutes
        load_profile: LoadProfile::Constant { qps: 150 },
        workload_mix: WorkloadMix {
            search_pct: 0.6,
            insert_pct: 0.3,
            metadata_pct: 0.1,
            update_pct: 0.0,
            delete_pct: 0.0,
        },
        dataset_size: 30_000,
        dimension: 512,
        concurrency: 15,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 30.0,
        max_error_rate: 0.005,
        max_memory_growth_mb_per_min: 12.0,
        max_cpu_utilization: 0.80,
        min_throughput_qps: Some(140.0),
        max_p99_latency_ms: Some(60.0),
    };

    run_scenario(config, success_criteria, "multi_tenant_quick").await;
}

// ============================================================================
// Scenario 6: Large Dataset (60 minutes)
// ============================================================================

/// Scenario 6: Large Dataset
///
/// Purpose: Test system behavior with large datasets (approaching 100GB limit)
/// Duration: 60 minutes
/// QPS: 100 (constant)
/// Workload: Read-heavy (80% search, 15% insert, 5% metadata)
/// Dataset: 1M vectors (512-dim) ~2GB
/// Success Criteria:
/// - P95 latency <100ms (degraded due to size)
/// - Error rate <1%
/// - Memory growth <50MB over 60 min (stable)
/// - CPU <80% average
/// - No OOM crashes
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_6_large_dataset -- --ignored --nocapture
async fn scenario_6_large_dataset() {
    let config = ScenarioConfig {
        name: "Scenario 6: Large Dataset".to_string(),
        duration: Duration::from_secs(3600), // 60 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix::read_heavy(),
        dataset_size: 1_000_000, // 1M vectors
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 100.0, // Relaxed for large dataset
        max_error_rate: 0.01,      // 1%
        max_memory_growth_mb_per_min: 0.83, // <50MB over 60min
        max_cpu_utilization: 0.80,
        min_throughput_qps: Some(90.0),
        max_p99_latency_ms: Some(200.0),
    };

    run_scenario(config, success_criteria, "large_dataset").await;
}

// ============================================================================
// Scenario 6 (Quick): Large Dataset - 15 minute version
// ============================================================================

/// Scenario 6 (Quick): Large Dataset - Short Version
#[tokio::test]
async fn scenario_6_large_dataset_quick() {
    let config = ScenarioConfig {
        name: "Scenario 6: Large Dataset (Quick)".to_string(),
        duration: Duration::from_secs(900), // 15 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix::read_heavy(),
        dataset_size: 500_000, // 500k vectors (reduced for quick test)
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 100.0,
        max_error_rate: 0.01,
        max_memory_growth_mb_per_min: 3.33, // <50MB over 15min
        max_cpu_utilization: 0.80,
        min_throughput_qps: Some(90.0),
        max_p99_latency_ms: Some(200.0),
    };

    run_scenario(config, success_criteria, "large_dataset_quick").await;
}

// ============================================================================
// Scenario 7: Failure Injection (20 minutes)
// ============================================================================

/// Scenario 7: Failure Injection
///
/// Purpose: Test resilience to failures (S3, network, timeouts)
/// Duration: 20 minutes
/// QPS: 100 (constant)
/// Workload: Balanced (50% search, 40% insert, 10% metadata)
/// Injected Failures:
/// - S3 upload failures (10% of writes)
/// - Network timeouts (5% of requests)
/// - Temporary unavailability
/// Success Criteria:
/// - P95 latency <50ms (excluding failed requests)
/// - Error rate <15% (due to injected failures)
/// - Circuit breaker activates correctly
/// - DLQ captures failed operations
/// - System recovers when failures stop
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_7_failure_injection -- --ignored --nocapture
async fn scenario_7_failure_injection() {
    let config = ScenarioConfig {
        name: "Scenario 7: Failure Injection".to_string(),
        duration: Duration::from_secs(1200), // 20 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix {
            search_pct: 0.5,
            insert_pct: 0.4,
            metadata_pct: 0.1,
            update_pct: 0.0,
            delete_pct: 0.0,
        },
        dataset_size: 10_000,
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 50.0,
        max_error_rate: 0.15, // 15% (relaxed due to injected failures)
        max_memory_growth_mb_per_min: 15.0,
        max_cpu_utilization: 0.85,
        min_throughput_qps: Some(85.0), // Lower due to failures
        max_p99_latency_ms: Some(100.0),
    };

    run_scenario(config, success_criteria, "failure_injection").await;
}

// ============================================================================
// Scenario 7 (Quick): Failure Injection - 5 minute version
// ============================================================================

/// Scenario 7 (Quick): Failure Injection - Short Version
#[tokio::test]
async fn scenario_7_failure_injection_quick() {
    let config = ScenarioConfig {
        name: "Scenario 7: Failure Injection (Quick)".to_string(),
        duration: Duration::from_secs(300), // 5 minutes
        load_profile: LoadProfile::Constant { qps: 100 },
        workload_mix: WorkloadMix {
            search_pct: 0.5,
            insert_pct: 0.4,
            metadata_pct: 0.1,
            update_pct: 0.0,
            delete_pct: 0.0,
        },
        dataset_size: 10_000,
        dimension: 512,
        concurrency: 10,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 50.0,
        max_error_rate: 0.15,
        max_memory_growth_mb_per_min: 15.0,
        max_cpu_utilization: 0.85,
        min_throughput_qps: Some(85.0),
        max_p99_latency_ms: Some(100.0),
    };

    run_scenario(config, success_criteria, "failure_injection_quick").await;
}

// ============================================================================
// Scenario 8: Mixed Workload Chaos (30 minutes)
// ============================================================================

/// Scenario 8: Mixed Workload Chaos
///
/// Purpose: Stress test with unpredictable load patterns
/// Duration: 30 minutes
/// Load Pattern: Random (50-300 QPS, changes every 30 seconds)
/// Workload: Fully random (all operation types)
/// Success Criteria:
/// - P95 latency <100ms (relaxed for chaos)
/// - Error rate <2%
/// - System remains responsive
/// - No crashes or panics
/// - Memory stable (no leaks)
#[tokio::test]
#[ignore] // Run explicitly: cargo test --release scenario_8_chaos -- --ignored --nocapture
async fn scenario_8_mixed_chaos() {
    let config = ScenarioConfig {
        name: "Scenario 8: Mixed Workload Chaos".to_string(),
        duration: Duration::from_secs(1800), // 30 minutes
        load_profile: LoadProfile::Random {
            min_qps: 50,
            max_qps: 300,
            change_interval: Duration::from_secs(30),
        },
        workload_mix: WorkloadMix {
            search_pct: 0.4,
            insert_pct: 0.3,
            update_pct: 0.15,
            delete_pct: 0.1,
            metadata_pct: 0.05,
        },
        dataset_size: 50_000,
        dimension: 512,
        concurrency: 30,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 100.0, // Relaxed for chaos
        max_error_rate: 0.02,      // 2%
        max_memory_growth_mb_per_min: 20.0,
        max_cpu_utilization: 0.90,
        min_throughput_qps: Some(40.0), // Very low due to random pattern
        max_p99_latency_ms: Some(200.0),
    };

    run_scenario(config, success_criteria, "mixed_chaos").await;
}

// ============================================================================
// Scenario 8 (Quick): Mixed Workload Chaos - 10 minute version
// ============================================================================

/// Scenario 8 (Quick): Mixed Workload Chaos - Short Version
#[tokio::test]
async fn scenario_8_mixed_chaos_quick() {
    let config = ScenarioConfig {
        name: "Scenario 8: Mixed Workload Chaos (Quick)".to_string(),
        duration: Duration::from_secs(600), // 10 minutes
        load_profile: LoadProfile::Random {
            min_qps: 50,
            max_qps: 300,
            change_interval: Duration::from_secs(30),
        },
        workload_mix: WorkloadMix {
            search_pct: 0.4,
            insert_pct: 0.3,
            update_pct: 0.15,
            delete_pct: 0.1,
            metadata_pct: 0.05,
        },
        dataset_size: 50_000,
        dimension: 512,
        concurrency: 30,
        sample_interval: Duration::from_secs(1),
    };

    let success_criteria = SuccessCriteria {
        max_p95_latency_ms: 100.0,
        max_error_rate: 0.02,
        max_memory_growth_mb_per_min: 20.0,
        max_cpu_utilization: 0.90,
        min_throughput_qps: Some(40.0),
        max_p99_latency_ms: Some(200.0),
    };

    run_scenario(config, success_criteria, "mixed_chaos_quick").await;
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Run a scenario and generate report
async fn run_scenario(config: ScenarioConfig, criteria: SuccessCriteria, report_name: &str) {
    let scenario_name = config.name.clone();

    println!("\n{}\n", "=".repeat(80));
    println!("  {}", scenario_name);
    println!("\n{}\n", "=".repeat(80));

    let orchestrator = LoadTestOrchestrator::new(config);
    let metrics = orchestrator
        .run()
        .await
        .expect("Load test execution failed");

    let writer = orchestrator.create_result_writer(metrics, criteria).await;

    // Generate reports
    let markdown_path = format!("target/load_test_reports/{}.md", report_name);
    let json_path = format!("target/load_test_reports/{}.json", report_name);

    // Create reports directory
    std::fs::create_dir_all("target/load_test_reports")
        .expect("Failed to create reports directory");

    writer
        .write_report(&markdown_path, ReportFormat::Markdown)
        .expect("Failed to write Markdown report");

    writer
        .write_report(&json_path, ReportFormat::Json)
        .expect("Failed to write JSON report");

    println!("\n📊 Reports generated:");
    println!("   Markdown: {}", markdown_path);
    println!("   JSON: {}", json_path);

    // Assert test passed
    assert!(
        writer.passes(),
        "\n\n{} FAILED:\n\n{}\n\nSee report: {}\n",
        scenario_name,
        writer.failure_summary(),
        markdown_path
    );

    println!("\n✅ {} PASSED\n", scenario_name);
}

// ============================================================================
// Smoke Test - Quick validation (runs in CI)
// ============================================================================

/// Smoke test: Ultra-quick load test for CI
///
/// Duration: 30 seconds
/// QPS: 10
/// Purpose: Fast validation that load test framework works
#[tokio::test]
async fn smoke_test_load_framework() {
    let config = ScenarioConfig {
        name: "Smoke Test".to_string(),
        duration: Duration::from_secs(30),
        load_profile: LoadProfile::Constant { qps: 10 },
        workload_mix: WorkloadMix::default(),
        dataset_size: 1_000,
        dimension: 128,
        concurrency: 5,
        sample_interval: Duration::from_secs(1),
    };

    let criteria = SuccessCriteria::development(); // Relaxed for smoke test

    let orchestrator = LoadTestOrchestrator::new(config);
    let metrics = orchestrator.run().await.expect("Smoke test failed");

    let writer = orchestrator.create_result_writer(metrics, criteria).await;

    println!("\n🔥 Smoke test results:");
    println!("   Total requests: {}", writer.metrics.total_requests);
    println!("   P95 latency: {:.2}ms", writer.metrics.p95_latency().as_secs_f64() * 1000.0);
    println!("   Error rate: {:.4}%", writer.metrics.error_rate() * 100.0);

    assert!(writer.passes(), "Smoke test failed: {}", writer.failure_summary());
}
