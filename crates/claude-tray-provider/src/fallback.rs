//! Trying several providers in turn.

use core::time::Duration;
use std::sync::Arc;

use async_trait::async_trait;
use claude_tray_core::UsageSnapshot;

use crate::{Health, ProviderCapabilities, ProviderError, UsageProvider};

/// Several providers tried in order, first success wins.
///
/// A chain is itself a [`UsageProvider`], so the scheduler holds one provider
/// whether the user configured one source or four, and nothing downstream has to
/// know which it is.
///
/// The snapshot is passed through untouched, so
/// [`UsageSnapshot::source`](claude_tray_core::UsageSnapshot::source) still
/// names the provider that actually produced it rather than the chain. Which
/// source a number came from is the sort of thing a user needs to be able to
/// see when two of them disagree.
///
/// ```
/// # use claude_tray_provider::{Fault, FallbackChain, MockProvider, UsageProvider};
/// # use claude_tray_core::Percent;
/// # use std::sync::Arc;
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let chain = FallbackChain::new([
///     Arc::new(MockProvider::failing(Fault::Timeout).with_id("primary")) as Arc<dyn UsageProvider>,
///     Arc::new(MockProvider::fixed(Percent::new(42.0)?).with_id("secondary")),
/// ]);
///
/// let snapshot = chain.fetch().await?;
/// assert_eq!(snapshot.source.as_str(), "secondary");
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct FallbackChain {
    id: String,
    providers: Vec<Arc<dyn UsageProvider>>,
}

impl FallbackChain {
    /// The id a chain reports unless renamed.
    pub const DEFAULT_ID: &'static str = "fallback";

    /// Creates a chain that tries the given providers in order.
    #[must_use]
    pub fn new(providers: impl IntoIterator<Item = Arc<dyn UsageProvider>>) -> Self {
        Self {
            id: Self::DEFAULT_ID.to_owned(),
            providers: providers.into_iter().collect(),
        }
    }

    /// Renames the chain, for a build that runs more than one of them.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    /// Returns the providers, in the order they are tried.
    #[must_use]
    pub fn providers(&self) -> &[Arc<dyn UsageProvider>] {
        &self.providers
    }

    /// How many providers the chain will try.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Whether the chain has nothing to try.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

#[async_trait]
impl UsageProvider for FallbackChain {
    fn id(&self) -> &str {
        &self.id
    }

    /// Reports the union of what the members can supply.
    ///
    /// Optimistic on purpose. These capabilities decide which rows the tooltip
    /// shows at all, and a row is also empty-checked against the snapshot
    /// itself — so an overstated capability costs at most a blank row, while an
    /// understated one would hide data the serving provider did have.
    fn capabilities(&self) -> ProviderCapabilities {
        let mut combined =
            self.providers
                .iter()
                .fold(ProviderCapabilities::none(), |mut combined, provider| {
                    let capabilities = provider.capabilities();
                    combined.windows.extend(capabilities.windows);
                    combined.limits |= capabilities.limits;
                    combined.reset_times |= capabilities.reset_times;
                    combined.plan |= capabilities.plan;
                    combined.model |= capabilities.model;
                    combined
                });

        combined.windows.sort_unstable();
        combined.windows.dedup();
        combined
    }

    async fn fetch(&self) -> Result<UsageSnapshot, ProviderError> {
        let mut failures = Vec::new();

        for provider in &self.providers {
            match provider.fetch().await {
                Ok(snapshot) => return Ok(snapshot),
                Err(error) => failures.push((provider.id().to_owned(), error)),
            }
        }

        Err(combine(&failures))
    }

    /// Reports whether anything in the chain can serve, and whether the
    /// preferred one is what will.
    async fn health(&self) -> Health {
        let Some(first) = self.providers.first() else {
            return Health::Unavailable {
                reason: "no providers configured".to_owned(),
            };
        };

        let preferred = first.health().await;
        if preferred.is_usable() {
            return preferred;
        }

        let preferred_reason = preferred.reason().unwrap_or("unavailable").to_owned();
        let mut reasons = vec![format!("{}: {preferred_reason}", first.id())];

        for provider in self.providers.iter().skip(1) {
            let health = provider.health().await;
            if health.is_usable() {
                // Worth saying out loud in the menu: the numbers are coming
                // from somewhere other than the source the user chose first.
                return Health::Degraded {
                    reason: format!(
                        "{} is unavailable ({preferred_reason}); falling back to {}",
                        first.id(),
                        provider.id()
                    ),
                };
            }
            reasons.push(format!(
                "{}: {}",
                provider.id(),
                health.reason().unwrap_or("unavailable")
            ));
        }

        Health::Unavailable {
            reason: reasons.join("; "),
        }
    }
}

/// Folds every provider's failure into the one the tray will show.
///
/// The kind comes from the first provider, because that is the one the user put
/// first and the one whose problem they would most want to fix. The retry
/// policy, in contrast, is the chain's: if *any* member could succeed on a
/// retry, the chain could.
fn combine(failures: &[(String, ProviderError)]) -> ProviderError {
    let Some((_, first)) = failures.first() else {
        return ProviderError::internal("the fallback chain has no providers to try");
    };

    let detail = failures
        .iter()
        .map(|(id, error)| format!("{id}: {error}"))
        .collect::<Vec<_>>()
        .join("; ");

    let retryable: Vec<&ProviderError> = failures
        .iter()
        .map(|(_, error)| error)
        .filter(|error| error.is_retryable())
        .collect();

    // A delay only binds the chain when every retryable member asked for one:
    // if any of them is ready now, the chain is.
    let retry_after: Option<Duration> = if retryable.iter().all(|e| e.retry_after().is_some()) {
        retryable.iter().filter_map(|e| e.retry_after()).min()
    } else {
        None
    };

    let error = ProviderError::new(
        first.kind(),
        format!("no provider could serve usage — {detail}"),
    )
    .with_retryable(!retryable.is_empty());

    match retry_after {
        Some(delay) => error.with_retry_after(delay),
        None => error,
    }
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    use super::FallbackChain;
    use crate::{Fault, Health, MockProvider, ProviderCapabilities, Step, UsageProvider};
    use claude_tray_core::{ErrorKind, Percent, WindowKind};
    use core::time::Duration;
    use std::sync::Arc;

    fn percent(value: f64) -> Percent {
        Percent::new(value).unwrap()
    }

    fn working(id: &str, remaining: f64) -> Arc<dyn UsageProvider> {
        Arc::new(MockProvider::fixed(percent(remaining)).with_id(id.to_owned()))
    }

    fn broken(id: &str, fault: Fault) -> Arc<dyn UsageProvider> {
        Arc::new(MockProvider::failing(fault).with_id(id.to_owned()))
    }

    #[tokio::test]
    async fn the_first_provider_that_works_wins() {
        let chain = FallbackChain::new([
            broken("primary", Fault::Timeout),
            working("secondary", 42.0),
            working("tertiary", 10.0),
        ]);

        let snapshot = chain.fetch().await.unwrap();

        assert_eq!(snapshot.remaining_percent(), Some(percent(42.0)));
        assert_eq!(
            snapshot.source.as_str(),
            "secondary",
            "the snapshot names the provider that made it, not the chain"
        );
    }

    /// A working first provider means the rest are never asked — which for a
    /// real chain is the difference between one network call and three.
    #[tokio::test]
    async fn later_providers_are_not_consulted_after_a_success() {
        let stepper = Arc::new(
            MockProvider::scripted([
                Step::Remaining(percent(80.0)),
                Step::Remaining(percent(1.0)),
            ])
            .with_id("second"),
        );
        let chain = FallbackChain::new([working("first", 50.0), stepper.clone()]);

        chain.fetch().await.unwrap();
        chain.fetch().await.unwrap();

        assert_eq!(
            stepper.fetch().await.unwrap().remaining_percent(),
            Some(percent(80.0)),
            "the second provider's script never advanced"
        );
    }

    #[tokio::test]
    async fn a_chain_of_one_behaves_like_the_provider_itself() {
        let chain = FallbackChain::new([working("only", 33.0)]);

        assert_eq!(
            chain.fetch().await.unwrap().remaining_percent(),
            Some(percent(33.0))
        );
        assert_eq!(chain.len(), 1);
        assert!(!chain.is_empty());
    }

    /// The user's first choice is the one whose problem they would most want to
    /// fix, so its kind is what the tray shows.
    #[tokio::test]
    async fn total_failure_reports_the_preferred_provider_s_kind() {
        let chain = FallbackChain::new([
            broken("primary", Fault::Unauthorized),
            broken("secondary", Fault::Timeout),
        ]);

        let error = chain.fetch().await.unwrap_err();

        assert_eq!(error.kind(), ErrorKind::NotAuthenticated);
        assert!(
            error.message().contains("primary") && error.message().contains("secondary"),
            "the log needs every failure, not just the first: {}",
            error.message()
        );
    }

    /// If any member could succeed on a retry, the chain could.
    #[tokio::test]
    async fn the_chain_is_retryable_when_any_member_is() {
        let chain = FallbackChain::new([
            broken("primary", Fault::Unauthorized),
            broken("secondary", Fault::Timeout),
        ]);

        let error = chain.fetch().await.unwrap_err();

        assert_eq!(error.kind(), ErrorKind::NotAuthenticated);
        assert!(
            error.is_retryable(),
            "the secondary is worth retrying even though the primary is not"
        );
    }

    #[tokio::test]
    async fn a_chain_of_hopeless_failures_is_not_retryable() {
        let chain = FallbackChain::new([
            broken("primary", Fault::Unauthorized),
            broken("secondary", Fault::Malformed),
        ]);

        assert!(!chain.fetch().await.unwrap_err().is_retryable());
    }

    /// Waiting means waiting for the first member that will be ready, not for
    /// the slowest.
    #[tokio::test]
    async fn a_shared_rate_limit_waits_for_the_earliest_member() {
        let chain = FallbackChain::new([
            broken(
                "primary",
                Fault::RateLimited {
                    retry_after: Some(Duration::from_mins(10)),
                },
            ),
            broken(
                "secondary",
                Fault::RateLimited {
                    retry_after: Some(Duration::from_mins(2)),
                },
            ),
        ]);

        assert_eq!(
            chain.fetch().await.unwrap_err().retry_after(),
            Some(Duration::from_mins(2))
        );
    }

    /// A member that is retryable with no stated delay means the chain has
    /// nothing to wait for — its own backoff applies.
    #[tokio::test]
    async fn one_member_ready_now_means_no_mandated_wait() {
        let chain = FallbackChain::new([
            broken(
                "primary",
                Fault::RateLimited {
                    retry_after: Some(Duration::from_mins(10)),
                },
            ),
            broken("secondary", Fault::Timeout),
        ]);

        let error = chain.fetch().await.unwrap_err();

        assert!(error.is_retryable());
        assert_eq!(error.retry_after(), None);
    }

    #[tokio::test]
    async fn an_empty_chain_fails_rather_than_panicking() {
        let chain = FallbackChain::new([]);

        assert!(chain.is_empty());
        assert_eq!(chain.fetch().await.unwrap_err().kind(), ErrorKind::Internal);
        assert!(!chain.health().await.is_usable());
    }

    #[tokio::test]
    async fn health_is_ready_when_the_preferred_provider_is() {
        let chain = FallbackChain::new([
            working("primary", 50.0),
            broken("secondary", Fault::Timeout),
        ]);

        assert_eq!(chain.health().await, Health::Ready);
    }

    /// Running on a fallback is worth saying out loud: the numbers are coming
    /// from somewhere other than the source the user chose first.
    #[tokio::test]
    async fn health_says_so_when_running_on_a_fallback() {
        let chain = FallbackChain::new([
            broken("primary", Fault::Unauthorized),
            working("secondary", 50.0),
        ]);

        let health = chain.health().await;

        assert!(health.is_usable());
        assert_eq!(
            health,
            Health::Degraded {
                reason: "primary is unavailable (not signed in); falling back to secondary"
                    .to_owned()
            }
        );
    }

    #[tokio::test]
    async fn health_collects_every_reason_when_nothing_can_serve() {
        let chain = FallbackChain::new([
            broken("primary", Fault::Unauthorized),
            broken("secondary", Fault::Unauthorized),
        ]);

        let health = chain.health().await;

        assert!(!health.is_usable());
        assert_eq!(
            health.reason(),
            Some("primary: not signed in; secondary: not signed in")
        );
    }

    /// Understating capabilities would hide data the serving provider did have,
    /// which is worse than the blank row an overstatement costs.
    #[tokio::test]
    async fn capabilities_are_the_union_of_the_members() {
        let modelless = Arc::new(
            MockProvider::fixed(percent(50.0))
                .with_id("modelless")
                .with_model(None),
        );
        let chain =
            FallbackChain::new([modelless as Arc<dyn UsageProvider>, working("full", 50.0)]);

        assert_eq!(
            chain.capabilities(),
            ProviderCapabilities {
                windows: vec![WindowKind::Session],
                limits: true,
                reset_times: true,
                plan: true,
                model: true,
            }
        );
    }

    #[tokio::test]
    async fn an_empty_chain_claims_nothing() {
        assert_eq!(
            FallbackChain::new([]).capabilities(),
            ProviderCapabilities::none()
        );
    }

    #[tokio::test]
    async fn the_chain_reports_its_own_id() {
        assert_eq!(FallbackChain::new([]).id(), "fallback");
        assert_eq!(
            FallbackChain::new([]).with_id("primary-chain").id(),
            "primary-chain"
        );
    }

    /// The whole point: everything downstream holds one provider, however many
    /// sources are behind it.
    #[tokio::test]
    async fn a_chain_is_itself_a_provider() {
        let inner = FallbackChain::new([broken("a", Fault::Timeout), working("b", 20.0)]);
        let outer: Box<dyn UsageProvider> = Box::new(inner);

        assert_eq!(
            outer.fetch().await.unwrap().remaining_percent(),
            Some(percent(20.0))
        );
    }
}
