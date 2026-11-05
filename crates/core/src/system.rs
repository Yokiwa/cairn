//! Facts about the computer the launcher runs on.

/// Total physical memory in MB (0 if it can't be read).
pub fn total_memory_mb() -> u64 {
    let mut s = sysinfo::System::new();
    s.refresh_memory();
    s.total_memory() / (1024 * 1024)
}

#[cfg(test)]
mod tests {
    #[test]
    fn reports_some_memory() {
        assert!(super::total_memory_mb() > 256);
    }
}
