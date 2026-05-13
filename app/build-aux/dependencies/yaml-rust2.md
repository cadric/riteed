# yaml-rust2

- Crate: `yaml-rust2 = "=0.11.0"` with `default-features = false`.
- Purpose: parse optional YAML frontmatter at the start of Markdown documents.
- Justification: V1 needs tolerant YAML frontmatter diagnostics while keeping Markdown body rendering independent from metadata parsing.
- Feature review: default features are disabled so the optional encoding feature is not active. Riteed only calls `YamlLoader::load_from_str` on the frontmatter slice and never executes templates, Liquid, shortcodes, or external includes.
- Vendor review: `cargo tree -e normal -p yaml-rust2 --target all` shows active normal dependencies `arraydeque`, `hashlink`, `hashbrown 0.16.1`, and `foldhash`.
- Security review: parsing runs on already-open document text, performs no file access, no network access, no command execution, and does not expand or fetch referenced resources.
- Supply-chain review: pinned exact crate version, committed `Cargo.lock`, committed vendored dependencies, and no git dependencies.
- License review: `yaml-rust2`, `hashlink`, and `hashbrown` are MIT OR Apache-2.0; `arraydeque` is MIT/Apache-2.0; `foldhash` is Zlib. The Flatpak build installs their license texts alongside Riteed's own license notes.
