#!/usr/bin/env bash
set -euo pipefail

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT

schema_dir="$tmpdir/schemas"
config_dir="$tmpdir/config"
mkdir -p "$schema_dir" "$config_dir"

write_old_schema() {
  cat >"$schema_dir/io.github.cadric.Riteed.gschema.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<schemalist>
  <schema id="io.github.cadric.Riteed" path="/io/github/cadric/Riteed/">
    <key name="theme" type="s">
      <default>'system'</default>
      <summary>Theme Preference</summary>
      <description>Controls whether Riteed uses the system appearance, a light appearance, or a dark appearance.</description>
    </key>
    <key name="source-control-view-mode" type="s">
      <default>'tree'</default>
      <summary>Source Control View Mode</summary>
      <description>Controls whether the source control changes view uses a folder tree or a flat list.</description>
    </key>
  </schema>
</schemalist>
XML
}

write_new_schema() {
  cat >"$schema_dir/io.github.cadric.Riteed.gschema.xml" <<'XML'
<?xml version="1.0" encoding="UTF-8"?>
<schemalist>
  <enum id="io.github.cadric.Riteed.ThemePreference">
    <value nick="system" value="0"/>
    <value nick="light" value="1"/>
    <value nick="dark" value="2"/>
  </enum>
  <enum id="io.github.cadric.Riteed.SourceControlViewMode">
    <value nick="tree" value="0"/>
    <value nick="list" value="1"/>
  </enum>
  <schema id="io.github.cadric.Riteed" path="/io/github/cadric/Riteed/">
    <key name="theme" enum="io.github.cadric.Riteed.ThemePreference">
      <default>'system'</default>
      <summary>Theme Preference</summary>
      <description>Controls whether Riteed uses the system appearance, a light appearance, or a dark appearance.</description>
    </key>
    <key name="source-control-view-mode" enum="io.github.cadric.Riteed.SourceControlViewMode">
      <default>'tree'</default>
      <summary>Source Control View Mode</summary>
      <description>Controls whether the source control changes view uses a folder tree or a flat list.</description>
    </key>
  </schema>
</schemalist>
XML
}

write_old_schema
glib-compile-schemas --strict "$schema_dir"

GSETTINGS_BACKEND=keyfile \
  XDG_CONFIG_HOME="$config_dir" \
  GSETTINGS_SCHEMA_DIR="$schema_dir" \
  gsettings set io.github.cadric.Riteed theme "'dark'"
GSETTINGS_BACKEND=keyfile \
  XDG_CONFIG_HOME="$config_dir" \
  GSETTINGS_SCHEMA_DIR="$schema_dir" \
  gsettings set io.github.cadric.Riteed source-control-view-mode "'list'"

write_new_schema
glib-compile-schemas --strict "$schema_dir"

theme="$(
  GSETTINGS_BACKEND=keyfile \
    XDG_CONFIG_HOME="$config_dir" \
    GSETTINGS_SCHEMA_DIR="$schema_dir" \
    gsettings get io.github.cadric.Riteed theme
)"
mode="$(
  GSETTINGS_BACKEND=keyfile \
    XDG_CONFIG_HOME="$config_dir" \
    GSETTINGS_SCHEMA_DIR="$schema_dir" \
    gsettings get io.github.cadric.Riteed source-control-view-mode
)"
test "$theme" = "'dark'"
test "$mode" = "'list'"

printf 'Schema migration smoke check passed.\n'
