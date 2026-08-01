//! What a provider says when it cannot produce a reading.

use core::time::Duration;

use claude_tray_core::ErrorKind;

/// Why a fetch failed, and what the caller should do about it.
///
/// Carries a [`ErrorKind`] so the tray can render the failure without knowing
/// which provider produced it, and a retry policy so the scheduler can decide
/// what to do without a table of special cases per provider.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct ProviderError {
    kind: ErrorKind,
    message: String,
    retryable: bool,
    retry_after: Option<Duration>,
}

impl ProviderError {
    /// Creates an error, taking its retry policy from the kind.
    #[must_use]
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retryable: kind.is_retryable(),
            retry_after: None,
        }
    }

    /// There are no usable credentials.
    #[must_use]
    pub fn not_authenticated(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotAuthenticated, message)
    }

    /// The source could not be reached, including because a request timed out.
    #[must_use]
    pub fn network_unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NetworkUnavailable, message)
    }

    /// The source asked us to slow down.
    ///
    /// Unlike [`ErrorKind::is_retryable`], which answers "may this be retried
    /// on the *normal schedule*" and so says no, a rate limit is retryable —
    /// just not yet. The distinction matters to the scheduler, which needs to
    /// come back later rather than give up.
    #[must_use]
    pub fn rate_limited(message: impl Into<String>, retry_after: Option<Duration>) -> Self {
        Self {
            retryable: true,
            retry_after,
            ..Self::new(ErrorKind::RateLimited, message)
        }
    }

    /// The source is reachable but not serving usage.
    #[must_use]
    pub fn provider_unavailable(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::ProviderUnavailable, message)
    }

    /// A response arrived in a shape the provider did not recognise.
    #[must_use]
    pub fn parse_failed(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::ParseFailed, message)
    }

    /// A fault on our side.
    #[must_use]
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    /// Overrides the retry policy the kind implies.
    #[must_use]
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// Records how long the caller should wait before trying again.
    #[must_use]
    pub fn with_retry_after(mut self, retry_after: Duration) -> Self {
        self.retry_after = Some(retry_after);
        self
    }

    /// Returns the failure as the domain model describes it.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the human-readable detail, which is for logs rather than for the
    /// tooltip — the tooltip renders [`Self::kind`].
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Whether trying again could plausibly succeed.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        self.retryable
    }

    /// How long to wait before trying again, when the source said so.
    ///
    /// Absent does not mean "retry immediately" — it means the source gave no
    /// guidance and the caller's own backoff applies.
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        self.retry_after
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderError;
    use claude_tray_core::ErrorKind;
    use core::time::Duration;

    #[test]
    fn the_retry_policy_follows_the_kind_by_default() {
        assert!(ProviderError::network_unavailable("offline").is_retryable());
        assert!(!ProviderError::not_authenticated("no token").is_retryable());
        assert!(!ProviderError::parse_failed("bad shape").is_retryable());
    }

    /// A rate limit is retryable — later. The scheduler needs to come back
    /// rather than give up, which is the opposite of what the kind's own
    /// same-schedule answer would suggest.
    #[test]
    fn a_rate_limit_is_retryable_but_carries_a_delay() {
        let error = ProviderError::rate_limited("slow down", Some(Duration::from_mins(2)));

        assert_eq!(error.kind(), ErrorKind::RateLimited);
        assert!(error.is_retryable());
        assert_eq!(error.retry_after(), Some(Duration::from_mins(2)));
        assert!(
            !ErrorKind::RateLimited.is_retryable(),
            "the kind still answers the narrower question"
        );
    }

    #[test]
    fn a_source_that_gave_no_delay_leaves_the_backoff_to_the_caller() {
        assert_eq!(
            ProviderError::rate_limited("slow down", None).retry_after(),
            None
        );
        assert_eq!(
            ProviderError::provider_unavailable("503").retry_after(),
            None
        );
    }

    #[test]
    fn the_policy_can_be_overridden() {
        let error = ProviderError::parse_failed("transient garbage").with_retryable(true);
        assert!(error.is_retryable());
    }

    #[test]
    fn the_message_is_for_logs_and_the_kind_is_for_the_tooltip() {
        let error = ProviderError::not_authenticated("no credential in the keyring");

        assert_eq!(error.message(), "no credential in the keyring");
        assert_eq!(error.to_string(), "no credential in the keyring");
        assert_eq!(error.kind().to_string(), "not signed in");
    }
}
