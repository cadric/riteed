# sourceview5

- Crate: `sourceview5 = "=0.11.0"`
- Purpose: provide GNOME-native editor primitives for line numbers, search, replace, and better long-document behavior in Riteed v3.
- Justification: v3 adds line numbers and in-document search/replace. Implementing those on top of `GtkTextView` would require custom gutter and search infrastructure that is larger, less native, and harder to maintain than using GtkSourceView.
- Maintenance review: `sourceview5` is part of the gtk-rs GNOME bindings ecosystem and tracks GtkSourceView 5. The crate line used here matches the existing pinned `gtk4 0.11.x` stack.
- Security review: this adds no network capability, no sandbox expansion, and no command execution surface. It links against the system GtkSourceView library that is already distributed as part of the GNOME platform stack.
- Supply-chain review: pinned exact crate version, committed `Cargo.lock`, committed vendored dependencies, and no git dependencies.
- License review: gtk-rs bindings are MIT-licensed and remain compatible with the repository MIT license and the GNOME stack already used by Riteed.
