#!/usr/bin/env bash
set -euo pipefail

root="${RITEED_GIT_STRESS_ROOT:-$(pwd)/stress/git-repos/generated}"
count="${RITEED_GIT_STRESS_COUNT:-5000}"

git_init() {
  git -C "$1" -c init.defaultBranch=main init -q
}

git_identity() {
  git -C "$1" config user.name "Riteed Stress"
  git -C "$1" config user.email "riteed-stress@example.invalid"
}

commit_all() {
  git -C "$1" add -A
  git -C "$1" commit -q -m "$2"
}

make_files() {
  repo="$1"
  prefix="$2"
  text="$3"
  i=1
  while [ "$i" -le "$count" ]; do
    mkdir -p "$repo/${prefix}/$(printf '%04d' "$((i / 100))")"
    printf '%s %s\n' "$text" "$i" > "$repo/${prefix}/$(printf '%04d' "$((i / 100))")/file-$(printf '%05d' "$i").txt"
    i=$((i + 1))
  done
}

reset_root() {
  rm -rf "$root"
  mkdir -p "$root"
}

many_untracked() {
  repo="$root/many-untracked"
  mkdir -p "$repo"
  git_init "$repo"
  git_identity "$repo"
  printf 'tracked\n' > "$repo/tracked.txt"
  commit_all "$repo" "initial"
  make_files "$repo" "untracked" "untracked"
}

many_modified() {
  repo="$root/many-modified"
  mkdir -p "$repo"
  git_init "$repo"
  git_identity "$repo"
  make_files "$repo" "tracked" "before"
  commit_all "$repo" "initial"
  make_files "$repo" "tracked" "after"
}

conflicted() {
  repo="$root/conflicted"
  mkdir -p "$repo"
  git_init "$repo"
  git_identity "$repo"
  printf 'base\n' > "$repo/conflict.txt"
  commit_all "$repo" "base"
  git -C "$repo" checkout -q -b side
  printf 'side\n' > "$repo/conflict.txt"
  commit_all "$repo" "side"
  git -C "$repo" checkout -q main
  printf 'main\n' > "$repo/conflict.txt"
  commit_all "$repo" "main"
  git -C "$repo" merge side >/dev/null 2>&1 || true
}

non_utf8_paths() {
  repo="$root/non-utf8-paths"
  mkdir -p "$repo"
  git_init "$repo"
  git_identity "$repo"
  REPO="$repo" python3 - <<'PY'
import os
root = os.fsencode(os.environ["REPO"])
path = os.path.join(root, b"bad-\xff.txt")
fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o644)
with os.fdopen(fd, "wb") as handle:
    handle.write(b"non-utf8 display\n")
PY
  git -C "$repo" add -A
}

huge_status() {
  repo="$root/huge-status"
  mkdir -p "$repo"
  git_init "$repo"
  git_identity "$repo"
  make_files "$repo" "tracked" "before"
  commit_all "$repo" "initial"
  make_files "$repo" "tracked" "after"
  make_files "$repo" "untracked" "untracked"
}

submodule_and_symlink() {
  repo="$root/submodule-and-symlink"
  nested="$root/submodule-source"
  mkdir -p "$repo" "$nested"
  git_init "$nested"
  git_identity "$nested"
  printf 'nested\n' > "$nested/README.txt"
  commit_all "$nested" "nested"
  git_init "$repo"
  git_identity "$repo"
  git -C "$repo" -c protocol.file.allow=always submodule add "$nested" module >/dev/null
  ln -s module/README.txt "$repo/module-link.txt"
  commit_all "$repo" "submodule and symlink"
}

missing_identity() {
  repo="$root/missing-identity"
  mkdir -p "$repo"
  git_init "$repo"
  printf 'needs identity\n' > "$repo/identity.txt"
  git -C "$repo" add identity.txt
}

index_lock_present() {
  repo="$root/index-lock-present"
  mkdir -p "$repo"
  git_init "$repo"
  git_identity "$repo"
  printf 'tracked\n' > "$repo/tracked.txt"
  commit_all "$repo" "initial"
  printf 'lock\n' > "$repo/.git/index.lock"
}

reset_root
many_untracked
many_modified
conflicted
non_utf8_paths
huge_status
submodule_and_symlink
missing_identity
index_lock_present
