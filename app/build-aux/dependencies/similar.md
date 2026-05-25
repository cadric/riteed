# similar

- Crate: `similar = "=3.1.1"` with `inline` and `unicode` features.
- Purpose: provide the existing line-diff algorithm for Riteed Compare, plus V11 intra-line grapheme/word ranges inside modified rows.
- Justification: V11 changes compare presentation, not the diff algorithm. Enabling `similar`'s built-in inline and Unicode tools avoids adding a new diff crate or hand-rolling Unicode-sensitive character ranges.
- Feature review: `inline` keeps using the existing text diff machinery; `unicode` adds `unicode-segmentation` so modified-line ranges can follow grapheme boundaries. `cargo tree -e features -i unicode-segmentation` shows it is pulled only through `similar`'s `unicode` feature.
- Cargo source review: `unicode-segmentation`, `bstr`, and `serde` are locked in `app/build-aux/cargo/cargo-sources.json` because Cargo resolves `similar`'s feature graph from local Flatpak sources. `cargo tree -e normal -i bstr --target all` and `cargo tree -e normal -i serde --target all` print no active runtime dependency path for the current Riteed feature set.
- Security review: this adds no network capability, no sandbox expansion, no file-system access, and no command execution surface.
- Supply-chain review: pinned exact crate version, committed `Cargo.lock`, committed Flatpak Cargo source manifest, and no git dependencies.
- License review: `similar` is Apache-2.0; `unicode-segmentation`, `bstr`, and `serde` use permissive MIT/Apache-compatible licenses. The Flatpak build installs the active `similar` license text alongside Riteed's own license notes.
