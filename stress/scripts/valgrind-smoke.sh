#!/usr/bin/env bash
set -euo pipefail

app_id="io.github.cadric.Riteed"
manifest="app/build-aux/${app_id}.yml"
repo_root="$(pwd)"
suppression_file="${repo_root}/stress/valgrind/riteed-flatpak.supp"

flatpak-builder --user --install --force-clean app/build-dir "$manifest"
runtime_remote="$(flatpak remotes --user --columns=name | awk '
  $1 == "flathub" {
    print $1;
    found = 1;
    exit
  }
  $1 == "flathub-user" {
    fallback = $1
  }
  END {
    if (!found && fallback != "") {
      print fallback
    }
  }
')"

if [[ -n "$runtime_remote" ]]; then
  flatpak install --user --include-sdk --include-debug -y "$runtime_remote" \
    org.gnome.Platform/x86_64/50 org.gnome.Sdk/x86_64/50
fi

flatpak run --user --devel --filesystem="${repo_root}:ro" --command=sh "$app_id" -lc \
  'timeout "${RITEED_VALGRIND_TIMEOUT:-120}" valgrind --leak-check=full --errors-for-leak-kinds=definite --error-exitcode=37 --suppressions="$1" /app/bin/riteed --gapplication-service' \
  sh "$suppression_file"
