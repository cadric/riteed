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

## Purpose

V6 introduces lightweight workspace behavior.

The app is still not a full IDE, but it becomes much better for working across sets of related files.

## What V6 adds

* Open folder support
* Project tree view
* Split-pane layout
* Better multi-file navigation

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

## Purpose

V7 adds powerful comparison workflows.

The goal is to support diff and side-by-side inspection without turning the app into a merge tool or VCS client.

## What V7 adds

* Diff support
* Side-by-side compare view
* Compare current buffer with saved/on-disk version
* Compare two files
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

# Summary of the full progression

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

---

# The shape of the product over time

By the end of the roadmap, the app becomes:

* a GNOME-native Rust editor
* lightweight but capable
* suitable for plain text, config files, markdown, and light code editing
* stronger in editing and comparison than a basic notepad
* intentionally still smaller and simpler than a full IDE

---

# Recommended mental model for the agent

Each version should preserve these principles:

* Keep the code modular
* Centralize document state
* Reuse workflows instead of duplicating logic
* Prefer GNOME conventions over custom UI inventions
* Add only the structure needed for the next step
* Stay out of IDE territory

---
