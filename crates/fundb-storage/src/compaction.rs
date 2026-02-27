//! Compaction policy for the LSM-tree storage engine — STORY-4-4
//!
//! Two policies are supported:
//!
//! - [`CompactionPolicy::Leveled`] — approximates RocksDB-style leveled
//!   compaction.  For simplicity the heuristic is: compact when the total
//!   number of SSTable files exceeds `max_levels * 4`.  Files selected for
//!   compaction are the oldest `ceil(len / 2)` by filename.
//!
//! - [`CompactionPolicy::SizeTiered`] — compact when the file count reaches
//!   `min_sstable_count`, selecting the oldest `min_sstable_count` files.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// CompactionPolicy
// ---------------------------------------------------------------------------

/// Selects when and which SSTable files to merge.
#[derive(Debug, Clone)]
pub enum CompactionPolicy {
    /// Leveled compaction (approximated).
    ///
    /// `level_size_ratio` is kept for future use; the current trigger
    /// heuristic is purely based on total file count vs. `max_levels`.
    Leveled {
        /// Ratio between consecutive levels (future use).
        level_size_ratio: u32,
        /// Maximum number of levels before compaction is triggered.
        max_levels: usize,
    },

    /// Size-tiered compaction.
    ///
    /// Triggers as soon as `min_sstable_count` files accumulate.
    SizeTiered {
        /// Minimum number of SSTable files that must exist before compaction.
        min_sstable_count: usize,
    },
}

impl CompactionPolicy {
    /// Return `true` if the given list of SSTable files warrants a compaction
    /// run according to this policy.
    ///
    /// # Arguments
    /// * `sstable_files` — all current `.sst` file paths in the storage
    ///   directory, in any order.
    pub fn should_compact(&self, sstable_files: &[PathBuf]) -> bool {
        match self {
            CompactionPolicy::Leveled { max_levels, .. } => {
                // Heuristic: trigger when total file count exceeds max_levels * 4.
                sstable_files.len() > max_levels * 4
            }
            CompactionPolicy::SizeTiered {
                min_sstable_count,
            } => sstable_files.len() >= *min_sstable_count,
        }
    }

    /// Choose which SSTable files should participate in the next compaction.
    ///
    /// Files are assumed to be sorted **ascending by filename** (oldest
    /// first) by the caller.  Both policies select the oldest files to merge.
    ///
    /// # Arguments
    /// * `sstable_files` — all current `.sst` file paths, sorted oldest-first
    ///   by filename.
    ///
    /// # Returns
    /// A `Vec<PathBuf>` of the files that should be merged together.  The
    /// returned slice is always a prefix of `sstable_files`.  If there are
    /// fewer files than the minimum required for this policy an empty `Vec`
    /// is returned.
    pub fn select_files(&self, sstable_files: &[PathBuf]) -> Vec<PathBuf> {
        match self {
            CompactionPolicy::Leveled { .. } => {
                // Select the oldest ceil(len / 2) files.
                let n = sstable_files.len();
                if n == 0 {
                    return vec![];
                }
                let take = (n + 1) / 2; // ceil division
                sstable_files[..take].to_vec()
            }
            CompactionPolicy::SizeTiered {
                min_sstable_count,
            } => {
                let n = sstable_files.len();
                if n < *min_sstable_count {
                    return vec![];
                }
                sstable_files[..*min_sstable_count].to_vec()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(|n| PathBuf::from(n)).collect()
    }

    // -----------------------------------------------------------------------
    // Leveled — should_compact
    // -----------------------------------------------------------------------

    #[test]
    fn test_leveled_should_compact_false_below_threshold() {
        let policy = CompactionPolicy::Leveled {
            level_size_ratio: 10,
            max_levels: 7,
        };
        // 7 * 4 = 28; 20 files is below the threshold.
        let files = paths(&(0..20).map(|i| format!("{:020}.sst", i)).collect::<Vec<_>>()
            .iter().map(|s| s.as_str()).collect::<Vec<_>>());
        assert!(!policy.should_compact(&files));
    }

    #[test]
    fn test_leveled_should_compact_true_above_threshold() {
        let policy = CompactionPolicy::Leveled {
            level_size_ratio: 10,
            max_levels: 7,
        };
        // 7 * 4 = 28; 29 files exceeds the threshold.
        let files: Vec<PathBuf> = (0..29).map(|i| PathBuf::from(format!("{:020}.sst", i))).collect();
        assert!(policy.should_compact(&files));
    }

    // -----------------------------------------------------------------------
    // Leveled — select_files
    // -----------------------------------------------------------------------

    #[test]
    fn test_leveled_select_ceil_half() {
        let policy = CompactionPolicy::Leveled {
            level_size_ratio: 10,
            max_levels: 7,
        };
        // 10 files → ceil(10 / 2) = 5 selected.
        let files: Vec<PathBuf> = (0..10u32)
            .map(|i| PathBuf::from(format!("{:020}.sst", i)))
            .collect();
        let selected = policy.select_files(&files);
        assert_eq!(selected.len(), 5);
        // The oldest five must be returned.
        for (i, path) in selected.iter().enumerate() {
            assert_eq!(path, &files[i]);
        }
    }

    #[test]
    fn test_leveled_select_odd_count() {
        let policy = CompactionPolicy::Leveled {
            level_size_ratio: 10,
            max_levels: 7,
        };
        // 7 files → ceil(7 / 2) = 4 selected.
        let files: Vec<PathBuf> = (0..7u32)
            .map(|i| PathBuf::from(format!("{:020}.sst", i)))
            .collect();
        let selected = policy.select_files(&files);
        assert_eq!(selected.len(), 4);
    }

    #[test]
    fn test_leveled_select_empty() {
        let policy = CompactionPolicy::Leveled {
            level_size_ratio: 10,
            max_levels: 7,
        };
        let selected = policy.select_files(&[]);
        assert!(selected.is_empty());
    }

    // -----------------------------------------------------------------------
    // SizeTiered — should_compact
    // -----------------------------------------------------------------------

    #[test]
    fn test_size_tiered_should_compact_false() {
        let policy = CompactionPolicy::SizeTiered {
            min_sstable_count: 4,
        };
        let files: Vec<PathBuf> = (0..3u32)
            .map(|i| PathBuf::from(format!("{:020}.sst", i)))
            .collect();
        assert!(!policy.should_compact(&files));
    }

    #[test]
    fn test_size_tiered_should_compact_true_at_threshold() {
        let policy = CompactionPolicy::SizeTiered {
            min_sstable_count: 4,
        };
        let files: Vec<PathBuf> = (0..4u32)
            .map(|i| PathBuf::from(format!("{:020}.sst", i)))
            .collect();
        assert!(policy.should_compact(&files));
    }

    // -----------------------------------------------------------------------
    // SizeTiered — select_files
    // -----------------------------------------------------------------------

    #[test]
    fn test_size_tiered_select_files() {
        let policy = CompactionPolicy::SizeTiered {
            min_sstable_count: 4,
        };
        let files: Vec<PathBuf> = (0..10u32)
            .map(|i| PathBuf::from(format!("{:020}.sst", i)))
            .collect();
        let selected = policy.select_files(&files);
        assert_eq!(selected.len(), 4);
        for (i, path) in selected.iter().enumerate() {
            assert_eq!(path, &files[i]);
        }
    }

    #[test]
    fn test_size_tiered_select_empty_when_below_min() {
        let policy = CompactionPolicy::SizeTiered {
            min_sstable_count: 4,
        };
        let files: Vec<PathBuf> = (0..3u32)
            .map(|i| PathBuf::from(format!("{:020}.sst", i)))
            .collect();
        let selected = policy.select_files(&files);
        assert!(selected.is_empty());
    }
}
