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
summary metadata, verifies that the imported secret key matches the committed
public key, and exports the committed binary public key into `GPGKey=` fields.

## Key Rotation And Recovery

Start planned rotation at least 3 months before the key expires.

1. Generate the replacement beta key offline and record its full fingerprint,
   expiry date, and user ID in this file.
2. Replace `riteed-beta-public.asc` with the new armored public key and update
   the `FLATPAK_GPG_KEY_ID` environment secret to the full new fingerprint.
3. Replace `FLATPAK_GPG_PRIVATE_KEY` and `FLATPAK_GPG_PASSPHRASE` in the
   protected `flatpak-beta-signing` environment.
4. Publish the next beta from a normal version tag. The publish workflow must
   fail before signing if the imported secret key does not match
   `riteed-beta-public.asc`.
5. Keep the old public key and release notes available until the previous beta
   is no longer a supported update source.

For revocation or suspected compromise, disable the `flatpak-beta-signing`
environment first so no further signed updates can publish. Commit the revoked
public key material and a replacement `riteed-beta-public.asc`, rotate all three
signing secrets, and publish only from an explicitly reviewed recovery tag.

Emergency cutover uses the same key-pin check as planned rotation. If the Pages
remote must move backwards or switch keys because a bad beta already shipped,
use the emergency rollback input on the publish workflow, document the target
version/ref in the release notes, and preserve the previous Pages artifact for
post-incident comparison. Emergency rollback publishing routes through the
separate `flatpak-beta-rollback` GitHub environment; its required reviewer
identity is policy-pinned and verified by the live `ruleset-governance` job.

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
