#!/usr/bin/env python3
"""Generate the GitHub Pages files for the Riteed beta Flatpak remote."""

from __future__ import annotations

import argparse
import html
from pathlib import Path


APP_ID = "io.github.cadric.Riteed"
APP_NAME = "Riteed"
REMOTE_NAME = "riteed-beta"
BRANCH = "beta"
HOMEPAGE = "https://github.com/cadric/riteed"
RUNTIME_REPO = "https://dl.flathub.org/repo/flathub.flatpakrepo"


def write(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def page(title: str, body: str) -> str:
    return f"""<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{html.escape(title)}</title>
  <style>
    :root {{ color-scheme: light dark; font-family: system-ui, sans-serif; }}
    body {{ margin: 0; padding: 2rem; line-height: 1.5; max-width: 52rem; }}
    code, pre {{ font-family: ui-monospace, monospace; }}
    pre {{ overflow-x: auto; padding: 1rem; border: 1px solid currentColor; }}
    a {{ color: LinkText; }}
  </style>
</head>
<body>
{body}
</body>
</html>
"""


def install_block(ref_url: str) -> str:
    return f"""<pre><code>flatpak install --user {html.escape(ref_url)}
flatpak update --user {APP_ID}</code></pre>"""


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--site-dir", required=True)
    parser.add_argument("--base-url", required=True)
    parser.add_argument("--repo-url", required=True)
    parser.add_argument("--gpg-key-base64", required=True)
    parser.add_argument("--fingerprint", required=True)
    parser.add_argument("--version", required=True)
    args = parser.parse_args()

    site_dir = Path(args.site_dir)
    flatpak_dir = site_dir / "flatpak"
    ref_url = f"{args.base_url.rstrip('/')}/{APP_ID}.flatpakref"
    repo_file_url = f"{args.base_url.rstrip('/')}/{REMOTE_NAME}.flatpakrepo"
    fingerprint = args.fingerprint.upper()
    spaced_fingerprint = " ".join(
        fingerprint[index : index + 4] for index in range(0, len(fingerprint), 4)
    )

    write(site_dir / ".nojekyll", "")
    write(
        site_dir / "index.html",
        page(
            "Riteed Beta Flatpak",
            f"""<h1>Riteed Beta Flatpak</h1>
<p>{APP_NAME} beta Flatpak updates are published from this GitHub Pages
repository until the app is ready for Flathub.</p>
{install_block(ref_url)}
<p><a href="flatpak/">Flatpak repository details</a></p>
""",
        ),
    )
    write(
        flatpak_dir / "index.html",
        page(
            "Riteed Flatpak Repository",
            f"""<h1>Riteed Flatpak Repository</h1>
<p>Remote: <code>{REMOTE_NAME}</code><br>
Branch: <code>{BRANCH}</code><br>
Version: <code>{html.escape(args.version)}</code></p>
{install_block(ref_url)}
<p>Explicit remote setup:</p>
<pre><code>flatpak remote-add --user --if-not-exists {REMOTE_NAME} {html.escape(repo_file_url)}
flatpak install --user {REMOTE_NAME} {APP_ID}//{BRANCH}</code></pre>
<p>Signing fingerprint:</p>
<pre><code>{html.escape(spaced_fingerprint)}</code></pre>
""",
        ),
    )

    write(
        flatpak_dir / f"{REMOTE_NAME}.flatpakrepo",
        f"""[Flatpak Repo]
Title=Riteed Beta
Url={args.repo_url}
Homepage={HOMEPAGE}
Comment=Riteed beta Flatpak repository
Description=Beta builds of Riteed, hosted on GitHub Pages until Flathub submission.
DefaultBranch={BRANCH}
GPGKey={args.gpg_key_base64}
""",
    )
    write(
        flatpak_dir / f"{APP_ID}.flatpakref",
        f"""[Flatpak Ref]
Title=Riteed
Name={APP_ID}
Branch={BRANCH}
Url={args.repo_url}
IsRuntime=false
RuntimeRepo={RUNTIME_REPO}
SuggestRemoteName={REMOTE_NAME}
GPGKey={args.gpg_key_base64}
""",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
