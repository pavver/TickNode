use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{DateTime, Datelike, Utc};
use ticknode_shared::UsageBucket;
use tracing::{info, warn};

pub fn stats_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("stats")
}

pub fn month_file_name(dt: DateTime<Utc>) -> String {
    format!("usage-{:04}-{:02}.jsonl", dt.year(), dt.month())
}

pub fn append_hour_bucket(data_dir: &Path, bucket: &UsageBucket) -> anyhow::Result<()> {
    let dir = stats_dir(data_dir);
    fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create stats directory {}", dir.display()))?;

    let dt = DateTime::from_timestamp(bucket.timestamp, 0).unwrap_or_else(Utc::now);
    let path = dir.join(month_file_name(dt));

    let json = serde_json::to_string(bucket)
        .with_context(|| "failed to serialize usage bucket to JSON")?;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open usage file for append: {}", path.display()))?;

    writeln!(file, "{json}")
        .with_context(|| format!("failed to write usage bucket to {}", path.display()))?;
    file.flush()
        .with_context(|| format!("failed to flush usage file {}", path.display()))?;
    Ok(())
}

pub fn load_recent_hour_buckets(
    data_dir: &Path,
    max_count: usize,
) -> anyhow::Result<Vec<UsageBucket>> {
    let dir = stats_dir(data_dir);
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let files = collect_stats_files(&dir)?;
    let mut buckets: Vec<UsageBucket> = Vec::new();

    for file in files {
        read_buckets_from_file(&file, &mut buckets);
    }

    if buckets.len() > max_count {
        let excess = buckets.len() - max_count;
        buckets.drain(..excess);
    }

    Ok(buckets)
}

fn collect_stats_files(dir: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = Vec::new();
    let entries = fs::read_dir(dir)
        .with_context(|| format!("failed to read stats directory {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with("usage-")
            && name.ends_with(".jsonl")
        {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

fn read_buckets_from_file(file: &Path, buckets: &mut Vec<UsageBucket>) {
    let f = match fs::File::open(file) {
        Ok(f) => f,
        Err(e) => {
            warn!("Failed to open stats file {}: {e}", file.display());
            return;
        }
    };
    let reader = BufReader::new(f);
    for line in reader.lines() {
        match line {
            Ok(line_str) => {
                let trimmed = line_str.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<UsageBucket>(trimmed) {
                    Ok(b) => buckets.push(b),
                    Err(e) => warn!("Skipping invalid line in {}: {e}", file.display()),
                }
            }
            Err(e) => {
                warn!("Error reading line from {}: {e}", file.display());
                break;
            }
        }
    }
}

pub fn prune_old_stats(
    data_dir: &Path,
    keep_months: u32,
    now: DateTime<Utc>,
) -> anyhow::Result<usize> {
    let dir = stats_dir(data_dir);
    if !dir.exists() {
        return Ok(0);
    }

    let mut cutoff_year = now.year();
    let mut cutoff_month = now.month() as i32 - keep_months as i32;
    while cutoff_month <= 0 {
        cutoff_year -= 1;
        cutoff_month += 12;
    }
    let cutoff_str = format!("usage-{:04}-{:02}.jsonl", cutoff_year, cutoff_month);

    let mut pruned = 0;
    let entries = fs::read_dir(&dir)
        .with_context(|| format!("failed to read stats directory {}", dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.starts_with("usage-")
            && name.ends_with(".jsonl")
            && name < cutoff_str.as_str()
        {
            info!("Pruning old usage stats file: {}", name);
            if let Err(e) = fs::remove_file(&path) {
                warn!("Failed to remove old stats file {}: {e}", path.display());
            } else {
                pruned += 1;
            }
        }
    }

    Ok(pruned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_load_and_prune() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path();

        let b1 = UsageBucket {
            timestamp: 1700000000,
            ticks: 100,
            ..Default::default()
        };
        append_hour_bucket(data_dir, &b1).unwrap();

        let b2 = UsageBucket {
            timestamp: 1700003600,
            ticks: 200,
            ..Default::default()
        };
        append_hour_bucket(data_dir, &b2).unwrap();

        let loaded = load_recent_hour_buckets(data_dir, 10).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].ticks, 100);
        assert_eq!(loaded[1].ticks, 200);

        let loaded_capped = load_recent_hour_buckets(data_dir, 1).unwrap();
        assert_eq!(loaded_capped.len(), 1);
        assert_eq!(loaded_capped[0].ticks, 200);

        let old_time = DateTime::from_timestamp(1700000000, 0).unwrap();
        let future_now = old_time + chrono::Duration::days(250);
        let pruned = prune_old_stats(data_dir, 6, future_now).unwrap();
        assert_eq!(pruned, 1);

        let after_prune = load_recent_hour_buckets(data_dir, 10).unwrap();
        assert_eq!(after_prune.len(), 0);
    }

    #[test]
    fn test_stats_edge_cases() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path();

        // Non-existent stats dir
        assert_eq!(load_recent_hour_buckets(data_dir, 10).unwrap().len(), 0);
        assert_eq!(prune_old_stats(data_dir, 6, Utc::now()).unwrap(), 0);

        // Invalid line and empty lines in file
        let dir = stats_dir(data_dir);
        fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("usage-2023-11.jsonl");
        let valid_bucket = UsageBucket {
            timestamp: 1700000000,
            ticks: 5,
            ..Default::default()
        };
        let valid_json = serde_json::to_string(&valid_bucket).unwrap();
        fs::write(&file_path, format!("\n   \n{valid_json}\nnot json\n")).unwrap();

        let loaded = load_recent_hour_buckets(data_dir, 10).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].ticks, 5);
    }
}
