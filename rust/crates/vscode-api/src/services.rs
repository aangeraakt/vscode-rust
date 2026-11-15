//! Service registry.
//!
//! Ported from `src/vs/platform/instantiation/common/{instantiation,serviceCollection}.ts`.
//!
//! VS Code identifies a service by a decorator id (`createDecorator<IFooService>('fooService')`)
//! and injects it into constructors. In Rust the service type itself is the key: register an
//! `Arc<dyn FooService>` and look it up by `dyn FooService`. Construction stays explicit; the
//! `lathe` binary builds services in dependency order and registers them (ADR 0005), so there is
//! no constructor injection, lazy instantiation, or cycle detection to port.

use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A thread-safe map from service type to its single instance (`ServiceCollection`).
#[derive(Default)]
pub struct ServiceRegistry {
    services: RwLock<HashMap<TypeId, Box<dyn Any + Send + Sync>>>,
}

impl ServiceRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `service` under the type `S` (usually a `dyn Trait`) and returns the instance it
    /// replaced, if any. Replacing matches `ServiceCollection.set` and is how tests install stubs.
    pub fn register<S>(&self, service: Arc<S>) -> Option<Arc<S>>
    where
        S: ?Sized + Send + Sync + 'static,
    {
        let previous = self
            .services
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(TypeId::of::<Arc<S>>(), Box::new(service));
        previous.and_then(|b| b.downcast::<Arc<S>>().ok().map(|b| *b))
    }

    /// Returns the instance registered for `S`, if any.
    pub fn get<S>(&self) -> Option<Arc<S>>
    where
        S: ?Sized + Send + Sync + 'static,
    {
        self.services
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(&TypeId::of::<Arc<S>>())
            .and_then(|b| b.downcast_ref::<Arc<S>>())
            .cloned()
    }

    /// Returns the instance registered for `S`.
    ///
    /// # Panics
    /// If no instance is registered; that is a wiring bug in the `lathe` binary, like VS Code's
    /// "unknown service" error.
    pub fn require<S>(&self) -> Arc<S>
    where
        S: ?Sized + Send + Sync + 'static,
    {
        self.get::<S>()
            .unwrap_or_else(|| panic!("unknown service: {}", type_name::<S>()))
    }

    /// Whether an instance is registered for `S`.
    pub fn has<S>(&self) -> bool
    where
        S: ?Sized + Send + Sync + 'static,
    {
        self.services
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&TypeId::of::<Arc<S>>())
    }
}

impl std::fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.services.read().map(|m| m.len()).unwrap_or(0);
        f.debug_struct("ServiceRegistry")
            .field("services", &len)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Greeter: Send + Sync {
        fn greet(&self) -> String;
    }
    struct En;
    impl Greeter for En {
        fn greet(&self) -> String {
            "hello".into()
        }
    }
    struct Nl;
    impl Greeter for Nl {
        fn greet(&self) -> String {
            "hallo".into()
        }
    }

    #[test]
    fn registers_and_resolves_trait_objects() {
        let registry = ServiceRegistry::new();
        assert!(!registry.has::<dyn Greeter>());
        assert!(registry.register::<dyn Greeter>(Arc::new(En)).is_none());
        assert_eq!(registry.require::<dyn Greeter>().greet(), "hello");
    }

    #[test]
    fn replacing_returns_previous_instance() {
        let registry = ServiceRegistry::new();
        let _ = registry.register::<dyn Greeter>(Arc::new(En));
        let previous = registry.register::<dyn Greeter>(Arc::new(Nl)).unwrap();
        assert_eq!(previous.greet(), "hello");
        assert_eq!(registry.require::<dyn Greeter>().greet(), "hallo");
    }

    #[test]
    fn concrete_and_trait_keys_are_distinct() {
        let registry = ServiceRegistry::new();
        let _ = registry.register(Arc::new(En));
        assert!(registry.get::<En>().is_some());
        assert!(registry.get::<dyn Greeter>().is_none());
    }

    #[test]
    #[should_panic(expected = "unknown service")]
    fn require_panics_for_missing_service() {
        ServiceRegistry::new().require::<dyn Greeter>();
    }
}
