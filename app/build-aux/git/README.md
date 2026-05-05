# Bundled Git Verification

Riteed bundles Git inside the Flatpak sandbox as `/app/bin/git` for the V9 source control sidebar. The app never calls host Git or `flatpak-spawn`.

The public key in `kernelorg-checksum-autosigner.asc` is the Kernel.org checksum autosigner key used for `sha256sums.asc`:

`B886 8C80 BA62 A1FF FAF5 FDA9 632D 3A06 589D A6B1`

Provenance: `https://www.kernel.org/signature.html` documents that `sha256sums.asc` is signed by Kernel.org's dedicated checksum autosigner. The vendored key was retrieved with `gpg --locate-keys autosigner@kernel.org`, which uses the Kernel.org WKD over TLS.

The Flatpak module imports the vendored key with an isolated `GNUPGHOME`, verifies `sha256sums.asc`, then verifies the `git-2.54.0.tar.xz` checksum from that signed file before extraction. Kernel.org documents this as a mirror-integrity check, not a replacement for developer release signatures.

The module builds Git for local plumbing only, disables debuginfo extraction for the Git module so Flatpak-builder does not try to rewrite hardlinked Git aliases in the read-only staging tree, and explicitly strips `/app/bin/git` before caching the module.

Riteed may invoke only these Git operations through `src/git_process.rs`: `rev-parse`, `status`, `config --get`, `check-attr`, `cat-file blob`, `hash-object`, `update-index`, `ls-tree`, `commit`, `log`, and `restore --worktree`. This list is the source of truth for the Flatpak Git payload; adding another Git command must update this file and re-justify the bundled Git surface in the same change.

The module intentionally disables curl, expat, Perl, Python, Tcl/Tk, and gettext support, then removes unused helper entrypoints from both `/app/bin` and `/app/libexec/git-core`. Helper cleanup uses `rm -f` and tolerates absent paths because Git build flags can suppress different aliases across releases. Re-enabling network, scripting, GUI, or remote-helper features leaves the local-plumbing-only contract and requires explicit review.

The Flatpak build installs Git's top-level GPL-2.0-only license text plus the LGPL, BSD, and MIT-compatible license files for bundled Git subcomponents under `/app/share/licenses/io.github.cadric.Riteed/git/`.

If a future Git release fails signature verification with the vendored key, audit and rotate this key file with verified provenance. Do not bypass `gpg --verify` to unblock a build.
