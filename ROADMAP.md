---
created: 2026-04-19
updated: 2026-05-02
status: active
priority: high
type: roadmap
completed_through: v10
next_version: v11
final_scheduled_version: v12
---

# Complete Roadmap: Mini GNOME Text Editor in Rust

## Product direction

This project is a GNOME-native desktop text editor built in Rust.

The editor starts as a minimal plain-text application and evolves gradually into a more capable lightweight editor for text, code, and structured files. The roadmap is intentionally designed to stay focused and restrained.

The product should remain:

* GNOME-native
* lightweight
* clean in structure
* practical for real use
* extensible over time

The product should **not** become a full IDE.

Explicit non-goals across the roadmap:

* No LSP integration
* No debugger integration
* No terminal pane
* No plugin system
* No advanced refactoring tools
* No build/run tooling

---

# V1 — Minimal usable GNOME text editor

> created: 2026-04-19
> updated: 2026-04-19
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `77e7643` — Add embedded Riteed GNOME editor app

## Purpose

V1 establishes the foundation: a small but proper GNOME desktop app for opening, editing, and saving plain `.txt` files.

This version is about creating the core application structure correctly from the start. It should already feel like a real GNOME app, not a prototype.

## What V1 is

V1 is:

* a plain-text editor
* single-document focused
* simple and native in feel
* designed to grow later

## Key features

* Open `.txt` files
* Create a new document
* Edit text
* Save
* Save As
* About dialog
* Preferences
* Basic keyboard shortcuts
* Theme preference: System / Light / Dark
* Word wrap option
* Remember window size
* Unsaved-changes protection

## Why this version matters

This is the architectural base. If V1 is clean, later versions can add tabs, file awareness, syntax, and workspace features without large rewrites.

## Prompt for V1

```text
Build a v1 GNOME desktop application in Rust: a small, clean text editor for plain `.txt` files.

The goal is to create a minimal but proper GNOME app, not just a demo. It should feel native to GNOME, use GTK4 and Libadwaita, follow GNOME HIG where practical, and have a clean structure that can grow later.

What we are creating:
- A lightweight text editor for opening, viewing, editing, and saving plain text files
- A simple GNOME-style application with a main window, header bar, menu, preferences, about dialog, and keyboard shortcuts
- A foundation for future features like tabs, recent files, search, and session restore

Scope for v1:
- Open existing `.txt` files
- Create a new empty document
- Edit text in a central text area
- Save and Save As
- Prompt before closing if there are unsaved changes
- Basic preferences:
  - theme preference: System / Light / Dark
  - word wrap on/off
  - remember window size
- About dialog
- Keyboard shortcuts window
- Standard shortcuts such as New, Open, Save, Save As, Quit
- Basic file error handling with user-friendly messages

Technical expectations:
- Rust
- GTK4 via gtk4-rs
- Libadwaita
- GSettings for preferences
- gettext-ready strings for future i18n
- Clear app/module structure
- Keep the code simple, readable, and maintainable
- Avoid overengineering and avoid adding features outside v1 unless required to support the architecture

Non-goals for v1:
- No rich text
- No syntax highlighting
- No plugin system
- No database
- No cloud sync
- No advanced multi-document workflow unless it is needed for a clean design
- No custom theming beyond System / Light / Dark

Implementation guidance:
- Prefer GNOME conventions over custom UI patterns
- Use app actions and accelerators cleanly
- Separate application logic, window logic, document state, settings, and dialogs into sensible modules
- Design the code so tabs and recent files can be added later without major restructuring

Deliverable:
Create the initial application structure and implement a working v1 of this mini text editor, with code organized clearly enough that it can serve as the base for future versions.
```

---

# V2 — Multi-document workflow

> created: 2026-04-19
> updated: 2026-04-19
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `3afc4b8` — Implement Riteed v2 tabbed editor workflow

## Purpose

V2 makes the app practical for everyday use by adding basic multi-document behavior and smoother reopen flows.

This is the step from “small single-file editor” to “usable daily editor”.

## What V2 adds

* Tabs
* Recent files
* Restore last session
* Proper unsaved-changes handling
* Drag-and-drop file opening

## Why this version matters

V2 introduces document lifecycle complexity:

* multiple open documents
* reopen behavior
* closing logic
* startup restore
* external file opening via drag-and-drop

That makes it the first real “editor workflow” release.

## Prompt for V2

```text
Build v2 of the GNOME desktop application in Rust by extending the existing mini plain-text editor from v1.

The goal of v2 is to make the app more practical for daily use while keeping it lightweight, native to GNOME, and easy to maintain. This version should still feel minimal, but it should now support basic multi-document workflows and a smoother document-opening experience.

What v2 adds:
- Tabs for working with multiple text documents in the same window
- Recent files support
- Restore last session on app startup
- Proper unsaved-changes handling
- Drag-and-drop file opening

Scope for v2:
- Add tabbed document support
  - Open multiple documents in one window using tabs
  - Each tab should display a sensible title based on the file name or “Untitled” for new documents
  - Switching, closing, and creating tabs should be straightforward and GNOME-appropriate
- Add a recent files feature
  - Track recently opened files
  - Provide a simple way to reopen them from the UI
  - Avoid duplicating entries unnecessarily
- Add session restore
  - Restore the set of previously open documents when the app starts again
  - Restore enough state to make reopening feel useful, but keep implementation simple
- Add an unsaved-changes dialog
  - Prompt the user before closing a tab, closing the window, or quitting the app if there are unsaved changes
  - The dialog should clearly support cancel, discard, and save flows
- Add drag-and-drop support
  - Allow text files to be opened by dropping them onto the window
  - If appropriate, dropped files should open in new tabs

Behavior expectations:
- The app should remain clean and responsive even with multiple open tabs
- Tab behavior should follow GNOME conventions where practical
- Recent files and session restore should work reliably without introducing unnecessary complexity
- Unsaved-changes logic should be consistent across all close flows
- Drag-and-drop should integrate naturally with the existing open-document workflow

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita codebase
- Preserve a clear module structure
- Reuse and improve existing document state management rather than duplicating logic
- Store recent files and session state in a simple, maintainable way
- Keep GSettings and other persisted preferences aligned with the existing architecture
- Keep all user-facing strings ready for gettext-based localization

Non-goals for v2:
- No rich text
- No syntax highlighting
- No split view
- No advanced session management beyond reopening prior documents
- No complex file history or version tracking
- No custom workspace management

Implementation guidance:
- Design tabs as a natural extension of the existing single-document architecture
- Centralize document lifecycle handling so open, close, restore, drag-and-drop, and recent-file reopening all use the same core flow
- Treat unsaved document state as a first-class concern across tabs and app shutdown
- Keep the UI simple and GNOME-native rather than feature-heavy

Deliverable:
Implement a working v2 of the mini GNOME text editor with tabs, recent files, session restore, unsaved-changes handling, and drag-and-drop file opening, while keeping the codebase clean enough to support future versions.
```

---

# V3 — Editing polish and daily usability

> created: 2026-04-19
> updated: 2026-04-19
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `013dfd1` — Build Riteed v3 editing workflow

## Purpose

V3 improves the editing experience itself.

This version is not about new document models or code editing yet. It is about the core text editing workflow feeling complete enough for daily use.

## What V3 adds

* Search
* Replace
* Line numbers
* Better document status and editing feedback
* Better usability for longer text files

## Why this version matters

This is where the app stops feeling minimal and starts feeling comfortable.

It also prepares the architecture for a future editor upgrade without yet introducing GtkSourceView complexity.

## Prompt for V3

```text
Build v3 of the GNOME desktop application in Rust by improving the editing workflow and overall usability of the existing mini text editor.

The goal of v3 is to make the app feel practical and polished for everyday text editing while still keeping it lightweight, GNOME-native, and easy to maintain. This version should improve the core editing experience without turning the app into a code editor or an IDE.

What v3 adds:
- Search
- Replace
- Line numbers
- Better document status and editing feedback
- Improved editor usability for longer text files

Scope for v3:
- Add in-document search
  - Support finding text within the current document
  - Provide next and previous match navigation
  - Make the search UI simple and GNOME-appropriate
- Add replace
  - Replace the current match
  - Replace all matches in the current document
  - Keep the interaction clear and safe
- Add line numbers
  - Make line numbers available in the editor view
  - Keep them visually unobtrusive
- Add basic status information
  - Show useful editor state such as line and column position
  - Show whether the current document is modified
  - Show a sensible file name or untitled state
- Improve editing ergonomics
  - Make handling of large or longer text documents feel stable and clean
  - Keep scrolling, cursor movement, and search interactions responsive
- Improve document behavior where needed
  - Ensure unsaved-changes state remains accurate
  - Keep tab titles and window state in sync with document state

Behavior expectations:
- The app should remain minimal and fast
- Search and replace should be straightforward and suitable for everyday use
- Line numbers and status information should support usability without cluttering the interface
- The app should still feel like a plain-text-focused GNOME editor, not a development tool

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita codebase
- Preserve a clean architecture and keep document state centralized
- Integrate search, replace, and status updates without duplicating document logic
- Keep user-facing strings ready for gettext localization
- Avoid overengineering and avoid features that belong in later versions

Non-goals for v3:
- No syntax highlighting
- No minimap
- No project sidebar
- No LSP integration
- No terminal pane
- No plugin system
- No advanced IDE-style tooling

Implementation guidance:
- Treat search and replace as part of the editor workflow, not as isolated UI features
- Keep status information lightweight and useful
- Add only the amount of structure needed to make later GtkSourceView migration easier
- Preserve GNOME conventions and avoid custom UI patterns unless clearly necessary

Deliverable:
Implement a working v3 of the mini GNOME text editor with search, replace, line numbers, and improved editing feedback, while keeping the app lightweight, clean, and ready for future upgrades.
```

---

# V4 — Lightweight code-friendly editor

> created: 2026-04-19
> updated: 2026-04-19
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `6aee029` — Build Riteed v4 code editor workflow

## Purpose

V4 is the editor-engine upgrade.

The app evolves from a plain-text tool into a lightweight editor for code, markdown, config files, and structured text.

## What V4 adds

* GtkSourceView integration
* Syntax highlighting
* Optional minimap
* Real-time file monitoring
* Smarter file and language handling

## Why this version matters

This is the biggest technical jump in the roadmap.

It introduces real source-editing capabilities, while still keeping the product out of IDE territory.

## Prompt for V4

```text
Build v4 of the GNOME desktop application in Rust by evolving the existing mini text editor into a lightweight code-friendly editor.

The goal of v4 is to introduce source-editing capabilities and stronger file awareness while keeping the app fast, simple, and GNOME-native. This version should support common workflows for editing code, config files, markdown, and other structured text, but it should still remain a lightweight editor rather than a full IDE.

What v4 adds:
- GtkSourceView integration
- Syntax highlighting
- Optional minimap
- Real-time file monitoring
- Smarter file and language handling

Scope for v4:
- Integrate GtkSourceView into the existing editor architecture
  - Replace or extend the current text editing component with a GtkSourceView-based editor
  - Preserve the existing document and tab workflow where possible
- Add syntax highlighting
  - Detect supported languages automatically when practical
  - Apply highlighting for code, config files, markdown, and structured text formats where available
- Add optional minimap
  - Provide a document overview minimap
  - Make it easy to enable or disable
  - Keep it optional so the editor still feels clean and lightweight
- Add real-time file monitoring
  - Monitor open files for external changes on disk
  - Notify the user when an open file has changed outside the app
  - If the document has no unsaved local changes, support a simple reload path
  - If the document has unsaved local changes, require explicit user choice before replacing in-memory content
- Add smarter file awareness
  - Keep file identity, path state, and external-change handling centralized per document
  - Improve how the editor reacts to renamed, deleted, or externally modified files when practical

Behavior expectations:
- The app should still feel like a lightweight GNOME editor
- Syntax highlighting should improve readability without adding complexity to the UI
- The minimap should be optional and should not dominate the interface
- External file changes should never silently destroy unsaved user work
- The editor should remain responsive and clean even when multiple documents are open

Technical expectations:
- Use Rust
- Use GTK4 and Libadwaita
- Use GtkSourceView 5 where appropriate
- Keep the module structure clean and maintainable
- Centralize document lifecycle, reload logic, and file monitoring behavior
- Reuse existing tab, settings, and document infrastructure wherever possible
- Keep all user-facing strings ready for gettext localization
- Avoid overengineering and avoid IDE-style features outside the requested scope

Non-goals for v4:
- No LSP integration
- No debugger integration
- No terminal pane
- No project tree or full workspace model
- No plugin system
- No advanced refactoring tools
- No build/run tooling

Implementation guidance:
- Treat GtkSourceView migration as a foundation for future editor capabilities
- Keep file monitoring logic robust and conservative in order to protect user edits
- Prefer GNOME-native patterns and restrained UI over feature-heavy design
- Build only the editor features needed for this version, while leaving room for future enhancements

Deliverable:
Implement a working v4 of the app as a lightweight GNOME editor with GtkSourceView, syntax highlighting, an optional minimap, real-time file monitoring, and improved file-aware behavior.
```

---

# V5 — Editor control and text-format awareness

> created: 2026-04-23
> updated: 2026-04-23
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `063944c`, `67423d6`, `27836c5` — v5 controls, internals, and policy fixes

## Purpose

V5 makes the editor smarter and more precise.

It focuses on the details that matter when editing structured text, config files, and code-like content.

## What V5 adds

* Automatic indentation
* Tabs vs spaces control
* Tab width control
* Line ending awareness
* Encoding awareness
* Font selection
* Zoom controls

## Why this version matters

This is where the app becomes a genuinely capable editor instead of just a viewer/editor with syntax colors.

## Prompt for V5

```text
Build v5 of the GNOME desktop application in Rust by improving editor control, text-format awareness, and everyday editing precision.

The goal of v5 is to make the app feel like a more capable lightweight editor for plain text, code, and structured text files, while still keeping it simple, GNOME-native, and clearly not an IDE. This version should focus on editor behavior, file-text conventions, and user control over how text is displayed and inserted.

What v5 adds:
- Automatic indentation
- Control over tabs vs spaces
- Tab width preferences
- Line ending awareness and control
- Character encoding awareness and handling
- Font selection
- Zoom controls

Scope for v5:
- Add automatic indentation behavior where appropriate
  - New lines should inherit or continue indentation in a sensible way
  - Keep indentation behavior predictable and lightweight
- Add indentation preferences
  - Let the user choose whether indentation inserts tabs or spaces
  - Let the user control tab width / indentation width
- Add line ending awareness
  - Detect common line ending styles when opening files
  - Preserve existing line endings where practical
  - Allow the user to view and optionally change line ending style
- Add encoding awareness
  - Handle text file encodings more explicitly
  - Surface useful information when opening files with non-default encodings
  - Keep behavior safe and understandable
- Add font controls
  - Let the user choose the editor font, ideally with support for a monospace default
  - Keep this integrated into preferences in a GNOME-appropriate way
- Add zoom controls
  - Support zoom in, zoom out, and reset zoom
  - Make zoom affect the editing experience cleanly without changing unrelated UI elements
- Improve editor feedback
  - Show useful information such as line ending mode, encoding, and zoom level where appropriate
  - Keep the interface restrained and uncluttered

Behavior expectations:
- The app should remain lightweight and fast
- Editor behavior should feel predictable and practical for real text editing
- Tabs/spaces, indentation, line endings, and encoding should be treated as first-class editor concerns
- Font and zoom should improve readability without turning the app into a heavily customizable environment

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita + GtkSourceView codebase
- Reuse and strengthen the existing document model
- Keep text-format metadata centralized per document where possible
- Preserve a clean, maintainable module structure
- Keep all user-facing strings ready for gettext localization
- Avoid overengineering and avoid IDE-like scope expansion

Non-goals for v5:
- No project tree
- No open-folder workspace model
- No LSP integration
- No debugger integration
- No terminal pane
- No plugin system
- No advanced refactoring tools
- No build or run tooling

Implementation guidance:
- Treat indentation, line endings, encoding, font, and zoom as core editor capabilities
- Prefer simple, robust behavior over highly configurable edge-case handling
- Preserve GNOME-native conventions and avoid dense or technical UI unless clearly useful
- Build the document model so later folder navigation and diff features can be added cleanly

Deliverable:
Implement a working v5 of the app with automatic indentation, tabs-versus-spaces settings, tab width control, line ending and encoding awareness, font selection, and zoom controls, while keeping the app lightweight, clear, and maintainable.
```

---

# V6 — Workspace navigation

> created: 2026-04-24
> updated: 2026-04-24
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `d299e3c`, `63bed14` — v6 folder navigation and project-tree auto-refresh fallback

## Purpose

V6 introduces lightweight workspace behavior.

The app is still not a full IDE, but it becomes much better for working across sets of related files.

## What V6 adds

* Open folder support
* Project tree view
* Split-pane layout
* Better multi-file navigation
* Automatic refresh for loaded project-tree folders where practical

## Why this version matters

This is the step where the app scales from “many open files” to “many related files”.

It creates the navigation architecture needed for compare features later.

## Prompt for V6

```text
Build v6 of the GNOME desktop application in Rust by introducing lightweight workspace navigation for working with multiple files more efficiently.

The goal of v6 is to make the app more effective for navigating and editing sets of related files, while still remaining a lightweight GNOME editor rather than a full IDE. This version should add folder-oriented navigation, a project-style file view, and split-pane layout support without introducing build tools, debugging, or language intelligence.

What v6 adds:
- Open folder / workspace support
- Project tree view
- Split-pane layout
- Better multi-file navigation

Scope for v6:
- Add open-folder support
  - Allow the user to open a folder as a lightweight workspace
  - Keep the concept simple and document-oriented rather than IDE-like
- Add a project tree / file tree sidebar
  - Display files and folders in a navigable tree view
  - Make it easy to open files from the sidebar
  - Keep the sidebar visually clean and GNOME-appropriate
- Add split-pane layout
  - Support a sidebar-and-editor layout for file navigation
  - Make the split layout adaptive and well-behaved across window sizes
- Improve multi-file workflows
  - Keep tabs working naturally alongside the project tree
  - Make it easy to switch between open documents and files in the current folder
- Improve workspace behavior where useful
  - Keep selection, current file, and active tab state synchronized
  - Handle deleted, renamed, or externally changed files sensibly when practical
- Keep the app restrained
  - This should remain a lightweight editor with file navigation, not a full development environment

Behavior expectations:
- Opening a folder should feel simple and natural
- The project tree should improve navigation without making the UI heavy
- The split layout should support a clean sidebar + editor workflow
- Tabs and the tree view should complement each other instead of competing
- The app should remain responsive and uncluttered

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita + GtkSourceView codebase
- Use GNOME-appropriate split navigation patterns where practical
- Keep workspace state and document state clearly separated but well integrated
- Preserve a maintainable architecture that can later support compare/diff workflows
- Keep all user-facing strings ready for gettext localization
- Avoid overengineering and avoid IDE-style subsystems

Non-goals for v6:
- No LSP integration
- No debugger integration
- No terminal pane
- No plugin system
- No advanced refactoring tools
- No build or run tooling
- No full IDE workspace management

Implementation guidance:
- Treat open-folder support as a lightweight navigation feature, not a project system
- Keep the file tree focused on browsing and opening files
- Make the split-pane layout adaptive, clean, and GNOME-native
- Build the selection and document architecture so later compare features can reuse it

Deliverable:
Implement a working v6 of the app with open-folder support, a project tree view, split-pane layout, and improved multi-file navigation, while preserving the app’s identity as a lightweight GNOME editor.
```

---

# V7 — Compare and advanced split workflows

> created: 2026-04-24
> updated: 2026-04-24
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `2e42722`, `e97f857` — v7 compare prep and compare workflows

## Purpose

V7 adds powerful comparison workflows.

The goal is to support diff and side-by-side inspection without turning the app into a merge tool or VCS client.

## What V7 adds

* Diff support
* Side-by-side compare view
* Compare current buffer with saved/on-disk version
* Compare two files
* Manual reference refresh
* Difference navigation
* Stronger split-view workflows

## Why this version matters

This is the “power user” release. It adds the strongest analysis feature in the roadmap while still staying within the editor’s product identity.

## Prompt for V7

```text
Build v7 of the GNOME desktop application in Rust by adding document comparison and advanced split-view workflows.

The goal of v7 is to introduce powerful compare-focused editing features while keeping the application lightweight, GNOME-native, and explicitly outside the scope of a full IDE. This version should help users inspect differences between files and between in-memory edits and on-disk content in a clear and practical way.

What v7 adds:
- Diff support
- Side-by-side compare view
- Compare current buffer with saved/on-disk version
- Compare two files in a split view
- Stronger split editing workflow

Scope for v7:
- Add compare / diff capabilities
  - Allow the user to compare two files
  - Allow the user to compare the current document with its saved version or on-disk state where practical
- Add side-by-side compare view
  - Present compared content in a clear split layout
  - Make the comparison readable and useful without overwhelming the interface
- Improve split workflows
  - Support editor layouts that make compare operations feel natural
  - Keep the layout clean and manageable within the existing GNOME application style
- Improve compare-related navigation
  - Make it easy to move through differences
  - Make it clear which side is editable and which side is reference content where applicable
- Keep compare behavior safe
  - Avoid destructive or confusing flows when unsaved changes are involved
  - Make the relationship between live document state and comparison state explicit
- Preserve the editor identity
  - This should remain a lightweight editor with compare tools, not a merge tool, IDE, or version control client

Behavior expectations:
- Comparing files should be straightforward and visually understandable
- Split compare views should feel integrated into the existing workspace and tab model
- The user should be able to tell what is editable, what is read-only, and what is being compared
- The app should remain responsive and should not become cluttered with development-oriented tooling

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita + GtkSourceView codebase
- Reuse the split and workspace architecture introduced in earlier versions
- Keep compare state and document state clearly modeled
- Keep all user-facing strings ready for gettext localization
- Maintain a clean architecture that avoids unnecessary complexity

Non-goals for v7:
- No LSP integration
- No debugger integration
- No terminal pane
- No plugin system
- No advanced refactoring tools
- No build or run tooling
- No full merge conflict resolution system
- No version control client features

Implementation guidance:
- Treat diff as a document comparison feature, not as source control tooling
- Keep compare UI focused, restrained, and easy to understand
- Prefer robust file-to-file and buffer-to-file comparison flows over feature-heavy merge functionality
- Reuse earlier split-view and workspace infrastructure rather than creating a separate parallel UI model

Deliverable:
Implement a working v7 of the app with diff support, side-by-side compare views, buffer-versus-file comparison where practical, and stronger split-view workflows, while preserving the app’s role as a lightweight GNOME editor.
```

---

# V8 — Polish, accessibility, and editing safety

> created: 2026-04-24
> updated: 2026-04-24
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: `e8f4b0c`, `0abb36a`, `095191f`, `657d1e5`, `0a43d75` — V8 polish, accessibility, appearance, compare, and editing-safety work

## Purpose

V8 focuses on product maturity.

The goal is to make the editor feel more complete, trustworthy, and comfortable in everyday use. This version is about polish, accessibility, editor comfort, and stronger protection against accidental data loss.

## What V8 adds

* Editor palette selection
* Fullscreen support
* Accessibility improvements
* Current-line highlight toggle
* Autosave support
* Best-effort safe save behavior
* Save-polish features

## Why this version matters

V8 is where the product starts to feel finished rather than merely feature-complete. It improves readability, workflow comfort, reliability, and resilience while keeping the app lightweight and restrained.

## Prompt for V8

```text
Build v8 of the GNOME desktop application in Rust by focusing on polish, accessibility, editing safety, and overall product maturity.

The goal of v8 is to make the editor feel more complete, trustworthy, and comfortable in everyday use. This version should improve presentation, accessibility, save reliability, and user confidence without changing the product into an IDE or adding large new subsystems.

What v8 adds:
- Editor palette selection
- Fullscreen support
- Accessibility improvements
- Current-line highlight toggle
- Autosave support
- Best-effort safe save behavior
- Save-polish features

Scope for v8:
- Add editor palette selection
  - Allow the user to choose between a small curated set of editor color palettes
  - Keep palette selection focused on the editor surface rather than introducing full custom application theming
  - Make palette behavior work cleanly with syntax highlighting and dark/light modes where practical
- Add fullscreen support
  - Support fullscreen toggle, including an F11 shortcut
  - Make fullscreen behavior feel native and predictable
- Improve accessibility
  - Improve keyboard navigation across the app
  - Ensure controls have clear labels and accessible names
  - Strengthen focus visibility and general UI clarity
  - Improve contrast and readability where practical
  - Avoid relying on color alone to communicate important state
- Add current-line highlight support
  - Allow the user to enable or disable highlighting of the active line
  - Keep the visual treatment subtle, readable, and non-distracting
  - Ensure it works well across palettes, light/dark presentation, and accessibility-focused usage
- Add autosave support
  - Provide a user-facing toggle for autosave behavior
  - Define autosave conservatively and clearly so users understand when document contents are persisted
  - Limit autosave to already-saved writable files with no pending external conflict
- Add best-effort safe save behavior
  - Make file saving more robust and less likely to corrupt files during failures or interruptions
  - Ensure save flows remain safe and predictable
- Add save-polish behavior
  - Improve save conflict handling when files have changed on disk
  - Make read-only or unwritable file states clearer to the user
  - Preserve useful editing continuity such as cursor or scroll position where practical

Behavior expectations:
- The app should feel more polished and trustworthy
- Accessibility improvements should be practical and visible in everyday use
- Current-line highlighting should improve focus without becoming visually heavy
- Autosave and save-polish features should reduce the risk of data loss without creating confusing save behavior
- Palette support should improve editor comfort without turning the app into a fully theme-customizable environment
- Fullscreen should feel simple and native

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita + GtkSourceView codebase
- Keep editor presentation and save behavior clearly separated but well integrated
- Preserve a clean architecture and avoid one-off feature hacks
- Keep all user-facing strings ready for gettext localization
- Avoid overengineering and avoid scope expansion into IDE-style systems

Non-goals for v8:
- No LSP integration
- No debugger integration
- No terminal pane
- No plugin system
- No advanced refactoring tools
- No build or run tooling
- No full custom application theme engine
- No major new workspace model

Implementation guidance:
- Treat v8 as a product maturity release rather than a platform shift
- Prefer reliability, clarity, and accessibility over feature quantity
- Keep palette support curated and editor-focused
- Make autosave and best-effort safe save behavior explicit and conservative
- Preserve GNOME-native conventions throughout the UI

Deliverable:
Implement a working v8 of the app with editor palette selection, fullscreen support, accessibility improvements, current-line highlight support, autosave, best-effort safe save behavior, and stronger save polish, while preserving the app’s identity as a lightweight GNOME-native editor.
```

---

# V9 — Lightweight Git source control sidebar

> created: 2026-04-24
> updated: 2026-04-25
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: pending commit — V9 lightweight Git source control sidebar

## Purpose

V9 introduces practical Git awareness and basic source control workflows through a dedicated side panel.

The goal is to support repository state, changed files, per-file diffs, and local commits directly in the editor while keeping the app clearly outside the scope of a full IDE or standalone Git client.

## What V9 adds

* Git repository detection
* Source control side panel
* File status in the project tree
* Per-file Git diff
* Stage and unstage actions
* Commit workflow
* Refreshable repository state
* Explicit deferral of lightweight recent-commit history and discard-file-changes

## Why this version matters

V9 ties together the workspace model, diff infrastructure, and file awareness built in earlier versions. It gives the editor a practical source-control workflow without changing the product into a complex development environment.

## Prompt for V9

```text
Build v9 of the GNOME desktop application in Rust by adding lightweight Git source control support through a dedicated side panel.

The goal of v9 is to introduce practical, editor-friendly Git integration without turning the application into a full Git client or IDE. This version should help users understand repository state, inspect changed files, review diffs, and perform simple commit workflows directly inside the app.

What v9 adds:
- Git repository detection
- Source control side panel
- File status in the project tree
- Per-file diff from Git state
- Stage and unstage actions
- Commit workflow
- Refreshable repository state
- Optional lightweight recent-commit history

Scope for v9:
- Detect whether the current folder or open file belongs to a Git repository
- Show basic repository information such as the current branch
- Add a separate source control side panel
  - Present changed files clearly
  - Keep the UI lightweight and easy to scan
  - Make refresh behavior explicit and reliable
- Show Git file state in the project tree where practical
  - Modified
  - Added
  - Deleted
  - Untracked
  - Staged where appropriate
- Support per-file diff
  - Let the user inspect Git changes for a selected file
  - Reuse the existing split/diff infrastructure where possible
- Support stage and unstage actions
  - Allow file-level staging and unstaging
  - Keep the flows simple and predictable
- Add commit support
  - Provide a lightweight commit UI, either inline in the side panel or through a simple dialog
  - Require a commit message
  - Keep the flow focused on normal local commits
- Add refresh state support
  - Allow manual refresh of repository status
  - Keep repository state synchronized with editor saves where practical
- Defer lightweight recent commit history to a follow-up
  - Do not ship a log browser in the first V9 delivery
  - Keep the source control sidebar focused on status, diff, staging, and commit

Behavior expectations:
- The app should remain a lightweight GNOME editor, not a full source control client
- Git features should support the editing workflow rather than dominate it
- Changed files should be easy to scan and open
- Diff views should feel integrated with the existing compare/split workflow
- Commit actions should be simple, safe, and understandable

Technical expectations:
- Use the bundled sandbox Git CLI at `/app/bin/git`; never use host Git or `flatpak-spawn`
- Execute Git commands safely via subprocess calls without shell-string shortcuts
- Run Git operations off the main UI thread
- Parse stable machine-friendly Git output where practical
- Handle unavailable bundled Git and non-repository folders gracefully
- Keep Git logic centralized in a dedicated service layer
- Reuse the existing workspace, diff, and document architecture wherever possible
- Keep all user-facing strings ready for gettext localization

Recommended UI shape:
- A dedicated source control side panel
- Repository / branch information near the top
- A refresh action
- A commit message entry and commit action
- A changes list with file status indicators
- Optional recent commits section below the changes list

Nice-to-have for v9:
- Current branch display
- Discard file changes (deferred)
- A filter or quick view for changed files only
- A lightweight recent commits list (deferred)

Non-goals for v9:
- No branch switching UI
- No merge conflict resolution UI
- No stash manager
- No rebase or cherry-pick tooling
- No push/pull UI
- No blame view
- No GitHub or GitLab integration
- No full log browser
- No advanced repository management

Implementation guidance:
- Treat Git support as a lightweight editor-side workflow
- Reuse the existing diff system rather than building a separate comparison UI
- Keep state transitions explicit and safe
- Prefer clarity and reliability over feature breadth
- Preserve GNOME-native patterns and avoid turning the side panel into a complex dashboard

Deliverable:
Implement a working v9 of the app with Git repository detection, a lightweight source control side panel, project-tree file status indicators, per-file diff, stage/unstage actions, commit support, and refreshable repository state, while preserving the app’s identity as a lightweight GNOME-native editor. Lightweight recent history and discard-file-changes are deferred to follow-up work.
```

---

# V10 — Source control completion and UX regressions

> created: 2026-04-26
> updated: 2026-04-26
> status: complete
> priority: high
> type: roadmap-milestone
> implementation: pending commit — V10 local source-control completion and UX regression pass

## Purpose

V10 finishes the local source control work started in V9 and clears the polish regressions that have accumulated since V8.

The goal is to make local Git workflows feel complete enough for daily use while restoring the visual and interaction polish the app expects. Network-bound source control (push) is intentionally held out of V10 so the security-sensitive bundled Git rebuild can be reviewed on its own; it now lives in the Post-V12 unscheduled candidates and is not part of the current roadmap.

## What V10 adds

* Lightweight recent commit history in the source control side panel (deferred from V9)
* Discard file changes action (deferred from V9)
* Source control list view as an alternative to the current presentation
* Live source control monitoring with automatic state refresh
* Source Control icon regression fix
* Diff view syntax highlighting fix
* Diff view line number alignment fix
* Appearance trigger moved from the header bar to the main menu
* Recent Files dialog layout fix
* Sidebar slide animation regression fix
* Distinctive dividers between status bar segments
* First-open Appearance dialog tile sizing fix
* Updated help and refreshed translation catalog

## Why this version matters

V10 turns V9's local source-control core into a complete local workflow. The deferred history and discard items finish the local loop, while the polish work removes the lingering regressions that signal "still in motion". Push is intentionally held back so its security-sensitive bundled Git rebuild can be reviewed on its own; it sits in the Post-V12 unscheduled backlog and is not part of the active roadmap. After V10 the app should feel like the V9 plan was always going to land here.

## Prompt for V10

```text
Build v10 of the GNOME desktop application in Rust by completing the lightweight source control workflow started in v9 and resolving the polish regressions that have accumulated since v8.

The goal of v10 is to make Git workflows feel complete enough for daily use, restore the visual and interaction polish the app should have, and clear the small bug list — without expanding scope into IDE territory or full Git client territory.

What v10 adds:
- Lightweight recent commit history in the source control side panel
- Discard file changes action
- Source control list view alternative
- Live source control monitoring with automatic state refresh
- Source Control icon regression fix
- Diff view syntax highlighting
- Diff view line number alignment
- Appearance trigger moved from header bar to main menu
- Recent Files dialog layout fix
- Sidebar slide animation regression fix
- Distinctive dividers between status bar segments
- First-open Appearance dialog tile sizing fix
- Updated help docs and refreshed Danish translation catalog

Scope for v10:
- Add lightweight recent commit history
  - Show a small list of recent commits below the changes list
  - Keep the view read-only and focused on quick orientation
  - Do not build a full log browser
- Add discard file changes
  - Allow per-file discard of unstaged changes
  - Require explicit confirmation, especially when the document is open
  - Preserve the editor safety guarantees from v8
- Add a source control list view
  - Provide a flat list as an alternative to the current presentation
  - Keep both views accessible without dominating the UI
- Add live source control monitoring
  - Watch the working tree for changes and refresh state automatically
  - Coalesce events so refresh stays cheap and predictable
  - Reuse the file-monitoring patterns introduced in v4
- Fix the missing Source Control icon
  - Restore the icon and verify it renders across light/dark and high-contrast themes
- Fix diff view syntax highlighting
  - Diff comparisons should reuse the same language detection and style scheme as the source editor
- Fix diff view line number alignment
  - Side-by-side compare must keep both panes aligned to the same logical region; the content rows on the left and right must correspond, not drift independently as the user scrolls
- Move the Appearance trigger from the header bar to the main menu
  - Remove the header bar Appearance button and add a menu entry that presents the existing Appearance dialog
  - Remove the now-orphaned accessibility label and review entry
- Fix the Recent Files dialog layout
  - "Clear All" and "Close" must sit at the bottom of the dialog with no dead space below
  - Ensure dialog sizing feels intentional rather than oversized
- Fix the sidebar slide animation regression
  - Restore the slide-in/slide-out behavior; instant open/close is a regression from earlier versions
- Add distinctive dividers in the status bar
  - Separate status bar segments with subtle visual dividers so the boundaries are clear
- Fix the first-open Appearance dialog tile sizing
  - Tiles should render at their natural size on the first open without requiring user interaction to resize
- Refresh help and translations
  - Update the in-app help to cover features through v10
  - Regenerate the translation template and update the Danish catalog

Behavior expectations:
- Source control should feel like a complete light workflow, not a partial one
- Polish regressions should be invisible — fixed without introducing new visual noise
- Discard must be safe and explicit, never accidentally destroying unsaved work
- The app should remain a lightweight GNOME editor with a source control side panel, not a full Git client

Technical expectations:
- Continue to use only the bundled `/app/bin/git` boundary in `app/src/git_process.rs`
- Add only these new Git verbs to the typed Git boundary as required: `log` and `checkout`/`restore` for discard. Update `app/src/git_process.rs` and `app/build-aux/git/README.md` together; the README is the source of truth for the bundled Git surface
- Do not expand the bundled Git build flags in v10; network transport stays disabled and any future re-enable belongs to its own dedicated milestone (currently in the Post-V12 unscheduled backlog)
- Keep all source control UI changes inside the existing source control panel architecture
- Reuse the file-monitoring infrastructure from v4 for the live refresh
- Keep all user-facing strings ready for gettext localization
- Preserve hard limits: no source file over 600 lines, no `unsafe`/`unwrap`/`expect`, no broad permissions

Non-goals for v10:
- No full git log browser
- No branch management UI
- No pull, fetch, or merge UI
- No remote configuration UI
- No git push (held back so the bundled Git build expansion can be reviewed on its own; currently sits in the Post-V12 unscheduled backlog and is not part of the active roadmap)
- No stash, rebase, or cherry-pick tooling
- No conflict resolution UI
- No GitHub or GitLab integration

Implementation guidance:
- Treat v10 as the closing release for the v9 feature set, not a new platform
- Keep recent commits, discard, and list view as small additions to the existing panel
- Treat regression fixes as first-class work, not afterthoughts
- Refresh translations only after all user-visible strings are stable

Deliverable:
Implement a working v10 of the app with the local source control workflows completed (recent commit history, discard, list view, live monitoring), the polish regressions fixed, and updated help and translations, while preserving the app's identity as a lightweight GNOME editor with a source control side panel. Push is intentionally held back and is not in scope for v10.
```

---

# V11 — Split diff polish

> created: 2026-05-02
> updated: 2026-05-02
> status: planned
> priority: high
> type: roadmap-milestone
> implementation: pending

## Purpose

V11 makes Riteed's Compare and Git diff workflow practical enough for daily changed-file review.

This is a focused readability release. It keeps the existing compare model, but makes side-by-side diffs easier to scan, easier to trust, and useful enough that users do not need to open a heavier editor just to understand a local change.

## What V11 adds

* Logical side-by-side diff row alignment
* Placeholder rows for insertions and deletions
* Intra-line highlighting for changed regions within modified lines
* Clearer diff status and hunk navigation behavior
* The same improved diff surface for manual Compare and Git compare

## Why this version matters

V10 made Source Control useful enough that diff readability is now the main friction in daily review. Riteed already has compare entry points and Git-backed diffs, but the current split view is still too hard to read when inserted, deleted, and modified lines drift apart. V11 prioritizes this over broader editing power tools because changed-file review is one of the product's core reasons to exist.

## Prompt for V11

```text
Build v11 of the GNOME desktop application in Rust by polishing the existing split diff and compare workflow.

The goal of v11 is to make Compare and Git compare genuinely practical for daily changed-file review. The app already has manual compare actions, Git-backed compare, hunk navigation, and side-by-side panes. This version should keep that architecture, but make the diff output readable enough that users can quickly understand what changed without opening a heavier editor.

What v11 adds:
- Logical row alignment for side-by-side diffs
- Placeholder rows for insertions and deletions
- Intra-line highlighting for changed regions inside modified lines
- Clearer diff status and hunk navigation behavior
- The same improved diff surface for manual Compare and Git compare

Scope for v11:
- Improve the existing compare implementation rather than creating a separate diff application
- Build a shared logical diff-row model for the left and right panes
- Align both panes by logical diff rows, not just by independent scroll position
- Represent inserted and deleted lines with blank or placeholder rows on the opposite side
- Highlight changed regions inside modified lines, not only whole changed lines
- Introduce or preserve scroll sync so both panes follow the same logical diff-row position
- Keep existing hunk navigation working with the new row model
- Keep source style, high-contrast behavior, and tab-local compare state working
- Apply the same improved diff surface to:
  - Compare With File
  - Compare With Saved Version
  - Compare Pasted Text
  - Git compare from Source Control
- Keep the advanced compare dialog implementation available behind the simpler tab actions

Behavior expectations:
- The app should remain a lightweight GNOME editor
- Diffs should be readable at a glance, with corresponding left/right lines visually aligned
- Insertions and deletions should not make the opposite pane appear to drift
- Modified lines should show the changed region clearly enough to distinguish small edits from full-line replacements
- Hunk navigation should move to the same logical change in both panes
- Manual compare and Git compare should feel like one feature, not two separate implementations

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita + GtkSourceView codebase
- Reuse the current compare and source-control entry points
- Keep diff state tab-local and ephemeral
- Avoid adding a merge engine or conflict-resolution model
- Keep all user-facing strings ready for gettext localization
- Preserve hard limits: 600-line files, no `unsafe`/`unwrap`/`expect`, no broad permissions
- Add tests for row alignment, insertion/deletion placeholders, intra-line changes, hunk navigation, and Git compare reuse

Non-goals for v11:
- No merge editor
- No conflict resolver
- No three-way diff
- No standalone Git client behavior
- No branch, push, pull, stash, rebase, or remote workflow
- No large-file streaming or huge-file viewer

Implementation guidance:
- Treat v11 as a polish pass on the existing compare architecture
- Prefer a small shared diff-row model over ad hoc scroll compensation
- Keep the visual treatment GNOME-native and restrained
- Make the test cases small and explicit so future compare changes do not regress alignment
- Refresh help and translations only if user-visible strings change

Deliverable:
Implement a working v11 of the app where manual Compare and Git compare use a clearer, aligned side-by-side diff surface with placeholder rows, intra-line highlighting, reliable hunk navigation, and preserved GNOME-native behavior.
```

---

# V12 — Editing power tools

> created: 2026-05-02
> updated: 2026-05-02
> status: planned
> priority: medium
> type: roadmap-milestone
> implementation: pending

## Purpose

V12 broadens the everyday editing workflow with the missing power features that complement the existing search, edit, and workspace model.

This is a writing-and-editing release: it makes the editor self-sufficient for serious text work without crossing into IDE territory.

## What V12 adds

* Replace and Replace All
* Find in Files across the open workspace
* Document statistics (word, line, character counts)
* Print support

## Why this version matters

V12 closes practical gaps that have shown up in real use: replace was implied by V3's search but never fully landed, find-in-files leverages the V6 workspace model, and statistics and printing make the app appropriate for documents that leave the editor. V12 is the final scheduled milestone on the roadmap; remaining ideas (spell check, Markdown preview, large-file streaming, and git push) are collected as unscheduled candidates and only promoted to a real version once one of them earns it.

## Prompt for V12

```text
Build v12 of the GNOME desktop application in Rust by extending everyday editing capabilities with replace, project-wide search, document statistics, and printing.

The goal of v12 is to make the editor genuinely capable for serious text work, while keeping the app lightweight and GNOME-native. This version should round out features that complement the existing search and workspace model rather than introduce new architectural pieces.

What v12 adds:
- Replace and Replace All in the search bar
- Find in Files across the open workspace
- Document statistics (word count, line count, character counts with and without spaces)
- Print support via the portal

Scope for v12:
- Add Replace and Replace All
  - Extend the v3 search bar with a replacement field
  - Provide single-replace-and-advance and replace-all actions
  - Treat replace-all as a single undoable operation per document
- Add Find in Files
  - Search across the currently open workspace folder
  - Present results in a side panel or scoped result view, with file and line context
  - Allow opening any result in the editor with the match position selected
  - Reuse the v6 project tree's file enumeration and the v3 search infrastructure
  - Respect hidden-file filtering and skip clearly non-text files
- Add document statistics
  - Show word count, line count, character count with and without spaces, and selection-scoped counts where useful
  - Surface counts in a dedicated dialog or panel; do not crowd the status bar
- Add print support
  - Use the portal-based print flow
  - Print the editor buffer with current font and basic page layout
  - Keep options minimal and predictable

Behavior expectations:
- The app should remain a lightweight GNOME editor
- Replace and Replace All must integrate with the existing search bar, not introduce a parallel UI
- Find in Files must respect the workspace boundary and avoid scanning ignored or hidden directories
- Statistics should be quick to invoke and ignorable when not needed
- Print should feel native and predictable

Technical expectations:
- Extend the existing Rust + GTK4 + Libadwaita + GtkSourceView codebase
- Reuse the v3 search controller for replace
- Reuse the v6 workspace traversal for find-in-files
- Use the print portal rather than direct printer access
- Keep all user-facing strings ready for gettext localization
- Preserve hard limits: 600-line files, no `unsafe`/`unwrap`/`expect`, no broad permissions

Non-goals for v12:
- No regex-based find and replace if it requires non-trivial UI scope (defer if it grows)
- No find-in-files indexing layer; use a streaming scan
- No advanced print preview beyond what the portal provides
- No multi-file replace driven from find-in-files results in this version

Implementation guidance:
- Treat replace as a small extension of the search bar, not a new mode
- Keep find-in-files modest: one workspace, one query, clear results
- Make statistics a single dialog action available from the menu
- Refresh help and translations as part of the same change

Deliverable:
Implement a working v12 of the app with replace and replace-all, find in files across the workspace, document statistics, and print support, while preserving the app's identity as a lightweight GNOME editor.
```

---

# Post-V12 — Unscheduled candidates

> created: 2026-04-27
> updated: 2026-05-02
> status: deferred
> priority: low
> type: roadmap-backlog
> implementation: not scheduled

## Purpose

This section collects ideas that are explicitly **not** part of the current roadmap. They have not earned a version number and may never. They live here so they are not silently forgotten and so future scoping has a single place to look. None of these items should be planned, prompted, or started without first being promoted into a numbered version.

The rule for promotion is unchanged from the rest of the roadmap: a candidate becomes a version only when there is a concrete reason it must ship next, and only when it can fit a single coherent release without dragging in the others.

## Candidate items

* Optional GTK native spell check
* Lightweight Markdown preview for `.md` files
* Git push for the current branch and configured remote
* Bundled Git build expanded to support network transport (re-enabling curl/openssl/expat with documented justification)
* Authentication path for push using GNOME-appropriate credential storage
* Clear surfacing of authentication, network, and rejection errors in the source control side panel
* Initial chunk streaming for very large files

## Why each candidate is deferred

**Spell check** — depends on whether GtkSourceView's native spelling support is available on the bundled platform without sandbox or runtime gaps. Worth holding until there is a forcing reason.

**Markdown preview** — useful but distinct from the editor's identity as a plain-text/code editor; the existing diff/compare infrastructure already provides side-by-side viewing for the cases that matter most today.

**Git push (and the three items it requires)** — push is treated as a single coupled bundle: it cannot ship without re-enabling network transport in the bundled Git build (re-justifying NO_CURL/NO_OPENSSL/NO_EXPAT), without a credential storage path, and without explicit error surfacing. The bundled Git network expansion is security-sensitive enough that it should only be opened when push itself is the next thing to ship — not preemptively.

**Initial chunk streaming for very large files** — useful and worth investigating, but it likely needs its own document/viewer mode rather than normal GtkSourceView loading. It should be promoted only when the milestone can focus on large-file architecture, including clear behavior for search, syntax highlighting, minimap, compare, autosave, and session restore.

## Promotion rules

If any of these items is promoted to a real version later, the promoting change must:

* Pick a version number (V13, V14, …) and create a full milestone section with Purpose, What it adds, Why this version matters, and Prompt — matching the structure of V1–V12
* Pull only the items that genuinely belong together in one release; do not promote the whole list at once unless that is the actual decision
* Re-justify any bundled Git or Flatpak manifest expansion as part of the same change, not as a follow-up
* Update the header `final_scheduled_version` and the Summary section accordingly

---

# Summary of the full progression

V1–V10 are complete as of 2026-04-26. V10 closed V9's deferred local items (recent commit history, discard) and cleared the polish regressions accumulated since V8. V11 is a focused split-diff polish milestone. V12 is the final scheduled release: an editing-power-tools milestone (replace, find in files, statistics, print). Anything beyond V12 — spell check, Markdown preview, large-file streaming, git push and the bundled Git network expansion it requires — sits in the "Post-V12 — Unscheduled candidates" section and only earns a version number once one of them has a concrete reason to ship next.

## V1

Build the base app correctly.

## V2

Make the app usable with multiple documents.

## V3

Polish the core editing workflow.

## V4

Upgrade the editor to support code-friendly features.

## V5

Add precise editor controls and file-format awareness.

## V6

Add lightweight workspace navigation.

## V7

Add comparison and advanced split editing workflows.

## V8

Improve polish, accessibility, and editing safety.

## V9

Add lightweight Git awareness and source control workflow support.

## V10

Complete the source control workflow and clear accumulated polish regressions.

## V11

Polish split diff so manual Compare and Git compare become practical for daily changed-file review. The side-by-side panes should align by logical diff rows, show placeholders for insertions and deletions, and highlight changed regions inside modified lines.

## V12

Add editing power tools: replace, find in files, statistics, and printing. Final scheduled milestone.

## Post-V12

Unscheduled backlog. Holds spell check, Markdown preview, initial large-file streaming, and the git push bundle (push action + bundled Git network expansion + credential storage + error surfacing). Items only promote to a numbered version when one has a concrete reason to ship next.

---

# The shape of the product over time

By the end of the roadmap, the app becomes:

* a GNOME-native Rust editor
* lightweight but capable
* suitable for plain text, config files, markdown, and light code editing
* stronger in editing, comparison, and repository awareness than a basic notepad
* intentionally still smaller and simpler than a full IDE or dedicated Git client

---

# Recommended mental model for the agent

Each version should preserve these principles:

* Keep the code modular
* Centralize document state
* Reuse workflows instead of duplicating logic
* Prefer GNOME conventions over custom UI inventions
* Add only the structure needed for the next step
* Keep Git support lightweight and editor-centered
* Stay out of IDE territory

---
