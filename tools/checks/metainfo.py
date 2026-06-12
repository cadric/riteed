from __future__ import annotations

import xml.etree.ElementTree as ET
from pathlib import Path

from tools.validation_tooling import relpath


def check_release_descriptions(meta: ET.Element, metainfo: Path, root: Path, errors: list[str]) -> None:
    for releases in _children(meta, "releases"):
        for release in _children(releases, "release"):
            version = release.attrib.get("version", "unknown")
            for description in _children(release, "description"):
                if description.attrib.get("translate") != "no":
                    errors.append(
                        f"{relpath(metainfo, root)}: release {version} descriptions must use translate=\"no\""
                    )
                if _has_xml_lang(description):
                    errors.append(
                        f"{relpath(metainfo, root)}: release {version} descriptions must not carry localized xml:lang entries"
                    )


def _children(node: ET.Element, name: str) -> list[ET.Element]:
    return [child for child in node if child.tag.rsplit("}", 1)[-1] == name]


def _has_xml_lang(node: ET.Element) -> bool:
    if any(key.rsplit("}", 1)[-1] == "lang" for key in node.attrib):
        return True
    return any(_has_xml_lang(child) for child in node)
