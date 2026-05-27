# sourceview5

- Crate: `sourceview5 = "=0.11.0"`
- Purpose: provide GNOME-native editor primitives for line numbers, search, replace, and better long-document behavior in Riteed v3.
- Justification: v3 adds line numbers and in-document search/replace. Implementing those on top of `GtkTextView` would require custom gutter and search infrastructure that is larger, less native, and harder to maintain than using GtkSourceView.
- Maintenance review: `sourceview5` is part of the gtk-rs GNOME bindings ecosystem and tracks GtkSourceView 5. The crate line used here matches the existing pinned `gtk4 0.11.x` stack.
- Local patch note: Riteed carries a narrow `sourceview5` crate patch under `build-aux/cargo-patches/sourceview5` for `set_candidate_encodings(...)` and main-context async callbacks, so the app can implement deterministic “Reopen With Encoding…” without introducing `unsafe` in `app/src`.
- Security review: this adds no network capability, no sandbox expansion, and no command execution surface. It links against the system GtkSourceView library that is already distributed as part of the GNOME platform stack.
- Patch manifest: `../cargo-patches/sourceview5/patch-manifest.json` pins the official `sourceview5-0.11.0.crate` checksum, the reviewed upstream archive, the allowed local changed files, the diff checksum, and the unsafe/FFI baseline.
- Unsafe/FFI baseline: the reviewed sourceview5 patch tree currently has 1853 matches for `unsafe`, `extern "C"`, or `transmute` under `src/**/*.rs`; validator drift checks use the same `rg -o ... | wc -l` pattern recorded in the patch manifest.
- Supply-chain review: pinned exact crate version, committed `Cargo.lock`, committed Flatpak Cargo source manifest, reviewed local patch manifest, and no git dependencies.
- License review: gtk-rs bindings are MIT-licensed and remain compatible with the repository MIT license and the GNOME stack already used by Riteed.
