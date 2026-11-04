//! Minecraft versions: manifest, version JSON, rules, installation.

pub mod install;
pub mod manifest;
pub mod maven;
pub mod rules;
pub mod version_json;

use std::cmp::Ordering;

/// Compare version ids numerically, segment by segment ("1.20.5" < "1.20.10",
/// "26.3" > "1.21.4"). Non-numeric suffixes such as "-snapshot-7" are ignored;
/// missing segments count as 0.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    fn nums(s: &str) -> Vec<u64> {
        let core = s.split(['-', ' ', '+']).next().unwrap_or("");
        core.split('.')
            .map(|seg| {
                seg.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
                    .parse()
                    .unwrap_or(0)
            })
            .collect()
    }
    let (x, y) = (nums(a), nums(b));
    for i in 0..x.len().max(y.len()) {
        match x.get(i).unwrap_or(&0).cmp(y.get(i).unwrap_or(&0)) {
            Ordering::Equal => continue,
            o => return o,
        }
    }
    Ordering::Equal
}
