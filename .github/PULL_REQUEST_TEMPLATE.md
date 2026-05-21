## Summary

-

## Validation

- [ ] `python3 -m tools.policy_check --root app --strict`
- [ ] `python3 -m tools.coverage_check --root app`
- [ ] Other relevant checks:

## Review Notes

- Linked issue:
- `CHANGELOG.md` updated or not needed:
- Docs/help/README impact:
- Flatpak manifest, bundled sources, or release artifact impact:
- Security, sandbox, portal, or GitHub workflow impact:

## Maintainer Checklist

- [ ] The change stays within the native GNOME/libadwaita/Flatpak-first contract.
- [ ] User-visible strings are localizable or no new user-visible strings were added.
- [ ] Any `app/fuzz` or `app/Cargo.lock` dependency changes were checked against the other lockfile, especially `gtk4-sys`.
