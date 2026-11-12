use super::patterns::compiled;
use super::{REDACTABLE_FIELDS, SecretKind, redact, redaction_notice};

fn openai_key() -> String {
    format!("{}{}", "sk-", "a".repeat(48))
}

fn jwt() -> String {
    let segment = format!("eyJ{}", "a".repeat(30));
    format!("{segment}.eyJ{}.{}", "b".repeat(30), "c".repeat(30))
}

fn true_positives() -> Vec<(String, &'static str)> {
    vec![
        (
            format!("hit the API with {} key", openai_key()),
            "openai_key",
        ),
        (
            format!("token={}{}", "ghp_", "a".repeat(36)),
            "github_token",
        ),
        (
            format!("github_pat_{}_{}", "a".repeat(22), "b".repeat(59)),
            "github_pat",
        ),
        (
            format!("export AWS_ACCESS_KEY_ID=AKIA{}", "A".repeat(16)),
            "aws_access_key",
        ),
        (format!("config: AIza{}", "a".repeat(35)), "google_api_key"),
        (
            format!("send to xoxb-{}-{}", "1".repeat(12), "a".repeat(24)),
            "slack_token",
        ),
        (
            format!("stripe key sk_live_{}", "a".repeat(24)),
            "stripe_key",
        ),
        (
            format!("publishable pk_test_{}", "a".repeat(24)),
            "stripe_key",
        ),
        (format!("Authorization: {}", jwt()), "jwt"),
        (
            format!("curl -H 'Authorization: Bearer {}'", "a".repeat(30)),
            "bearer_token",
        ),
        (
            "config: -----BEGIN RSA PRIVATE KEY-----".to_owned(),
            "private_key",
        ),
    ]
}

#[test]
fn every_pattern_compiles() {
    assert_eq!(compiled(), 10);
}

#[test]
fn redacts_known_secret_shapes() {
    for (input, kind) in true_positives() {
        let out = redact(&input);
        assert!(
            out.fired.iter().any(|fired| fired.as_str() == kind),
            "{input}"
        );
        assert!(out.text.contains(&format!("[REDACTED:{kind}]")), "{input}");
    }
    let input = format!("x {} y", openai_key());
    let out = redact(&input);
    assert!(!out.text.contains("sk-aaaaaaaaaa"));
}

#[test]
fn does_not_touch_ordinary_prose() {
    for safe in [
        "tried `contain: layout` in styles.css and it did not fix the layout shift",
        "summary: bug in src/auth.py — login form submits twice",
        "Decision: use bcrypt rounds=12 for password hashing.",
        "OPENAI_API_KEY is required in the environment for this script to run.",
        ".dejafix/events.jsonl",
        "sk- short prefix only",
        "ghp_short",
        "Forgot password reset flow needs a token email.",
        "use bearer auth",
        "version eyJsomeshortstring",
        "",
    ] {
        let out = redact(safe);
        assert!(out.fired.is_empty(), "{safe}");
        assert_eq!(out.text, safe);
    }
}

#[test]
fn redactable_fields_constant_is_stable() {
    assert_eq!(
        REDACTABLE_FIELDS,
        ["summary", "notes", "command", "git_message", "location"]
    );
}

#[test]
fn bearer_needs_a_non_letter_before_it_and_retries_at_the_next_position() {
    let token = "A".repeat(24);
    assert_eq!(
        redact(&format!("xBearer {token}")).text,
        format!("xBearer {token}")
    );
    assert_eq!(
        redact(&format!("(Bearer {token}")).text,
        "([REDACTED:bearer_token]"
    );
    assert_eq!(
        redact(&format!("bearer\u{1c}{token}")).fired,
        [SecretKind::BearerToken]
    );
    assert_eq!(
        redact(&format!("1Bearer {token} and Bearer {token}")).fired,
        [SecretKind::BearerToken, SecretKind::BearerToken]
    );
}

#[test]
fn patterns_run_in_order_over_the_previous_output() {
    let stripe = format!("sk_live_{}", "a".repeat(30));
    let input = format!("{stripe} and sk-{}", "b".repeat(45));
    let out = redact(&input);
    assert_eq!(out.fired, [SecretKind::OpenaiKey, SecretKind::StripeKey]);
    assert_eq!(out.text, "[REDACTED:stripe_key] and [REDACTED:openai_key]");
}

#[test]
fn the_notice_counts_every_match_and_names_each_kind_once() {
    assert_eq!(redaction_notice(&[]), None);
    assert_eq!(
        redaction_notice(&[
            SecretKind::OpenaiKey,
            SecretKind::Jwt,
            SecretKind::OpenaiKey
        ])
        .as_deref(),
        Some("dejafix: redacted 3 secret(s) before write (jwt, openai_key)")
    );
}

proptest::proptest! {
    #[test]
    fn redaction_never_panics_and_is_idempotent(text in "\\PC{0,200}") {
        let once = redact(&text).text.into_owned();
        let twice = redact(&once);
        proptest::prop_assert!(twice.fired.is_empty());
        proptest::prop_assert_eq!(twice.text.as_ref(), once.as_str());
    }
}
