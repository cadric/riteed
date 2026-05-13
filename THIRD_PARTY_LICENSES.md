# Third-Party License Notes

Riteed's own source code and repository policy/tooling are licensed under MIT.
The root `LICENSE` file covers that code.

This file summarizes the third-party license surfaces that matter for the
current Flatpak-oriented build. It is not a replacement for the upstream license
texts shipped in `app/vendor/` and installed by the Flatpak build.

## Cargo Dependencies

The Rust dependencies used by Riteed are pinned in `app/Cargo.lock` and vendored
under `app/vendor/` for deterministic offline Flatpak builds.

Important runtime crates:

- `gtk4`, `libadwaita`, `sourceview5`, `gio`, `glib`, `gdk4`, `pango`, and the
  related gtk-rs crates are MIT-licensed bindings.
- `gettext-rs` and `gettext-sys` are MIT-licensed Rust crates. Riteed enables
  `gettext-rs/gettext-system`, so the build uses system gettext instead of
  statically building the vendored GNU gettext fallback.
- `similar` is Apache-2.0 licensed and provides the compare/diff engine.
- `pulldown-cmark` is MIT-licensed and provides CommonMark event parsing for
  the native Markdown preview.
- `yaml-rust2` is MIT OR Apache-2.0 licensed and parses optional Markdown
  frontmatter. Its active support crates are permissively licensed:
  `arraydeque` (MIT/Apache-2.0), `hashlink` and `hashbrown` (MIT OR
  Apache-2.0), `foldhash` (Zlib), and `unicase` (MIT OR Apache-2.0).
- Most support crates are MIT, Apache-2.0, or MIT OR Apache-2.0. Their exact
  upstream license files remain in `app/vendor/*/`.

## GNOME Platform Libraries

Riteed links against GTK, libadwaita, GtkSourceView, GLib/GIO, Pango, Cairo, and
related GNOME stack libraries provided by `org.gnome.Platform//50`. Those
libraries are runtime dependencies supplied by the Flatpak platform and remain
under their upstream GNOME licenses.

## Bundled Git

The Flatpak manifest builds and installs a trimmed `/app/bin/git` from the
official Git 2.54.0 source tarball. Riteed uses it only for local Source Control
operations inside the sandbox; it does not call host Git or `flatpak-spawn`.

Git's top-level license is GPL-2.0-only. The Git source tarball also includes
some files under LGPL-2.1-or-later, BSD-3-Clause, and MIT-compatible terms. The
Flatpak build installs the relevant license texts under:

```text
/app/share/licenses/io.github.cadric.Riteed/git/
```

The AppStream `project_license` reflects Riteed's MIT code plus the bundled Git
license surface in the distributed Flatpak package.
