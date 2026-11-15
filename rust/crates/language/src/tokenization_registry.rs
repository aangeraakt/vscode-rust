//! Registry of tokenization supports per language, with lazily created supports and change
//! events.
//!
//! Ported from `src/vs/editor/common/tokenizationRegistry.ts` (`TokenizationRegistry`,
//! `TokenizationSupportFactoryData`) and the `ITokenizationRegistry` interface in
//! `src/vs/editor/common/languages.ts`.
//!
//! The registry is generic over the support type `T`. The tokenizer that implements `T` lives in
//! another crate; this one only stores it.
//!
//! Behavior kept from the TypeScript:
//! - registering a support for a language replaces the previous one; unregistering only removes
//!   the support that is still current;
//! - a factory is resolved at most once; disposing a factory (or replacing it) unregisters the
//!   support it created;
//! - every registration change fires [`TokenizationRegistry::on_did_change`].
//!
//! Divergences from the TypeScript:
//! - The color map (`setColorMap` and friends) is not ported: the color type is not in the
//!   workspace yet.
//! - Factories are synchronous. `getOrCreate` returns a promise upstream because factories
//!   load code asynchronously. Here the caller runs the factory and may do so on another
//!   thread; a concurrent caller sees the state at that moment and does not wait.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use vscode_api::{Disposable, Emitter, Event, LanguageId};

/// Payload of [`TokenizationRegistry::on_did_change`]: the languages whose support changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenizationSupportChanged {
    /// Languages whose support was registered, replaced or removed.
    pub changed_languages: Vec<LanguageId>,
}

type CreateSupport<T> = Box<dyn FnOnce() -> Option<T> + Send>;

/// A lazily created support. `create` is taken on first resolution, so a factory runs at most
/// once. `registration` holds the support it produced, which disposing the factory undoes.
struct Factory<T> {
    create: Mutex<Option<CreateSupport<T>>>,
    registration: Mutex<Option<Disposable>>,
}

impl<T> Factory<T> {
    fn new(create: CreateSupport<T>) -> Self {
        Self {
            create: Mutex::new(Some(create)),
            registration: Mutex::new(None),
        }
    }

    fn is_resolved(&self) -> bool {
        lock(&self.create).is_none()
    }

    /// Undoes the support this factory produced, if any (`TokenizationSupportFactoryData.dispose`).
    fn dispose(&self) {
        // Take the registration out first: dropping it locks the registry state, which the caller
        // must not hold when it calls this.
        let registration = lock(&self.registration).take();
        drop(registration);
    }
}

struct State<T> {
    supports: HashMap<LanguageId, Arc<T>>,
    factories: HashMap<LanguageId, Arc<Factory<T>>>,
}

struct Shared<T> {
    state: Mutex<State<T>>,
    on_did_change: Emitter<TokenizationSupportChanged>,
}

impl<T> Shared<T> {
    fn fire_changed(&self, languages: Vec<LanguageId>) {
        self.on_did_change.fire(TokenizationSupportChanged {
            changed_languages: languages,
        });
    }
}

/// Holds the tokenization support of each language.
pub struct TokenizationRegistry<T> {
    shared: Arc<Shared<T>>,
}

impl<T: Send + Sync + 'static> Default for TokenizationRegistry<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Send + Sync + 'static> TokenizationRegistry<T> {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::new(State {
                    supports: HashMap::new(),
                    factories: HashMap::new(),
                }),
                on_did_change: Emitter::new(),
            }),
        }
    }

    /// Fires when a support is registered, replaced or removed (`onDidChange`).
    pub fn on_did_change(&self) -> Event<TokenizationSupportChanged> {
        self.shared.on_did_change.event()
    }

    /// Announces a change for `languages` without changing any support. Use this when a
    /// language's tokenization depends on an embedded language (`handleChange`).
    pub fn handle_change(&self, languages: Vec<LanguageId>) {
        self.shared.fire_changed(languages);
    }

    /// Registers `support` for `language`, replacing any previous one. The returned disposable
    /// removes it again, if it is still the current support.
    pub fn register(&self, language: impl Into<LanguageId>, support: T) -> Disposable {
        let language = language.into();
        let support = Arc::new(support);
        lock(&self.shared.state)
            .supports
            .insert(language.clone(), support.clone());
        self.shared.fire_changed(vec![language.clone()]);
        self.support_registration(language, support)
    }

    /// The support registered for `language`, if any (`get`).
    pub fn get(&self, language: &str) -> Option<Arc<T>> {
        lock(&self.shared.state).supports.get(language).cloned()
    }

    /// Registers a factory for `language`. The factory runs on the first
    /// [`get_or_create`](Self::get_or_create) for that language. Replacing a factory disposes the
    /// previous one, which removes the support it created. The returned disposable does the same
    /// for this factory.
    pub fn register_factory(
        &self,
        language: impl Into<LanguageId>,
        create: impl FnOnce() -> Option<T> + Send + 'static,
    ) -> Disposable {
        let language = language.into();
        let factory = Arc::new(Factory::new(Box::new(create)));
        let previous = lock(&self.shared.state)
            .factories
            .insert(language.clone(), factory.clone());
        if let Some(previous) = previous {
            previous.dispose();
        }

        let shared = Arc::downgrade(&self.shared);
        Disposable::new(move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let removed = {
                let mut state = lock(&shared.state);
                if state
                    .factories
                    .get(&language)
                    .is_some_and(|f| Arc::ptr_eq(f, &factory))
                {
                    state.factories.remove(&language)
                } else {
                    None
                }
            };
            if let Some(removed) = removed {
                removed.dispose();
            }
        })
    }

    /// The support for `language`, creating it from its factory if needed (`getOrCreate`).
    /// Returns `None` when there is no support and no factory, or the factory produced none.
    pub fn get_or_create(&self, language: &str) -> Option<Arc<T>> {
        if let Some(support) = self.get(language) {
            return Some(support);
        }

        let factory = lock(&self.shared.state).factories.get(language).cloned()?;
        // Taking the factory out makes sure it runs once. It runs with no lock held, so it may use
        // the registry itself.
        let create = lock(&factory.create).take();
        if let Some(value) = create.and_then(|create| create()) {
            self.install_from_factory(&LanguageId::from(language), &factory, value);
        }
        self.get(language)
    }

    /// Whether `language` needs no further work: it has a support, or has no factory, or its
    /// factory already ran (`isResolved`).
    pub fn is_resolved(&self, language: &str) -> bool {
        if self.get(language).is_some() {
            return true;
        }
        let factory = lock(&self.shared.state).factories.get(language).cloned();
        factory.is_none_or(|f| f.is_resolved())
    }

    /// Stores a support created by `factory`, unless the factory was replaced or disposed while
    /// it ran.
    fn install_from_factory(&self, language: &LanguageId, factory: &Arc<Factory<T>>, value: T) {
        let support = Arc::new(value);
        {
            let mut state = lock(&self.shared.state);
            let current = state
                .factories
                .get(language)
                .is_some_and(|f| Arc::ptr_eq(f, factory));
            if !current {
                return;
            }
            state.supports.insert(language.clone(), support.clone());
            // Lock order: registry state, then the factory's registration slot. `dispose` takes
            // them one at a time, so this cannot deadlock.
            *lock(&factory.registration) =
                Some(self.support_registration(language.clone(), support));
        }
        self.shared.fire_changed(vec![language.clone()]);
    }

    /// A disposable that removes `support` for `language` if it is still the current support.
    fn support_registration(&self, language: LanguageId, support: Arc<T>) -> Disposable {
        let shared = Arc::downgrade(&self.shared);
        Disposable::new(move || {
            let Some(shared) = shared.upgrade() else {
                return;
            };
            let removed = {
                let mut state = lock(&shared.state);
                let current = state
                    .supports
                    .get(&language)
                    .is_some_and(|s| Arc::ptr_eq(s, &support));
                current && state.supports.remove(&language).is_some()
            };
            if removed {
                shared.fire_changed(vec![language]);
            }
        })
    }
}

/// Locks, ignoring poisoning: no registry code panics while holding a lock, so the data is
/// consistent.
fn lock<U>(mutex: &Mutex<U>) -> MutexGuard<'_, U> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Changed language ids of each delivered event, in delivery order.
    type Log = Arc<Mutex<Vec<Vec<String>>>>;

    fn recorder() -> (Log, impl Fn(&TokenizationSupportChanged)) {
        let log = Arc::new(Mutex::new(Vec::new()));
        let sink = log.clone();
        let listener = move |event: &TokenizationSupportChanged| {
            sink.lock().unwrap().push(
                event
                    .changed_languages
                    .iter()
                    .map(|l| l.as_str().to_owned())
                    .collect(),
            );
        };
        (log, listener)
    }

    #[test]
    fn register_and_get() {
        let registry = TokenizationRegistry::<u32>::new();
        let (log, listener) = recorder();
        let _sub = registry.on_did_change().subscribe(listener);

        let mut registration = registry.register("rust", 7);
        assert_eq!(*registry.get("rust").unwrap(), 7);
        assert_eq!(*log.lock().unwrap(), [vec!["rust"]]);

        registration.dispose();
        assert!(registry.get("rust").is_none());
        assert_eq!(log.lock().unwrap().len(), 2);
    }

    #[test]
    fn newer_registration_survives_disposing_the_older_one() {
        let registry = TokenizationRegistry::<u32>::new();
        let mut older = registry.register("rust", 1);
        let _newer = registry.register("rust", 2);

        older.dispose();
        assert_eq!(*registry.get("rust").unwrap(), 2);
    }

    #[test]
    fn factory_runs_once_and_registers_its_support() {
        let registry = TokenizationRegistry::<u32>::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        let _factory = registry.register_factory("json", move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Some(42)
        });

        assert!(!registry.is_resolved("json"));
        assert_eq!(*registry.get_or_create("json").unwrap(), 42);
        assert_eq!(*registry.get_or_create("json").unwrap(), 42);
        assert!(registry.is_resolved("json"));
        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn factory_producing_nothing_resolves_to_none() {
        let registry = TokenizationRegistry::<u32>::new();
        let _factory = registry.register_factory("empty", || None);

        assert!(registry.get_or_create("empty").is_none());
        assert!(registry.is_resolved("empty"));
    }

    #[test]
    fn disposing_a_factory_before_it_runs_skips_it() {
        let registry = TokenizationRegistry::<u32>::new();
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();
        let mut factory = registry.register_factory("md", move || {
            counter.fetch_add(1, Ordering::SeqCst);
            Some(1)
        });

        factory.dispose();
        assert!(registry.get_or_create("md").is_none());
        assert!(registry.is_resolved("md"));
        assert_eq!(runs.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn disposing_a_resolved_factory_unregisters_its_support() {
        let registry = TokenizationRegistry::<u32>::new();
        let (log, listener) = recorder();
        let _sub = registry.on_did_change().subscribe(listener);

        let mut factory = registry.register_factory("ts", || Some(5));
        assert_eq!(*registry.get_or_create("ts").unwrap(), 5);

        factory.dispose();
        assert!(registry.get("ts").is_none());
        // Registration of the support, then its removal with the factory.
        assert_eq!(*log.lock().unwrap(), [vec!["ts"], vec!["ts"]]);
    }

    #[test]
    fn replacing_a_factory_disposes_the_previous_one() {
        let registry = TokenizationRegistry::<u32>::new();
        let _first = registry.register_factory("py", || Some(1));
        assert_eq!(*registry.get_or_create("py").unwrap(), 1);

        let _second = registry.register_factory("py", || Some(2));
        assert!(registry.get("py").is_none());
        assert_eq!(*registry.get_or_create("py").unwrap(), 2);
    }

    #[test]
    fn unknown_language_is_resolved_and_has_no_support() {
        let registry = TokenizationRegistry::<u32>::new();
        assert!(registry.get_or_create("nothing").is_none());
        assert!(registry.is_resolved("nothing"));
    }

    #[test]
    fn handle_change_fires_without_touching_supports() {
        let registry = TokenizationRegistry::<u32>::new();
        let _support = registry.register("html", 3);
        let (log, listener) = recorder();
        let _sub = registry.on_did_change().subscribe(listener);

        registry.handle_change(vec![LanguageId::from("html")]);
        assert_eq!(*registry.get("html").unwrap(), 3);
        assert_eq!(*log.lock().unwrap(), [vec!["html"]]);
    }
}
