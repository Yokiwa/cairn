use crate::{AppError, Result};

/// `group:artifact:version[:classifier][@ext]` → relative repository path.
pub fn maven_path(coord: &str) -> Result<String> {
    let (main, ext) = match coord.split_once('@') {
        Some((m, e)) => (m, e),
        None => (coord, "jar"),
    };
    let parts: Vec<&str> = main.split(':').collect();
    if !(3..=4).contains(&parts.len()) || parts.iter().any(|p| p.is_empty()) {
        return Err(
            AppError::new("error.parse").with_detail(format!("bad maven coordinate: {coord}"))
        );
    }
    let (group, artifact, version) = (parts[0].replace('.', "/"), parts[1], parts[2]);
    let classifier = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    Ok(format!(
        "{group}/{artifact}/{version}/{artifact}-{version}{classifier}.{ext}"
    ))
}

/// `group:artifact[:classifier]`, i.e. the coordinate without its version.
pub fn artifact_key(coord: &str) -> String {
    let main = coord.split('@').next().unwrap_or(coord);
    let parts: Vec<&str> = main.split(':').collect();
    match parts.len() {
        4 => format!("{}:{}:{}", parts[0], parts[1], parts[3]),
        n if n >= 2 => format!("{}:{}", parts[0], parts[1]),
        _ => main.to_string(),
    }
}
