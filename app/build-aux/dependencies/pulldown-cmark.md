# pulldown-cmark

- Crate: `pulldown-cmark = "=0.13.3"` with `default-features = false`.
- Purpose: parse CommonMark source into events for the native Markdown preview.
- Justification: V1 needs CommonMark behavior and source ranges without adding a browser, WebKit, DOM renderer, JavaScript runtime, network access, or hand-rolled Markdown parsing.
- Feature review: `Options::empty()` is the production parser mode. Riteed does not enable tables, task lists, footnotes, strikethrough, math, GFM, heading attributes, wikilinks, definition lists, subscript, superscript, smart punctuation, or the crate's HTML renderer feature.
- Cargo source review: `cargo tree -e normal -p pulldown-cmark --target all` shows active normal dependencies limited to `bitflags`/`memchr` plus `unicase`, all locked through `app/build-aux/cargo/cargo-sources.json`.
- Security review: the crate is used only as an in-process parser. Preview output is converted into a Riteed AST and rendered with GTK `TextBuffer` tags; it performs no file access, network access, command execution, HTML output, or resource loading.
- Supply-chain review: pinned exact crate version, committed `Cargo.lock`, committed Flatpak Cargo source manifest, and no git dependencies.
- License review: `pulldown-cmark` is MIT; `unicase` is MIT OR Apache-2.0. The Flatpak build installs their license texts alongside Riteed's own license notes.
