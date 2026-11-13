use super::types::Score;
use crate::core::json::write_python_json;

#[must_use]
pub fn score_reply(score: &Score) -> String {
    format!(
        "dejafix Prevention Score: {} ({}/100)\n  Failed approaches on record: {}\n  Decisions documented: {}\n  Fixes with context: {}\n  Debugging hours saved: ~{}h\n  Tokens saved: {}\n  Estimated USD saved: ${:.2}",
        score.grade,
        score.score,
        score.failed,
        score.decisions,
        score.fixes,
        python_float(score.hours),
        thousands(score.tokens),
        score.usd,
    )
}

fn python_float(value: f64) -> String {
    let mut out = Vec::new();
    match write_python_json(&mut out, &value) {
        Ok(()) => String::from_utf8_lossy(&out).into_owned(),
        Err(_) => value.to_string(),
    }
}

fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}
