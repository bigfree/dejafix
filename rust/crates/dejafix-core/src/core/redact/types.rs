use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SecretKind {
    OpenaiKey,
    GithubPat,
    GithubToken,
    AwsAccessKey,
    GoogleApiKey,
    SlackToken,
    StripeKey,
    Jwt,
    BearerToken,
    PrivateKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Redaction<'t> {
    pub text: Cow<'t, str>,
    pub fired: Vec<SecretKind>,
}
