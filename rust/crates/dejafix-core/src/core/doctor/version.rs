pub fn version_key(version: &str) -> Vec<u64> {
    version.split('.').take(3).map(chunk_number).collect()
}

#[must_use]
pub fn is_newer(candidate: &str, current: &str) -> bool {
    version_key(candidate) > version_key(current)
}

fn chunk_number(chunk: &str) -> u64 {
    let digits: String = chunk.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        0
    } else {
        digits.parse().unwrap_or(u64::MAX)
    }
}
