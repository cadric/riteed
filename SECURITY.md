# Security Policy

## Supported Versions

Riteed is an early public beta. Security fixes are handled for the current
released beta line and current `main`.

Older beta releases may receive a fix when the issue is severe and a small,
low-risk backport is practical. Otherwise, reporters should expect the fix to
land in the next beta release.

## Reporting a Vulnerability

Use GitHub private vulnerability reporting for this repository:

https://github.com/cadric/riteed/security/advisories/new

Please include:

- the affected Riteed version or commit;
- whether the issue affects the Flatpak build, a native build, or both;
- the GNOME, GTK, and Flatpak versions when relevant;
- reproduction steps and the smallest safe input file or repository layout you
  can share;
- whether the issue appears to expose data, escape the sandbox, write outside
  the selected file or project, or execute unexpected code.

Do not open a public issue for a suspected security vulnerability before it has
been triaged.

## Response Expectations

Riteed is maintained by a solo maintainer. Security reports are reviewed on a
best-effort basis, typically within 7 days.

If the report is accepted, the fix will normally land on `main` first and then
ship in the next beta release. Public disclosure should wait until a fixed beta
release or a coordinated advisory is available.

## Release Signing

Riteed's beta Flatpak remote is signed with the dedicated Riteed Flatpak Beta
key. The public key is committed at
`app/build-aux/flatpak/riteed-beta-public.asc`.

Fingerprint:

```text
1A04 CECD 3576 716F F309  0D27 5D2C 311E 81B8 5DC6
```

The GitHub Pages beta remote is a pre-Flathub distribution channel. Treat a
fingerprint mismatch as suspicious and report it privately.

## Security Scope

The following are in scope:

- Flatpak sandbox or portal misuse that allows unintended host access;
- arbitrary file read or write outside explicit user-selected files or folders;
- crashes or parser behavior for untrusted documents that plausibly lead to data
  exposure, code execution, or persistent compromise;
- misuse of the bundled sandbox-local Git path that crosses Riteed's local-only
  Source Control boundary;
- secret exposure through logs, diagnostics, settings, recent files, or release
  artifacts.

The following are normally out of scope unless they have a concrete security
impact:

- ordinary UI bugs, missing features, or usability issues;
- crashes with no data exposure, sandbox impact, persistence, or code execution;
- feature requests for networking, LSP, terminal, push, pull, branch management,
  or remote Git features that Riteed does not currently implement;
- issues that require disabling the Flatpak sandbox or running modified local
  builds outside the documented threat model.
