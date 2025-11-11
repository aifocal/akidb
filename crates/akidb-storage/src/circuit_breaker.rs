//! Circuit breaker pattern implementation for S3 operations
//!
//! Prevents cascade failures during S3 outages by tracking error rates
//! and temporarily blocking retries when error thresholds are exceeded.
//!
//! # States
//!
//! - **Closed:** Normal operation, all requests allowed
//! - **Open:** Circuit tripped due to high error rate, requests rejected
//! - **HalfOpen:** Testing recovery, limited requests allowed
//!
//! # Example
//!
//! ```rust
//! use akidb_storage::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
//! use std::time::Duration;
//!
//! let config = CircuitBreakerConfig {
//!     failure_threshold: 0.5,  // 50% error rate
//!     window_duration: Duration::from_secs(60),
//!     cooldown_duration: Duration::from_secs(300),
//!     half_open_successes: 10,
//! };
//!
//! let cb = CircuitBreaker::new(config);
//!
//! // Check if request should be allowed
//! if cb.should_allow_request() {
//!     // Perform operation
//!     let success = true; // or false
//!     cb.record_result(success);
//! }
//! ```

use parking_lot::RwLock;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Circuit breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Normal operation, retries enabled.
    Closed,

    /// Circuit breaker tripped, all retries rejected.
    Open,

    /// Testing recovery, limited retries allowed.
    HalfOpen,
}

impl CircuitBreakerState {
    /// Convert state to numeric value for metrics.
    ///
    /// 0 = Closed, 1 = Open, 2 = HalfOpen
    #[must_use]
    pub fn to_metric(&self) -> u8 {
        match self {
            Self::Closed => 0,
            Self::Open => 1,
            Self::HalfOpen => 2,
        }
    }
}

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Failure rate threshold to trip circuit (0.0-1.0).
    /// Default: 0.5 (50%)
    pub failure_threshold: f64,

    /// Error rate tracking window duration.
    /// Default: 60 seconds
    pub window_duration: Duration,

    /// Cooldown period before transitioning to HalfOpen.
    /// Default: 300 seconds (5 minutes)
    pub cooldown_duration: Duration,

    /// Number of consecutive successes required to close circuit.
    /// Default: 10
    pub half_open_successes: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 0.5,
            window_duration: Duration::from_secs(60),
            cooldown_duration: Duration::from_secs(300),
            half_open_successes: 10,
        }
    }
}

/// Error rate tracking for circuit breaker.
#[derive(Debug)]
struct ErrorRateTracker {
    /// Sliding window of request results (true = success, false = failure).
    window: Vec<(Instant, bool)>,

    /// Window duration.
    window_duration: Duration,
}

impl ErrorRateTracker {
    fn new(window_duration: Duration) -> Self {
        Self {
            window: Vec::new(),
            window_duration,
        }
    }

    /// Record a request result.
    fn record(&mut self, success: bool) {
        let now = Instant::now();

        // Add new result
        self.window.push((now, success));

        // Remove old results outside window
        let cutoff = now - self.window_duration;
        self.window.retain(|(timestamp, _)| *timestamp >= cutoff);
    }

    /// Calculate current error rate (0.0-1.0).
    fn error_rate(&self) -> f64 {
        if self.window.is_empty() {
            return 0.0;
        }

        let total = self.window.len();
        let failures = self.window.iter().filter(|(_, success)| !success).count();

        failures as f64 / total as f64
    }

    /// Get total requests in window.
    fn total_requests(&self) -> usize {
        self.window.len()
    }
}

/// Circuit breaker implementation.
pub struct CircuitBreaker {
    /// Current state.
    state: Arc<RwLock<CircuitBreakerState>>,

    /// Configuration.
    config: CircuitBreakerConfig,

    /// Error rate tracker.
    error_tracker: Arc<RwLock<ErrorRateTracker>>,

    /// Last state transition time.
    last_transition: Arc<RwLock<Instant>>,

    /// Consecutive successes in HalfOpen state.
    half_open_successes: Arc<RwLock<u32>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker in Closed state.
    #[must_use]
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitBreakerState::Closed)),
            config: config.clone(),
            error_tracker: Arc::new(RwLock::new(ErrorRateTracker::new(config.window_duration))),
            last_transition: Arc::new(RwLock::new(Instant::now())),
            half_open_successes: Arc::new(RwLock::new(0)),
        }
    }

    /// Get current state.
    #[must_use]
    pub fn state(&self) -> CircuitBreakerState {
        *self.state.read()
    }

    /// Get current error rate.
    #[must_use]
    pub fn error_rate(&self) -> f64 {
        self.error_tracker.read().error_rate()
    }

    /// Check if request should be allowed.
    ///
    /// Returns true if request should proceed, false if rejected.
    #[must_use]
    pub fn should_allow_request(&self) -> bool {
        let state = *self.state.read();

        match state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => {
                // Check if cooldown period has elapsed (without holding lock)
                let elapsed = {
                    let last_transition = self.last_transition.read();
                    last_transition.elapsed()
                };

                if elapsed >= self.config.cooldown_duration {
                    self.transition_to_half_open();
                    true // Allow test request
                } else {
                    false // Reject during cooldown
                }
            }
            CircuitBreakerState::HalfOpen => true, // Allow test requests
        }
    }

    /// Record request result.
    pub fn record_result(&self, success: bool) {
        let state = *self.state.read();

        // Record in error tracker
        self.error_tracker.write().record(success);

        match state {
            CircuitBreakerState::Closed => {
                // Check if should trip to Open (read metrics without holding lock)
                let (error_rate, total_requests) = {
                    let tracker = self.error_tracker.read();
                    (tracker.error_rate(), tracker.total_requests())
                };

                // Require minimum 10 requests before checking error rate
                if total_requests >= 10 && error_rate > self.config.failure_threshold {
                    tracing::warn!(
                        "Circuit breaker tripping: error_rate={:.2}%, threshold={:.2}%",
                        error_rate * 100.0,
                        self.config.failure_threshold * 100.0
                    );
                    self.transition_to_open();
                }
            }
            CircuitBreakerState::HalfOpen => {
                if success {
                    let new_count = {
                        let mut successes = self.half_open_successes.write();
                        *successes += 1;
                        *successes
                    };

                    if new_count >= self.config.half_open_successes {
                        tracing::info!(
                            "Circuit breaker closing after {} consecutive successes",
                            new_count
                        );
                        self.transition_to_closed();
                    }
                } else {
                    tracing::warn!("Circuit breaker failure during HalfOpen, reopening");
                    self.transition_to_open();
                }
            }
            CircuitBreakerState::Open => {
                // No action needed in Open state
            }
        }
    }

    /// Force transition to Closed state (manual reset).
    pub fn reset(&self) {
        tracing::info!("Circuit breaker manually reset to Closed");
        *self.state.write() = CircuitBreakerState::Closed;
        *self.last_transition.write() = Instant::now();
        *self.half_open_successes.write() = 0;

        // Clear error tracker
        self.error_tracker.write().window.clear();
    }

    fn transition_to_open(&self) {
        *self.state.write() = CircuitBreakerState::Open;
        *self.last_transition.write() = Instant::now();
        *self.half_open_successes.write() = 0;
    }

    fn transition_to_half_open(&self) {
        tracing::info!("Circuit breaker transitioning to HalfOpen for testing");
        *self.state.write() = CircuitBreakerState::HalfOpen;
        *self.last_transition.write() = Instant::now();
        *self.half_open_successes.write() = 0;
    }

    fn transition_to_closed(&self) {
        *self.state.write() = CircuitBreakerState::Closed;
        *self.last_transition.write() = Instant::now();
        *self.half_open_successes.write() = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_circuit_breaker_closed_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 0.5,
            window_duration: Duration::from_secs(60),
            cooldown_duration: Duration::from_secs(5),
            half_open_successes: 10,
        };

        let cb = CircuitBreaker::new(config);

        // Initial state should be Closed
        assert_eq!(cb.state(), CircuitBreakerState::Closed);

        // Record 10 failures (100% error rate)
        for _ in 0..10 {
            assert!(cb.should_allow_request());
            cb.record_result(false);
        }

        // Should transition to Open
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    #[test]
    fn test_circuit_breaker_open_to_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 0.5,
            window_duration: Duration::from_secs(60),
            cooldown_duration: Duration::from_millis(100), // Short for testing
            half_open_successes: 10,
        };

        let cb = CircuitBreaker::new(config);

        // Force to Open state
        for _ in 0..10 {
            cb.record_result(false);
        }
        assert_eq!(cb.state(), CircuitBreakerState::Open);

        // Requests should be rejected during cooldown
        assert!(!cb.should_allow_request());

        // Wait for cooldown
        thread::sleep(Duration::from_millis(150));

        // Should allow test request and transition to HalfOpen
        assert!(cb.should_allow_request());
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
    }

    #[test]
    fn test_circuit_breaker_half_open_to_closed() {
        let config = CircuitBreakerConfig {
            failure_threshold: 0.5,
            window_duration: Duration::from_secs(60),
            cooldown_duration: Duration::from_millis(100),
            half_open_successes: 5, // Lower for faster test
        };

        let cb = CircuitBreaker::new(config);

        // Force to HalfOpen state
        for _ in 0..10 {
            cb.record_result(false);
        }
        thread::sleep(Duration::from_millis(150));
        let _ = cb.should_allow_request(); // Transition to HalfOpen

        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        // Record 5 consecutive successes
        for _ in 0..5 {
            assert!(cb.should_allow_request());
            cb.record_result(true);
        }

        // Should transition to Closed
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_circuit_breaker_half_open_to_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 0.5,
            window_duration: Duration::from_secs(60),
            cooldown_duration: Duration::from_millis(100),
            half_open_successes: 10,
        };

        let cb = CircuitBreaker::new(config);

        // Force to HalfOpen state
        for _ in 0..10 {
            cb.record_result(false);
        }
        thread::sleep(Duration::from_millis(150));
        let _ = cb.should_allow_request();

        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        // Record success, then failure
        cb.record_result(true);
        cb.record_result(false);

        // Should transition back to Open
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    #[test]
    fn test_error_rate_tracking() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new(config);

        // Record 6 successes, 4 failures (40% error rate)
        for _ in 0..6 {
            cb.record_result(true);
        }
        for _ in 0..4 {
            cb.record_result(false);
        }

        let error_rate = cb.error_rate();
        assert!((error_rate - 0.4).abs() < 0.01); // 40% ± 1%

        // Should remain Closed (below 50% threshold)
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_manual_reset() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new(config);

        // Force to Open state
        for _ in 0..10 {
            cb.record_result(false);
        }
        assert_eq!(cb.state(), CircuitBreakerState::Open);

        // Manual reset
        cb.reset();

        // Should be Closed
        assert_eq!(cb.state(), CircuitBreakerState::Closed);

        // Error rate should be reset
        assert_eq!(cb.error_rate(), 0.0);
    }
}
