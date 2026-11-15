//! Command registry.
//!
//! Ported from `src/vs/platform/commands/common/commands.ts` (`CommandsRegistry`,
//! `ICommandHandler`, `onDidRegisterCommand`). Tests follow
//! `src/vs/platform/commands/test/common/commands.test.ts`.
//!
//! Commands are identified by string ids and take JSON arguments, which is what keybinding
//! files, the command palette and the extension host (ADR 0004) pass. Registering an id that
//! already exists shadows the previous handler; disposing the newer registration restores it.
//!
//! Handlers are synchronous and run on the caller's thread (the UI thread, ADR 0005). A command
//! that does I/O starts it on the runtime and returns. An async variant can be added later
//! without changing this API.

use crate::event::{Emitter, Event};
use crate::ids::CommandId;
use crate::lifecycle::Disposable;
use crate::services::ServiceRegistry;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

/// What a command returns. `Value::Null` stands for `undefined`.
pub type CommandResult = Result<Value, CommandError>;

/// A command implementation: gets the services and the JSON arguments.
pub type CommandHandler = Arc<dyn Fn(&ServiceRegistry, &[Value]) -> CommandResult + Send + Sync>;

/// Why a command did not complete.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandError {
    /// No handler is registered for the id (`command '<id>' not found`).
    NotFound(CommandId),
    /// The arguments did not match what the command expects.
    InvalidArguments(String),
    /// The handler failed.
    Failed(String),
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::NotFound(id) => write!(f, "command '{id}' not found"),
            CommandError::InvalidArguments(m) => write!(f, "invalid arguments: {m}"),
            CommandError::Failed(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for CommandError {}

#[derive(Default)]
struct Inner {
    next_token: u64,
    // Per id, newest registration last.
    commands: HashMap<CommandId, Vec<(u64, CommandHandler)>>,
}

/// The process-wide command registry. Cheap to clone; clones share state.
#[derive(Clone, Default)]
pub struct CommandRegistry {
    inner: Arc<Mutex<Inner>>,
    on_did_register: Arc<Emitter<CommandId>>,
}

impl CommandRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Registers `handler` for `id`, shadowing any earlier handler for the same id. The returned
    /// [`Disposable`] removes this registration only.
    pub fn register(
        &self,
        id: impl Into<CommandId>,
        handler: impl Fn(&ServiceRegistry, &[Value]) -> CommandResult + Send + Sync + 'static,
    ) -> Disposable {
        let id = id.into();
        assert!(!id.as_str().is_empty(), "invalid command: empty id");
        let token = {
            let mut inner = self.lock();
            let token = inner.next_token;
            inner.next_token += 1;
            inner
                .commands
                .entry(id.clone())
                .or_default()
                .push((token, Arc::new(handler)));
            token
        };
        self.on_did_register.fire(id.clone());

        let weak = Arc::downgrade(&self.inner);
        Disposable::new(move || {
            let Some(inner) = weak.upgrade() else { return };
            let removed = {
                let mut inner = inner.lock().unwrap_or_else(|e| e.into_inner());
                let Some(list) = inner.commands.get_mut(&id) else {
                    return;
                };
                let removed = list
                    .iter()
                    .position(|(t, _)| *t == token)
                    .map(|i| list.remove(i));
                if list.is_empty() {
                    inner.commands.remove(&id);
                }
                removed
            };
            // The handler's captures may touch the registry when dropped; drop them unlocked.
            drop(removed);
        })
    }

    /// The active (newest) handler for `id`.
    pub fn get(&self, id: &str) -> Option<CommandHandler> {
        self.lock()
            .commands
            .get(id)
            .and_then(|list| list.last())
            .map(|(_, h)| h.clone())
    }

    /// Whether any handler is registered for `id`.
    pub fn has(&self, id: &str) -> bool {
        self.lock().commands.contains_key(id)
    }

    /// All registered command ids, sorted.
    pub fn ids(&self) -> Vec<CommandId> {
        let mut ids: Vec<_> = self.lock().commands.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Fires with the id each time a handler is registered.
    pub fn on_did_register_command(&self) -> Event<CommandId> {
        self.on_did_register.event()
    }

    /// Runs the active handler for `id`. The registry lock is not held while the handler runs, so
    /// handlers may register or execute other commands.
    pub fn execute(&self, services: &ServiceRegistry, id: &str, args: &[Value]) -> CommandResult {
        let handler = self
            .get(id)
            .ok_or_else(|| CommandError::NotFound(CommandId::from(id)))?;
        handler(services, args)
    }
}

impl fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandRegistry")
            .field("commands", &self.lock().commands.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn constant(v: Value) -> impl Fn(&ServiceRegistry, &[Value]) -> CommandResult {
        move |_, _| Ok(v.clone())
    }

    #[test]
    fn register_execute_dispose() {
        let registry = CommandRegistry::new();
        let services = ServiceRegistry::new();
        let mut d = registry.register("test", |_, args| Ok(json!(args.len())));
        assert_eq!(
            registry.execute(&services, "test", &[json!(1), json!(2)]),
            Ok(json!(2))
        );
        d.dispose();
        assert!(!registry.has("test"));
        assert_eq!(
            registry.execute(&services, "test", &[]),
            Err(CommandError::NotFound("test".into()))
        );
    }

    #[test]
    fn register_register_dispose() {
        // commands.test.ts: "register/register/dispose"
        let registry = CommandRegistry::new();
        let services = ServiceRegistry::new();
        let run = || registry.execute(&services, "foo", &[]);

        // dispose overriding command
        let mut reg1 = registry.register("foo", constant(json!(1)));
        assert_eq!(run(), Ok(json!(1)));
        let mut reg2 = registry.register("foo", constant(json!(2)));
        assert_eq!(run(), Ok(json!(2)));
        reg2.dispose();
        assert_eq!(run(), Ok(json!(1)));
        reg1.dispose();
        assert!(registry.get("foo").is_none());

        // dispose override command first
        let mut reg1 = registry.register("foo", constant(json!(1)));
        let mut reg2 = registry.register("foo", constant(json!(2)));
        assert_eq!(run(), Ok(json!(2)));
        reg1.dispose();
        assert_eq!(run(), Ok(json!(2)));
        reg2.dispose();
        assert!(registry.get("foo").is_none());
    }

    #[test]
    fn dropping_a_handler_that_uses_the_registry_does_not_deadlock() {
        struct TouchOnDrop(CommandRegistry);
        impl Drop for TouchOnDrop {
            fn drop(&mut self) {
                let _ = self.0.has("other");
            }
        }
        let registry = CommandRegistry::new();
        let guard = TouchOnDrop(registry.clone());
        let mut reg = registry.register("foo", move |_, _| {
            let _ = &guard;
            Ok(Value::Null)
        });
        reg.dispose();
        assert!(!registry.has("foo"));
    }

    #[test]
    fn fires_on_register_and_lists_ids() {
        let registry = CommandRegistry::new();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s = seen.clone();
        let _l = registry
            .on_did_register_command()
            .subscribe(move |id| s.lock().unwrap().push(id.to_string()));
        let _b = registry.register("b", constant(Value::Null));
        let _a = registry.register("a", constant(Value::Null));
        assert_eq!(*seen.lock().unwrap(), ["b", "a"]);
        assert_eq!(
            registry.ids(),
            vec![CommandId::from("a"), CommandId::from("b")]
        );
    }

    #[test]
    fn handler_can_reenter_registry_and_services() {
        let registry = CommandRegistry::new();
        let services = ServiceRegistry::new();
        let _ = services.register(Arc::new(registry.clone()));
        let _inner = registry.register("inner", constant(json!("ok")));
        let _outer = registry.register("outer", |s, _| {
            s.require::<CommandRegistry>().execute(s, "inner", &[])
        });
        assert_eq!(registry.execute(&services, "outer", &[]), Ok(json!("ok")));
    }
}
