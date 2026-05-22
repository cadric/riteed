# Riteed Beta Flatpak Remote

Riteed publishes beta Flatpak updates from a GitHub Pages hosted Flatpak
repository until the app is ready for Flathub.

- Remote name: `riteed-beta`
- Flatpak branch: `beta`
- Remote file: `https://cadric.github.io/riteed/flatpak/riteed-beta.flatpakrepo`
- Ref file: `https://cadric.github.io/riteed/flatpak/io.github.cadric.Riteed.flatpakref`
- Repository URL: `https://cadric.github.io/riteed/flatpak/repo/`
- Runtime repository: `https://dl.flathub.org/repo/flathub.flatpakrepo`

## Signing Key

The committed `riteed-beta-public.asc` key is used only for the beta Flatpak
remote.

- Full fingerprint: `1A04 CECD 3576 716F F309  0D27 5D2C 311E 81B8 5DC6`
- Key ID secret value: `1A04CECD3576716FF3090D275D2C311E81B85DC6`
- Expires: `2028-05-21`
- User ID: `Riteed Flatpak Beta <cadric@users.noreply.github.com>`

GitHub Actions imports the private key only inside the protected
`flatpak-beta-signing` environment, signs both OSTree commits and repository
summary metadata, and exports the binary public key into `GPGKey=` fields.

Key rotation procedure: TBD. Start rotation at least 3 months before expiry and
document the client migration path before replacing this key.

## Install

```bash
flatpak install --user https://cadric.github.io/riteed/flatpak/io.github.cadric.Riteed.flatpakref
```

Explicit remote form:

```bash
flatpak remote-add --user --if-not-exists riteed-beta https://cadric.github.io/riteed/flatpak/riteed-beta.flatpakrepo
flatpak install --user riteed-beta io.github.cadric.Riteed//beta
```

Update:

```bash
flatpak update --user io.github.cadric.Riteed
```

## Generated Files

`generate_site.py` writes the GitHub Pages artifact under `site/`. That
directory is generated output and must not be committed.
