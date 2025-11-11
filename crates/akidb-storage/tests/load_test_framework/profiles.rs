//! Load profiles and workload configuration

use std::time::Duration;

/// Load profile defines how QPS changes over time
#[derive(Debug, Clone)]
pub enum LoadProfile {
    /// Constant QPS throughout the test
    Constant {
        qps: usize,
    },

    /// Ramp from one QPS to another over duration
    Ramp {
        from_qps: usize,
        to_qps: usize,
        ramp_duration: Duration,
    },

    /// Spike pattern: baseline -> spike -> baseline
    Spike {
        baseline_qps: usize,
        spike_qps: usize,
        spike_start: Duration,
        spike_duration: Duration,
    },

    /// Random QPS within range
    Random {
        min_qps: usize,
        max_qps: usize,
        change_interval: Duration,
    },
}

impl LoadProfile {
    /// Get QPS at a given time offset from test start
    pub fn qps_at(&self, elapsed: Duration) -> usize {
        match self {
            Self::Constant { qps } => *qps,

            Self::Ramp {
                from_qps,
                to_qps,
                ramp_duration,
            } => {
                if elapsed >= *ramp_duration {
                    *to_qps
                } else {
                    let progress = elapsed.as_secs_f64() / ramp_duration.as_secs_f64();
                    let delta = (*to_qps as f64 - *from_qps as f64) * progress;
                    (*from_qps as f64 + delta) as usize
                }
            }

            Self::Spike {
                baseline_qps,
                spike_qps,
                spike_start,
                spike_duration,
            } => {
                let spike_end = *spike_start + *spike_duration;
                if elapsed >= *spike_start && elapsed < spike_end {
                    *spike_qps
                } else {
                    *baseline_qps
                }
            }

            Self::Random {
                min_qps,
                max_qps,
                change_interval,
            } => {
                // Use deterministic random based on interval
                let interval_idx = elapsed.as_secs() / change_interval.as_secs();
                let range = max_qps - min_qps;
                let pseudo_random = (interval_idx * 1103515245 + 12345) % range as u64;
                min_qps + pseudo_random as usize
            }
        }
    }

    /// Get description of this load profile
    pub fn description(&self) -> String {
        match self {
            Self::Constant { qps } => format!("Constant {} QPS", qps),
            Self::Ramp {
                from_qps, to_qps, ..
            } => format!("Ramp {} → {} QPS", from_qps, to_qps),
            Self::Spike {
                baseline_qps,
                spike_qps,
                ..
            } => format!("Spike {} → {} QPS", baseline_qps, spike_qps),
            Self::Random { min_qps, max_qps, .. } => {
                format!("Random {}-{} QPS", min_qps, max_qps)
            }
        }
    }
}

/// Workload mix defines the percentage of different operation types
#[derive(Debug, Clone)]
pub struct WorkloadMix {
    /// Percentage of search operations (0.0-1.0)
    pub search_pct: f32,

    /// Percentage of insert operations (0.0-1.0)
    pub insert_pct: f32,

    /// Percentage of update operations (0.0-1.0)
    pub update_pct: f32,

    /// Percentage of delete operations (0.0-1.0)
    pub delete_pct: f32,

    /// Percentage of metadata operations (list, get info) (0.0-1.0)
    pub metadata_pct: f32,
}

impl Default for WorkloadMix {
    fn default() -> Self {
        Self {
            search_pct: 0.7,
            insert_pct: 0.2,
            metadata_pct: 0.1,
            update_pct: 0.0,
            delete_pct: 0.0,
        }
    }
}

impl WorkloadMix {
    /// Validate that percentages sum to 1.0
    pub fn validate(&self) -> Result<(), String> {
        let sum = self.search_pct
            + self.insert_pct
            + self.update_pct
            + self.delete_pct
            + self.metadata_pct;

        if (sum - 1.0).abs() > 0.01 {
            return Err(format!(
                "Workload percentages sum to {}, expected 1.0",
                sum
            ));
        }

        Ok(())
    }

    /// Read-heavy workload (90% searches, 10% inserts)
    pub fn read_heavy() -> Self {
        Self {
            search_pct: 0.9,
            insert_pct: 0.08,
            metadata_pct: 0.02,
            update_pct: 0.0,
            delete_pct: 0.0,
        }
    }

    /// Write-heavy workload (30% searches, 60% inserts, 10% updates)
    pub fn write_heavy() -> Self {
        Self {
            search_pct: 0.3,
            insert_pct: 0.6,
            metadata_pct: 0.0,
            update_pct: 0.1,
            delete_pct: 0.0,
        }
    }

    /// Balanced workload (equal search and insert)
    pub fn balanced() -> Self {
        Self {
            search_pct: 0.5,
            insert_pct: 0.4,
            metadata_pct: 0.05,
            update_pct: 0.05,
            delete_pct: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_profile() {
        let profile = LoadProfile::Constant { qps: 100 };

        assert_eq!(profile.qps_at(Duration::from_secs(0)), 100);
        assert_eq!(profile.qps_at(Duration::from_secs(10)), 100);
        assert_eq!(profile.qps_at(Duration::from_secs(60)), 100);
    }

    #[test]
    fn test_ramp_profile() {
        let profile = LoadProfile::Ramp {
            from_qps: 50,
            to_qps: 200,
            ramp_duration: Duration::from_secs(10),
        };

        assert_eq!(profile.qps_at(Duration::from_secs(0)), 50);
        assert_eq!(profile.qps_at(Duration::from_secs(5)), 125); // Midpoint
        assert_eq!(profile.qps_at(Duration::from_secs(10)), 200);
        assert_eq!(profile.qps_at(Duration::from_secs(15)), 200); // After ramp
    }

    #[test]
    fn test_spike_profile() {
        let profile = LoadProfile::Spike {
            baseline_qps: 100,
            spike_qps: 500,
            spike_start: Duration::from_secs(5),
            spike_duration: Duration::from_secs(3),
        };

        assert_eq!(profile.qps_at(Duration::from_secs(0)), 100); // Before spike
        assert_eq!(profile.qps_at(Duration::from_secs(6)), 500); // During spike
        assert_eq!(profile.qps_at(Duration::from_secs(10)), 100); // After spike
    }

    #[test]
    fn test_workload_mix_validation() {
        let valid = WorkloadMix::default();
        assert!(valid.validate().is_ok());

        let invalid = WorkloadMix {
            search_pct: 0.5,
            insert_pct: 0.5,
            metadata_pct: 0.5, // Sum > 1.0
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_workload_presets() {
        assert!(WorkloadMix::read_heavy().validate().is_ok());
        assert!(WorkloadMix::write_heavy().validate().is_ok());
        assert!(WorkloadMix::balanced().validate().is_ok());
    }
}
