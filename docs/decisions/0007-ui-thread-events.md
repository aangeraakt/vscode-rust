# ADR 0007: UI-thread events and the fire-after-apply rule

Status: accepted (Phase 1)

## Context

ADR 0005 puts text models and view models on the UI thread, where they hold `Rc`/`RefCell` state
and are not `Send`. `vscode-api::event::Emitter` requires `Send + Sync` listeners, because services
and the I/O runtime share it. A view model therefore could not subscribe with a closure that
captures its state, and it had no way to get a single-threaded emitter.

Two requirements came out of the Phase 0 review (COORDINATION.md, role 04/05 request):

1. UI-thread listeners must be able to capture `Rc` state.
2. A UI-thread listener of a content change must be able to read the model from inside the
   callback. In VS Code, `textModel.ts` `_emitContentChangedEvent` fires `onDidChangeContent` only
   after the buffer edit is complete, so listeners can read the model. This holds for
   `LocalEmitter` owners (see Decision), not for the `Send + Sync` `TextModel::on_did_change_content`.

## Decision

- **`Emitter` / `Event`** (unchanged): `Send + Sync` listeners, for services, the file system,
  commands and anything that crosses to the I/O runtime.
- **`LocalEmitter` / `LocalEvent`** (new, `vscode-api::event`): the same delivery semantics as
  `Emitter`, with `Rc<RefCell<_>>` state and `'static` listeners that need not be `Send`. It is
  used by UI-thread view models and cursors (roles 04 and 05). Its tests mirror `Emitter`'s.
- **`LocalDisposable`** (new, `vscode-api::lifecycle`): the unsubscribe handle of a
  `LocalEvent`. `Disposable` holds a `Send` closure, and a `Weak<RefCell<_>>` is not `Send`, so a
  `Disposable` cannot unsubscribe a local listener. `LocalDisposable` has the same semantics and
  cannot be converted into a `Disposable`.
- **Fire-after-apply rule** (documented on `TextModel::on_did_change_content`): every mutating
  method (`apply_edits`, `set_value`, `set_end_of_line_sequence`, and undo or redo where an
  implementation provides them) fires a content change only after the edit is fully applied, and
  never while one of its own interior borrows or locks is held. An owner that fires a
  `LocalEmitter` after a mutation releases any `borrow_mut` of the model first. Its UI-thread
  listeners then read the model through a shared handle. A test in `event.rs` shows this pattern,
  and another shows that firing inside a `borrow_mut` fails. A `TextModel` listener from
  `on_did_change_content` (`Send + Sync`) cannot read the model; it gets only the event value.
- **`TextModel` keeps `Event`.** The trait is part of the frozen contract and is consumed by the
  extension-host bridge, which needs `Send` listeners. UI-thread consumers that need model changes
  are notified by the owner that calls `apply_edits` (role 04 wraps the model), which fires its own
  `LocalEmitter` afterwards. Edits reach UI-thread listeners only through that owner, which fits
  ADR 0005: models are mutated only on the UI thread.

## Consequences

- Two emitter types with the same semantics. Keeping them in step is a review item, and the
  tests in `event.rs` mirror each other.
- `LocalDisposable` duplicates about 50 lines of `Disposable`. Making `Disposable` generic over
  its closure would change a frozen public type, so the duplication is the lower cost.
- The owner wrapper (roles 03/04) is the single mutation entry point for UI-thread text models.
  A mutation that bypasses it fires only the `Send` event, so local listeners miss it.
- `LocalEvent` listeners that capture the object owning the event must capture a `Weak`, or the
  `Rc` cycle leaks (documented on `LocalEmitter`).
- No owner-level UI-thread content event exists yet. Role 05 must not assume one; the request is
  open in COORDINATION.md.
- A `TextModel` implementation that wants to push changes straight to local listeners would need
  an additive trait method (for example `on_did_change_content_local`). No implementation exists
  yet, so this can be decided when role 03 starts `text-buffer`; it is not needed for cursor work.
