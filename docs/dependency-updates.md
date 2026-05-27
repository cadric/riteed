# Dependency Updates

Riteed accepts ordinary Rust dependency PRs from Dependabot, but GTK/GNOME
binding updates must be reviewed as a stack. The `gtk4 0.11.3` incident showed
that a safe binding can fail to compile when its matching `*-sys` crate stays on
an older patch release.

## Dependabot Flow

- General Cargo updates land through the `app-cargo` Dependabot group.
- GTK/GNOME binding updates land through the `gtk-rs-stack` group and must be
  reviewed manually before merge.
- Dependabot security updates may bypass grouping. The dependency preflight is
  the backstop and must stay required in CI.

## GTK Stack Update Checklist

1. Fetch the Dependabot branch locally.
2. Update the GTK/GNOME binding stack together in `app/`; do not merge a PR that
   changes only one side of a safe/sys binding pair.
3. Keep direct GTK-stack dependencies exact-pinned in `app/Cargo.toml`,
   including `sourceview5` and `glib-build-tools`.
4. Sync `app/fuzz/Cargo.lock` so any GTK-stack crate present there exactly
   matches `app/Cargo.lock`.
5. Regenerate Flatpak cargo sources:

   ```sh
   tmpdir="$(mktemp -d)"
   python3 -m venv "$tmpdir/venv"
   "$tmpdir/venv/bin/python" -m pip install flatpak-cargo-generator==0.1.3
   "$tmpdir/venv/bin/flatpak-cargo-generator" \
     app/Cargo.lock \
     -o app/build-aux/cargo/cargo-sources.json
   rm -rf "$tmpdir"
   ```

6. Run dependency and app validation:

   ```sh
   scripts/dependency-preflight --root app
   python3 -m tools.policy_check --root app --strict
   python3 -m tools.coverage_check --root app
   ```

7. Run at least one Flatpak build/smoke before merging a GTK-stack update.

Changing policy target versions in the same PR is not self-approval. Treat that
as a GTK-stack update and review it against the GNOME platform, Flatpak runtime,
and binding release notes before merge.

## Preflight Contract

- Exact full-version matching is required for binding pairs that release
  lockstep in this repo: `gtk4/gtk4-sys`, `gdk4/gdk4-sys`, `gsk4/gsk4-sys`,
  `libadwaita/libadwaita-sys`, `sourceview5/sourceview5-sys`,
  `gdk-pixbuf/gdk-pixbuf-sys`, `cairo-rs/cairo-sys-rs`, and
  `graphene-rs/graphene-sys`.
- GLib-family crates such as `glib`, `gio`, and `pango` do not always publish
  safe/sys crates with the same patch number. They are still grouped by
  Dependabot and must match exactly between `app/Cargo.lock` and
  `app/fuzz/Cargo.lock` when present in both.
- Direct GTK-stack manifest entries must be top-level exact pins. Workspace,
  caret, tilde, or target-only declarations are rejected by preflight.
- A GTK-stack crate present only in `app/Cargo.lock` is allowed. A GTK-stack
  crate present only in `app/fuzz/Cargo.lock` is a preflight failure.
