//! Turning provider ids from configuration into providers.

use std::sync::Arc;

use crate::{FallbackChain, UsageProvider};

/// The set of providers this build knows how to construct.
///
/// Configuration names providers by id, and this is what turns those names into
/// something that can be fetched from. Keeping the lookup in one place means an
/// id that does not exist produces one clear message listing what does, rather
/// than a different failure per caller.
///
/// Registration order is preference order: [`ProviderRegistry::chain`] hands out
/// providers in the order the ids were asked for, and [`Self::ids`] reports them
/// in the order they were registered.
#[derive(Debug, Default)]
pub struct ProviderRegistry {
    providers: Vec<Arc<dyn UsageProvider>>,
}

impl ProviderRegistry {
    /// Creates an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a provider.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::DuplicateId`] if something is already
    /// registered under that id. Silently replacing it would make which
    /// provider serves depend on registration order, which is not something a
    /// user could debug from a config file.
    pub fn register(&mut self, provider: Arc<dyn UsageProvider>) -> Result<(), RegistryError> {
        if self.get(provider.id()).is_some() {
            return Err(RegistryError::DuplicateId(provider.id().to_owned()));
        }
        self.providers.push(provider);
        Ok(())
    }

    /// Adds a provider, for building a registry in one expression.
    ///
    /// # Errors
    ///
    /// As [`Self::register`].
    pub fn with(mut self, provider: Arc<dyn UsageProvider>) -> Result<Self, RegistryError> {
        self.register(provider)?;
        Ok(self)
    }

    /// Returns the provider registered under an id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Arc<dyn UsageProvider>> {
        self.providers.iter().find(|provider| provider.id() == id)
    }

    /// Returns every registered id, in registration order.
    #[must_use]
    pub fn ids(&self) -> Vec<&str> {
        self.providers
            .iter()
            .map(|provider| provider.id())
            .collect()
    }

    /// Whether anything is registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// How many providers are registered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Resolves a list of ids to providers, in the order given.
    ///
    /// # Errors
    ///
    /// Returns [`RegistryError::UnknownProvider`] naming the first id that does
    /// not resolve, along with the ones that would have.
    pub fn resolve<I, S>(&self, ids: I) -> Result<Vec<Arc<dyn UsageProvider>>, RegistryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        ids.into_iter()
            .map(|id| {
                let id = id.as_ref();
                self.get(id)
                    .cloned()
                    .ok_or_else(|| RegistryError::UnknownProvider {
                        id: id.to_owned(),
                        available: self.ids().into_iter().map(str::to_owned).collect(),
                    })
            })
            .collect()
    }

    /// Resolves a list of ids into a chain that tries them in order.
    ///
    /// # Errors
    ///
    /// As [`Self::resolve`], plus [`RegistryError::NoneSelected`] for an empty
    /// list — a chain with nothing in it can only ever fail, and it is better to
    /// say so when the configuration is read than on every refresh.
    pub fn chain<I, S>(&self, ids: I) -> Result<FallbackChain, RegistryError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let providers = self.resolve(ids)?;
        if providers.is_empty() {
            return Err(RegistryError::NoneSelected);
        }
        Ok(FallbackChain::new(providers))
    }
}

/// Why a set of provider ids could not be turned into providers.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// Two providers claimed the same id.
    #[error("a provider with id `{0}` is already registered")]
    DuplicateId(String),

    /// An id in the configuration matches nothing this build knows about.
    #[error("unknown provider `{id}` (available: {})", .available.join(", "))]
    UnknownProvider {
        /// The id that did not resolve.
        id: String,
        /// The ids that would have.
        available: Vec<String>,
    },

    /// No providers were selected at all.
    #[error("no providers selected")]
    NoneSelected,
}

#[cfg(all(test, feature = "mock"))]
mod tests {
    use super::{ProviderRegistry, RegistryError};
    use crate::{MockProvider, UsageProvider};
    use claude_tray_core::Percent;
    use std::sync::Arc;

    fn mock(id: &str) -> Arc<dyn UsageProvider> {
        Arc::new(MockProvider::fixed(Percent::new(50.0).unwrap()).with_id(id.to_owned()))
    }

    fn registry(ids: &[&str]) -> ProviderRegistry {
        let mut registry = ProviderRegistry::new();
        for id in ids {
            registry.register(mock(id)).unwrap();
        }
        registry
    }

    #[test]
    fn an_empty_registry_resolves_nothing() {
        let registry = ProviderRegistry::new();

        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        assert!(registry.get("mock").is_none());
    }

    #[test]
    fn ids_come_back_in_registration_order() {
        let registry = registry(&["local", "http", "mock"]);
        assert_eq!(registry.ids(), vec!["local", "http", "mock"]);
    }

    /// Two providers under one id would make which one serves depend on
    /// registration order — not something a user could debug from a config
    /// file.
    #[test]
    fn a_duplicate_id_is_rejected() {
        let mut registry = registry(&["mock"]);

        assert_eq!(
            registry.register(mock("mock")).unwrap_err(),
            RegistryError::DuplicateId("mock".to_owned())
        );
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn resolving_keeps_the_order_asked_for() {
        let registry = registry(&["local", "http"]);
        let resolved = registry.resolve(["http", "local"]).unwrap();

        assert_eq!(
            resolved.iter().map(|p| p.id()).collect::<Vec<_>>(),
            vec!["http", "local"],
            "the configured order is the preference order, not the registered one"
        );
    }

    /// The error has to say what *would* have worked, or a typo in a config
    /// file is a guessing game.
    #[test]
    fn an_unknown_id_names_the_ones_that_exist() {
        let registry = registry(&["local", "mock"]);
        let error = registry.resolve(["claude.ai"]).unwrap_err();

        assert_eq!(
            error,
            RegistryError::UnknownProvider {
                id: "claude.ai".to_owned(),
                available: vec!["local".to_owned(), "mock".to_owned()],
            }
        );
        assert_eq!(
            error.to_string(),
            "unknown provider `claude.ai` (available: local, mock)"
        );
    }

    #[test]
    fn one_bad_id_fails_the_whole_resolution() {
        let registry = registry(&["local"]);
        assert!(registry.resolve(["local", "nope"]).is_err());
    }

    #[test]
    fn a_chain_can_be_built_straight_from_ids() {
        let registry = registry(&["local", "http"]);
        let chain = registry.chain(["local", "http"]).unwrap();

        assert_eq!(chain.len(), 2);
    }

    /// A chain with nothing in it can only ever fail. Better to say so when the
    /// configuration is read than on every refresh.
    #[test]
    fn a_chain_of_nothing_is_rejected() {
        let registry = registry(&["local"]);

        assert_eq!(
            registry.chain(Vec::<String>::new()).unwrap_err(),
            RegistryError::NoneSelected
        );
        assert!(
            registry.resolve(Vec::<String>::new()).is_ok(),
            "resolving nothing is still a valid answer to a question about nothing"
        );
    }

    #[test]
    fn it_builds_in_one_expression() {
        let registry = ProviderRegistry::new()
            .with(mock("local"))
            .unwrap()
            .with(mock("http"))
            .unwrap();

        assert_eq!(registry.ids(), vec!["local", "http"]);
    }
}
