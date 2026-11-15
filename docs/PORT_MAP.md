# Port map

Every major area of the TypeScript reference (paths relative to the repo root, pinned by
ADR 0001), the Rust crate it maps to, and its status. The port is complete when every row is
`done (tests)` or `dropped (reason)`.

Statuses: `not started`, `in progress (role NN, PR #)`, `done (tests)`, `dropped (reason)`.
Claim a row by changing it to `in progress` in a claim PR, and merge the claim before starting.
Splitting a row into finer rows is allowed in the claim PR.

Drops below are Architect decisions recorded with their reason. Anyone may propose reverting a
drop through a COORDINATION.md request.

## Foundations (`src/vs/base`)

| Area | TypeScript source | Crate | Owner | Milestone | Status |
|---|---|---|---|---|---|
| Event emitter | `src/vs/base/common/event.ts` | vscode-api | 01 | P0 | done (tests; nested-fire ordering differs, documented in `event.rs`) |
| Disposables, lifecycle | `src/vs/base/common/lifecycle.ts` | vscode-api | 01 | P0 | done (tests) |
| URI | `src/vs/base/common/uri.ts` | vscode-api | 01 | P0 | done (tests) |
| Resource and path helpers | `src/vs/base/common/{resources,path}.ts` | vscode-api | 01 | M1 | in progress (role 01, PR #12) |
| Async primitives (throttler, delayer, cancellation) | `src/vs/base/common/async.ts`, `cancellation.ts` | vscode-api | 01 | M1 | not started |
| Strings, glob, fuzzy scorer, filters | `src/vs/base/common/{strings,glob,fuzzyScorer,filters}.ts` | workspace | 08 | M2 | not started |
| JSON with comments parser and edit | `src/vs/base/common/{json,jsonEdit,jsonFormatter}.ts` | settings | 10 | M5 | not started |
| Keybinding model and key codes | `src/vs/base/common/{keybindings,keyCodes}.ts` | settings | 10 | M1 | in progress (role 10, PR #13) |
| Diff algorithm | `src/vs/base/common/diff/`, `src/vs/editor/common/diff/` | editor | 04 | M4 | not started |
| Markdown string, codicons, themables | `src/vs/base/common/{htmlContent,codicons,themables}.ts` | workbench | 07 | M3 | not started |
| UI widgets: list, tree, table | `src/vs/base/browser/ui/{list,tree,table}/` | workbench | 07 | M2 | not started |
| UI widgets: split view, grid, sash | `src/vs/base/browser/ui/{splitview,grid,sash,centered}/` | workbench | 07 | M2 | not started |
| UI widgets: scrollbar | `src/vs/base/browser/ui/scrollbar/` | editor-view | 05 | M1 | not started |
| UI widgets: inputs, buttons, toggles, menus, hover, dialog | `src/vs/base/browser/ui/{inputbox,findinput,button,toggle,menu,dropdown,hover,dialog,selectBox,actionbar,toolbar,iconLabel,highlightedlabel,keybindingLabel,progressbar,countBadge,breadcrumbs}/` | workbench | 07 | M2 | not started |
| Keyboard and mouse events | `src/vs/base/browser/{keyboardEvent,mouseEvent,touch,dnd}.ts` | platform | 02 | M1 | not started |
| Node helpers: ripgrep, processes, shell, pfs | `src/vs/base/node/` | workspace / terminal | 08 / 09 | M4 | not started |
| IPC (Electron, sockets) | `src/vs/base/parts/ipc/` | — | 01 | — | dropped (single process, ADR 0005; the extension host framing is covered by the extension host row) |
| Storage (SQLite key-value) | `src/vs/base/parts/storage/` | settings | 10 | M5 | not started |
| DOM, browser sniffing, trusted types, sanitizers | `src/vs/base/browser/{dom,browser,trustedTypes,domSanitize}.ts`, `dompurify/` | — | 01 | — | dropped (no DOM; rendering is ADR 0002) |

## Platform services (`src/vs/platform`)

| Area | TypeScript source | Crate | Owner | Milestone | Status |
|---|---|---|---|---|---|
| Instantiation / service registry | `src/vs/platform/instantiation/` | vscode-api | 01 | P0 | done (tests; explicit registration, no constructor injection, ADR 0005) |
| Commands | `src/vs/platform/commands/` | vscode-api | 01 | P0 | done (tests) |
| Configuration model | `src/vs/platform/configuration/common/{configurationModels,configuration}.ts` (model, value trees, overrides) | vscode-api | 01 | P0 | done (tests; array-merge quirk differs, documented) |
| Configuration registry and service (schemas, scopes, defaults, inspect, change events) | `src/vs/platform/configuration/`, `src/vs/workbench/services/configuration/` | settings | 10 | M5 | not started |
| Files: provider contract | `src/vs/platform/files/common/files.ts` | vscode-api | 01 | P0 | done (tests; descriptor and stream APIs deferred until a provider needs them) |
| Files: disk provider, file service, watcher | `src/vs/platform/files/{common/fileService.ts,node/}` | workspace | 08 | M1 | in progress (role 08, PR #16) |
| Context keys and `when` clauses | `src/vs/platform/contextkey/` | workbench | 07 | M2 | not started |
| Keybinding resolution and dispatch | `src/vs/platform/keybinding/`, `keyboardLayout/` | settings | 10 | M1 | not started |
| Actions and menus registry | `src/vs/platform/actions/`, `action/` | workbench | 07 | M2 | not started |
| Lifecycle phases | `src/vs/workbench/services/lifecycle/common/lifecycle.ts` (enums) | vscode-api | 01 | P0 | done (tests) |
| Lifecycle service (phase events, shutdown veto) | `src/vs/platform/lifecycle/`, `src/vs/workbench/services/lifecycle/` | lathe | 01 | M1 | not started |
| Window, native host, auxiliary windows | `src/vs/platform/{window,windows,native,auxiliaryWindow}/` | platform | 02 | M1 | in progress (role 02, PR #15) |
| Clipboard | `src/vs/platform/clipboard/` | platform | 02 | M1 | not started |
| Dialogs | `src/vs/platform/dialogs/` | platform | 02 | M1 | not started |
| Menubar (native) | `src/vs/platform/menubar/` | platform | 02 | M2 | not started |
| Secrets, encryption (keychain) | `src/vs/platform/{secrets,encryption}/` | platform | 02 | M5 | not started |
| Theme service and color registry | `src/vs/platform/theme/` | workbench | 07 | M2 | not started |
| Quick input | `src/vs/platform/quickinput/` | workbench | 07 | M2 | not started |
| Context view, hover, notification | `src/vs/platform/{contextview,hover,notification,progress}/` | workbench | 07 | M2 | not started |
| Undo/redo service | `src/vs/platform/undoRedo/` | editor | 04 | M1 | not started |
| Markers (diagnostics store) | `src/vs/platform/markers/` | language | 06 | M3 | not started |
| Log | `src/vs/platform/log/` | vscode-api | 01 | M1 | not started |
| Storage service | `src/vs/platform/storage/`, `state/` | settings | 10 | M5 | not started |
| Workspace and recent workspaces | `src/vs/platform/{workspace,workspaces,history}/` | workspace | 08 | M2 | not started |
| Environment, product, paths | `src/vs/platform/{environment,product,path}/` | lathe | 01 | M1 | not started |
| Label service | `src/vs/platform/label/`, `uriIdentity/` | workspace | 08 | M2 | not started |
| Terminal platform (PTY host) | `src/vs/platform/terminal/`, `shell/` | terminal | 09 | M4 | not started |
| External terminal | `src/vs/platform/externalTerminal/` | terminal | 09 | M4 | not started |
| Debug platform helpers | `src/vs/platform/debug/` | debug | 09 | M6 | not started |
| Extension management and gallery | `src/vs/platform/{extensionManagement,extensions,extensionResourceLoader,extensionRecommendations}/` | extensions | 10 | M5 | not started |
| User data profiles | `src/vs/platform/{userData,userDataProfile}/` | settings | 10 | M5 | not started |
| Request / download | `src/vs/platform/{request,download}/` | extensions | 10 | M5 | not started |
| Accessibility, accessibility signals | `src/vs/platform/{accessibility,accessibilitySignal}/` | workbench | 07 | M2 | not started |
| GPU helpers | `src/vs/platform/gpu/` | editor-view | 05 | M1 | not started |
| Drag and drop | `src/vs/platform/dnd/` | workbench | 07 | M2 | not started |
| Opener, URL handler | `src/vs/platform/{opener,url}/` | workbench | 07 | M2 | not started |
| JSON schemas | `src/vs/platform/jsonschemas/` | settings | 10 | M5 | not started |
| Git platform helpers | `src/vs/platform/git/` | scm | 10 | M4 | not started |
| Telemetry, OTel, assignment, diagnostics | `src/vs/platform/{telemetry,otel,assignment,diagnostics,profiling}/` | — | 01 | — | dropped (no telemetry in Lathe) |
| Update service | `src/vs/platform/update/`, `sign/`, `checksum/` | — | 01 | — | dropped (distribution is out of scope for v1) |
| Settings sync | `src/vs/platform/userDataSync/` | — | 01 | — | dropped (depends on a Microsoft-hosted sync service) |
| Remote, tunnels, remote tunnels | `src/vs/platform/{remote,tunnel,remoteTunnel}/` | — | 01 | — | dropped for v1 (local-only port; revisit after M6) |
| Chat, agent host, MCP, AI integrations | `src/vs/platform/{chat,agentHost,agentPlugins,mcp,artifactIntegrations,customizationMarketplace,localTranscription,webContentExtractor,onboarding}/` | — | 01 | — | dropped (Copilot and AI features are tied to hosted services and outside the core scope) |
| Webview, browser view | `src/vs/platform/{webview,browserView,webWorker}/` | — | 01 | — | dropped for v1 (needs an embedded browser engine; revisit with extensions) |
| Electron utility / shared process / sandbox | `src/vs/platform/{utilityProcess,sharedProcess,sandbox,ipc,protocol,process}/` | — | 01 | — | dropped (single process, ADR 0005) |
| Language packs | `src/vs/platform/languagePacks/` | — | 01 | — | dropped for v1 (English only; revisit after M5) |
| Policy, metered connection, network filter, default account, GitHub, external services | `src/vs/platform/{policy,meteredConnection,networkFilter,defaultAccount,github,externalServices,endpoint,dataChannel,userInteraction,cssDev,imageResize,domWidget,list,layout,editor,observable,registry,launch,markdown,globalKeybindings,actionWidget}/` | — | 01 | — | not started (triage: each splits into "absorb into an owning row" or "drop" in the first claim that touches it) |

## Editor (`src/vs/editor`)

| Area | TypeScript source | Crate | Owner | Milestone | Status |
|---|---|---|---|---|---|
| Position, range, selection, edits | `src/vs/editor/common/core/{position,range,selection}.ts` | vscode-api | 01 | P0 | done (tests) |
| Other editor core types (word helpers, char classifier, edit builders, line ranges) | `src/vs/editor/common/core/` (rest) | editor | 04 | M1 | not started |
| Text model contract | `src/vs/editor/common/model.ts`, `textModelEvents.ts` | vscode-api | 01 | P0 | done (contract only; behavior parity is tested under the "Piece tree" and "Text model" rows) |
| Piece tree text buffer | `src/vs/editor/common/model/pieceTreeTextBuffer/` | text-buffer | 03 | M1 | in progress (role 03, PR #14) |
| Text model (versioning, EOL, BOM, search) | `src/vs/editor/common/model/{textModel,textModelSearch,editStack,indentationGuesser}.ts` | text-buffer | 03 | M1 | not started |
| Encodings | `src/vs/workbench/services/textfile/common/encoding.ts` | text-buffer | 03 | M1 | not started |
| Decorations store (interval tree) | `src/vs/editor/common/model/intervalTree.ts` | text-buffer | 03 | M1 | not started |
| Bracket pairs, guides | `src/vs/editor/common/model/{bracketPairsTextModelPart,guidesTextModelPart.ts}` | editor | 04 | M3 | not started |
| Cursor and selections, multi-cursor | `src/vs/editor/common/cursor/`, `cursorCommon.ts`, `src/vs/editor/contrib/multicursor/` | editor | 04 | M1 | not started |
| Editing commands | `src/vs/editor/common/commands/`, `src/vs/editor/browser/coreCommands.ts`, `contrib/{linesOperations,wordOperations,wordPartOperations,caretOperations,comment,indentation,clipboard,cursorUndo,inPlaceReplace,insertFinalNewLine,lineSelection}/` | editor | 04 | M1 | not started |
| View model, line projection, word wrap | `src/vs/editor/common/viewModel/`, `viewModel.ts`, `modelLineProjectionData.ts`, `coordinatesConverter.ts` | editor | 04 | M1 | not started |
| Editor options | `src/vs/editor/common/config/` | editor | 04 | M1 | in progress (role 04, PR #17) |
| Find / replace | `src/vs/editor/contrib/find/` | editor | 04 | M1 | not started |
| Folding | `src/vs/editor/contrib/folding/` | editor | 04 | M1 | not started |
| Bracket matching | `src/vs/editor/contrib/bracketMatching/` | editor | 04 | M1 | not started |
| Snippets | `src/vs/editor/contrib/snippet/` | editor | 04 | M3 | not started |
| Linked editing, smart select, word highlighter | `src/vs/editor/contrib/{linkedEditing,smartSelect,wordHighlighter}/` | editor | 04 | M3 | not started |
| Line layout, scroll model | `src/vs/editor/common/viewLayout/` | editor-view | 05 | M1 | in progress (role 05, PR #18) |
| View: view lines, cursors, selections | `src/vs/editor/browser/view/`, `viewParts/{viewLines,viewCursors,selections,currentLineHighlight}/` | editor-view | 05 | M1 | not started |
| View: gutter (line numbers, glyph margin, folding margin) | `src/vs/editor/browser/viewParts/{lineNumbers,glyphMargin,margin,marginDecorations,linesDecorations}/` | editor-view | 05 | M1 | not started |
| View: minimap, overview ruler, scrollbar | `src/vs/editor/browser/viewParts/{minimap,overviewRuler,editorScrollbar,scrollDecoration}/` | editor-view | 05 | M1 | not started |
| View: decorations, indent guides, rulers, whitespace | `src/vs/editor/browser/viewParts/{decorations,indentGuides,rulers,whitespace,blockDecorations,wordWrapIndicator}/` | editor-view | 05 | M1 | not started |
| View: content and overlay widgets, view zones | `src/vs/editor/browser/viewParts/{contentWidgets,overlayWidgets,viewZones}/`, `contrib/zoneWidget/` | editor-view | 05 | M3 | not started |
| GPU line rendering | `src/vs/editor/browser/gpu/`, `viewParts/{viewLinesGpu,rulersGpu,gpuMark}/` | editor-view | 05 | M1 | not started |
| Editor input controller (mouse, keyboard, IME) | `src/vs/editor/browser/controller/` | editor-view | 05 | M1 | not started |
| Sticky scroll, unicode highlighter, long lines | `src/vs/editor/contrib/{stickyScroll,unicodeHighlighter,longLinesHelper,unusualLineTerminators}/` | editor-view | 05 | M3 | not started |
| Tokenization and language configuration | `src/vs/editor/common/{tokens,languages,tokenizationRegistry.ts,encodedTokenAttributes.ts}`, `contrib/tokenization/` | language | 06 | M3 | not started |
| Language features: hover, suggest, parameter hints | `src/vs/editor/contrib/{hover,suggest,parameterHints}/` | language | 06 | M3 | not started |
| Language features: go to, peek, rename, references | `src/vs/editor/contrib/{gotoSymbol,peekView,rename,gotoError}/` | language | 06 | M3 | not started |
| Language features: code actions, code lens, inlay hints, format, links, colors, semantic tokens, document symbols | `src/vs/editor/contrib/{codeAction,codelens,inlayHints,format,links,colorPicker,semanticTokens,documentSymbols}/` | language | 06 | M3 | not started |
| Diff editor | `src/vs/editor/browser/widget/diffEditor/`, `contrib/diffEditorBreadcrumbs/` | editor-view | 05 | M4 | not started |
| Inline completions (ghost text) | `src/vs/editor/contrib/inlineCompletions/` | language | 06 | M5 | not started |
| Standalone (Monaco) API | `src/vs/editor/standalone/`, `editor.api.ts`, `monaco.d.ts` | — | 01 | — | dropped (Monaco is a browser embedding API, not part of the app) |

## Workbench (`src/vs/workbench`)

| Area | TypeScript source | Crate | Owner | Milestone | Status |
|---|---|---|---|---|---|
| Layout and part grid | `src/vs/workbench/browser/{layout,workbench,part}.ts`, `browser/parts/partGrid/` | workbench | 07 | M2 | not started |
| Activity bar, sidebar, panel, auxiliary bar | `src/vs/workbench/browser/parts/{activitybar,sidebar,panel,auxiliarybar}/`, `paneComposite*.ts`, `compositeBar*.ts` | workbench | 07 | M2 | not started |
| Editor groups, splits, tabs | `src/vs/workbench/browser/parts/editor/`, `src/vs/workbench/services/editor/` | workbench | 07 | M2 | not started |
| Status bar | `src/vs/workbench/browser/parts/statusbar/`, `services/statusbar/` | workbench | 07 | M2 | not started |
| Title bar, banner, notifications, dialogs | `src/vs/workbench/browser/parts/{titlebar,banner,notifications,dialogs}/` | workbench | 07 | M2 | not started |
| Views and view containers | `src/vs/workbench/browser/parts/views/`, `services/views/` | workbench | 07 | M2 | not started |
| Quick open and command palette | `src/vs/workbench/contrib/quickaccess/`, `browser/quickaccess.ts`, `services/quickinput/` | workbench | 07 | M2 | not started |
| Theming (workbench colors, themes, icon themes) | `src/vs/workbench/services/themes/`, `contrib/themes/` | workbench | 07 | M5 | not started |
| Zen mode, centered layout | `src/vs/workbench/browser/workbench.zenMode.contribution.ts` | workbench | 07 | M2 | not started |
| Text files, working copies, backup, untitled | `src/vs/workbench/services/{textfile,workingCopy,untitled,textmodelResolver,filesConfiguration}/` | workspace | 08 | M1 | not started |
| File explorer | `src/vs/workbench/contrib/files/` | workspace (model) + workbench (view) | 08 / 07 | M2 | not started |
| Search and search editor | `src/vs/workbench/contrib/{search,searchEditor}/`, `services/search/` | workspace | 08 | M4 | not started |
| Workspace and workspaces contrib, local history | `src/vs/workbench/contrib/{workspace,workspaces,localHistory}/`, `services/workspaces/` | workspace | 08 | M2 | not started |
| Outline, breadcrumbs, timeline | `src/vs/workbench/contrib/{outline,timeline}/`, `services/outline/` | language | 06 | M3 | not started |
| Language status, language detection | `src/vs/workbench/contrib/{languageStatus,languageDetection}/`, `services/{language,languageStatus,languageDetection}/` | language | 06 | M3 | not started |
| TextMate and tree-sitter grammars | `src/vs/workbench/services/{textMate,treeSitter}/` | language | 06 | M3 | not started |
| Markers / problems panel | `src/vs/workbench/contrib/markers/` | language | 06 | M3 | not started |
| Call and type hierarchy | `src/vs/workbench/contrib/{callHierarchy,typeHierarchy}/` | language | 06 | M3 | not started |
| Bulk edit (workspace edits, refactor preview) | `src/vs/workbench/contrib/bulkEdit/` | language | 06 | M3 | not started |
| Snippets contrib, emmet | `src/vs/workbench/contrib/{snippets,emmet}/` | language | 06 | M5 | not started |
| Integrated terminal | `src/vs/workbench/contrib/terminal/`, `services/terminal/` | terminal | 09 | M4 | not started |
| Terminal contributions (find, links, suggest, sticky scroll, quick fix, zoom, ...) | `src/vs/workbench/contrib/terminalContrib/` | terminal | 09 | M4 | not started |
| Tasks | `src/vs/workbench/contrib/tasks/`, `services/configurationResolver/` | tasks | 09 | M4 | not started |
| Debug | `src/vs/workbench/contrib/debug/` | debug | 09 | M6 | not started |
| Output, logs | `src/vs/workbench/contrib/{output,logs}/`, `services/output/` | workbench | 07 | M4 | not started |
| Source control | `src/vs/workbench/contrib/scm/` | scm | 10 | M4 | not started |
| Merge editor, multi-diff editor | `src/vs/workbench/contrib/{mergeEditor,multiDiffEditor}/` | scm | 10 | M4 | not started |
| Settings editor and preferences | `src/vs/workbench/contrib/preferences/`, `services/preferences/`, `services/configuration/` | settings | 10 | M5 | not started |
| Keybindings editor and service | `src/vs/workbench/contrib/keybindings/`, `services/keybinding/` | settings | 10 | M5 | not started |
| User data profiles | `src/vs/workbench/contrib/userDataProfile/`, `services/userDataProfile/` | settings | 10 | M5 | not started |
| Extensions view and service | `src/vs/workbench/contrib/extensions/`, `services/{extensions,extensionManagement,extensionRecommendations}/` | extensions | 10 | M5 | not started |
| Extension host API and RPC | `src/vs/workbench/api/` | extensions | 10 | M5 | not started |
| Testing | `src/vs/workbench/contrib/testing/` | tasks | 09 | M6 | not started |
| Comments | `src/vs/workbench/contrib/comments/` | extensions | 10 | M5 | not started |
| Custom editors, webview panels and views | `src/vs/workbench/contrib/{customEditor,webview,webviewPanel,webviewView}/` | — | 01 | — | dropped for v1 (needs webviews, see the platform row) |
| Notebooks, interactive, REPL notebook | `src/vs/workbench/contrib/{notebook,interactive,replNotebook}/`, `services/notebook/` | — | 01 | — | dropped for v1 (output rendering needs webviews) |
| Welcome, walkthroughs, getting started | `src/vs/workbench/contrib/{welcomeGettingStarted,welcomeViews,welcomeWalkthrough,welcomeBanner,welcomeOnboarding,onboarding,splash}/` | workbench | 07 | M2 | not started (minimal welcome page only) |
| Accessibility contrib | `src/vs/workbench/contrib/{accessibility,accessibilitySignals}/` | workbench | 07 | M2 | not started |
| Markdown preview, media preview | `src/vs/workbench/contrib/{markdown,mediaPreview}/` | workbench | 07 | M3 | not started |
| Authentication, accounts | `src/vs/workbench/contrib/authentication/`, `services/{authentication,accounts}/` | extensions | 10 | M5 | not started |
| Chat, inline chat, agents, sessions, MCP, speech, AI | `src/vs/workbench/contrib/{chat,inlineChat,chatWindow,agentsVoice,sessions,sessionChanges,remoteCodingAgents,mcp,speech,canvases,imageCarousel,screenshotTool,editTelemetry,inlineCompletions}/`, `src/vs/sessions/` | — | 01 | — | dropped (Copilot and AI features, see the platform row) |
| Remote, tunnels, edit sessions, share | `src/vs/workbench/contrib/{remote,remoteTunnel,editSessions,share}/` | — | 01 | — | dropped for v1 (local only) |
| Telemetry, surveys, performance, issue reporter, update, relauncher, tags | `src/vs/workbench/contrib/{telemetry,surveys,performance,issue,update,relauncher,tags,bracketPairColorizer2Telemetry,emergencyAlert,limitIndicator}/` | — | 01 | — | dropped (no telemetry or hosted services; relaunch handled by `lathe`) |
| Settings sync contrib | `src/vs/workbench/contrib/userDataSync/` | — | 01 | — | dropped (see the platform row) |
| Localization | `src/vs/workbench/contrib/localization/`, `services/localization/` | — | 01 | — | dropped for v1 (English only) |
| Remaining contribs | `src/vs/workbench/contrib/{codeEditor,commands,dropOrPasteInto,externalTerminal,externalUriOpener,folding,format,inlayHints,codeActions,list,opener,processExplorer,sash,scrollLocking,url,keybindingsExport,policyExport,meteredConnection,modernUI,git,github,encryption,browserView}/` | — | 01 | — | not started (triage: each goes to the row whose behavior it extends, or is dropped with a reason, in the first claim that touches it) |

## Application, CLI, server

| Area | TypeScript source | Crate | Owner | Milestone | Status |
|---|---|---|---|---|---|
| Desktop entry point, argv, window restore | `src/vs/code/`, `src/main.ts`, `src/vs/platform/environment/node/argv.ts` | lathe | 01 | M1 | not started |
| Workbench wiring (service registration) | `src/vs/workbench/workbench.desktop.main.ts`, `workbench.common.main.ts` | lathe | 01 | M2 | not started |
| CLI (`code` launcher, tunnels) | `cli/src/` | lathe | 01 | M2 | not started (launcher and argv only; tunnels dropped with remote) |
| Server / web | `src/vs/server/`, `src/vs/workbench/workbench.web.main*.ts`, `src/vs/code/browser/` | — | 01 | — | dropped (native desktop only) |

## Built-in extensions (`extensions/`)

Built-in extensions are JavaScript and run on the extension host (ADR 0004). Grammar-only and
theme-only extensions are data and are loaded natively by `language` and `workbench`.

| Area | TypeScript source | Crate | Owner | Milestone | Status |
|---|---|---|---|---|---|
| Language grammars and configurations (TS, Rust, Python, JSON, Markdown for M3; the rest after) | `extensions/{typescript-basics,javascript,rust,python,json,markdown-basics,...}/` | language | 06 | M3 | not started |
| Color and icon themes | `extensions/theme-*/` | workbench | 07 | M5 | not started |
| Git extension behavior (status, diff, commit) | `extensions/{git,git-base}/` | scm | 10 | M4 | not started (reimplemented natively on gix; not run as an extension) |
| JSON language features (settings schemas) | `extensions/json-language-features/`, `configuration-editing/` | settings | 10 | M5 | not started |
| Language-feature extensions (TS, CSS, HTML, Markdown, PHP) | `extensions/*-language-features/` | extensions | 10 | M5 | not started (run on the extension host) |
| npm, gulp, grunt, jake task detection | `extensions/{npm,gulp,grunt,jake}/` | extensions | 10 | M5 | not started (run on the extension host) |
| Debug auto launch, server ready | `extensions/{debug-auto-launch,debug-server-ready}/` | extensions | 10 | M6 | not started (run on the extension host) |
| Merge conflict, references view, search result, simple browser, emmet | `extensions/{merge-conflict,references-view,search-result,simple-browser,emmet}/` | extensions | 10 | M5 | not started (run on the extension host) |
| Copilot, GitHub, Microsoft auth, tunnel forwarding | `extensions/{copilot,github,github-authentication,microsoft-authentication,tunnel-forwarding}/` | — | 01 | — | dropped (hosted services) |
| Extension test suites | `extensions/vscode-api-tests/`, `vscode-colorize-tests/`, `vscode-colorize-perf-tests/`, `vscode-test-resolver/` | extensions / language | 10 / 06 | M5 | not started (used as parity fixtures) |
