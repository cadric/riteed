from __future__ import annotations

import tempfile
import unittest
from contextlib import redirect_stderr
from io import StringIO
from pathlib import Path

from tools.scanners import ui_xml


class UiXmlHardeningTests(unittest.TestCase):
    def _root_with_ui(self, source: str) -> Path:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        path = root / "data" / "ui" / "fixture.ui"
        path.parent.mkdir(parents=True)
        path.write_text(source, encoding="utf-8")
        return root

    def test_surface_and_menu_discovery_uses_xml_structure_and_opening_lines(self) -> None:
        root = self._root_with_ui(
            """<interface>
  <menu id='primary-menu'/>
  <object
      id='window'
      class='AdwApplicationWindow'/>
  <template parent='AdwWindow' class='ExampleDialog'/>
</interface>
"""
        )

        hits = ui_xml.ui_surface_hits(root)

        self.assertEqual(
            [(hit.line, hit.kind, hit.match) for hit in hits],
            [
                (2, "menu", "<menu id='primary-menu'/>"),
                (3, "surface", "<object"),
                (6, "surface", "<template parent='AdwWindow' class='ExampleDialog'/>"),
            ],
        )

    def test_template_parent_identifies_a_surface_with_arbitrary_custom_class(self) -> None:
        root = self._root_with_ui("<interface>\n<template class='RiteedPrefs' parent='AdwPreferencesWindow'/>\n</interface>\n")
        self.assertEqual([(hit.line, hit.kind) for hit in ui_xml.ui_surface_hits(root)], [(2, 'surface')])

    def test_non_ascii_user_visible_text_requires_translation(self) -> None:
        root = self._root_with_ui("<interface><property name='label'>保存</property></interface>\n")
        self.assertEqual(len(ui_xml.translatable_property_errors(root)), 1)

    def test_multiple_surfaces_on_one_line_fail_as_ambiguous_review_anchors(self) -> None:
        root = self._root_with_ui(
            "<interface><object class='AdwWindow'/><object class='AdwDialog'/></interface>\n"
        )
        errors: list[str] = []

        hits = ui_xml.ui_surface_hits(root, errors)

        self.assertEqual(len(hits), 1)
        self.assertEqual(len(errors), 1)
        self.assertIn("multiple 'surface' review sites share one source line", errors[0])

    def test_translation_scan_handles_attribute_order_quotes_and_multiple_properties(self) -> None:
        root = self._root_with_ui(
            """<interface>
  <property translatable='yes' name='title'>Localized</property><property name='subtitle'>Missing subtitle</property>
  <property name='tooltip-text'>Missing tooltip</property><property name='label' translatable='yes'>Localized label</property>
</interface>
"""
        )

        self.assertEqual(
            ui_xml.translatable_property_errors(root),
            [
                "data/ui/fixture.ui:2: property 'subtitle' with text must set translatable='yes'",
                "data/ui/fixture.ui:3: property 'tooltip-text' with text must set translatable='yes'",
            ],
        )

    def test_translation_scan_reads_multiline_nested_property_text(self) -> None:
        root = self._root_with_ui(
            """<interface>
  <property
      name="title">
    Missing <b>nested</b> title
  </property>
</interface>
"""
        )

        self.assertEqual(
            ui_xml.translatable_property_errors(root),
            ["data/ui/fixture.ui:2: property 'title' with text must set translatable='yes'"],
        )

    def test_icon_only_button_scan_uses_element_and_property_boundaries(self) -> None:
        root = self._root_with_ui(
            """<interface>
  <object id='missing-name' class='GtkButton'>
    <!-- tooltip-text and accessible-name in comments are not names. -->
    <property name='action-name'>win.label-item</property>
    <property name='child'>
      <object class='GtkImage'>
        <property name='icon-name'>document-open-symbolic</property>
      </object>
    </property>
  </object>
  <object class='GtkButton' id='empty-label'>
    <property name='icon-name'>document-save-symbolic</property>
    <property name='label'></property>
  </object>
  <object class='GtkMenuButton' id='tooltip-name'>
    <property name='icon-name'>open-menu-symbolic</property>
    <property name='tooltip-text'>Main Menu</property>
  </object>
  <object class='GtkToggleButton' id='accessible-name'>
    <property name='icon-name'>sidebar-show-symbolic</property>
    <accessibility>
      <property name='label'>Show Sidebar</property>
    </accessibility>
  </object>
  <object class='GtkMenuButton' id='popover-label-is-not-a-button-name'>
    <property name='icon-name'>view-more-symbolic</property>
    <property name='popover'>
      <object class='GtkPopover'>
        <property name='label'>A menu item</property>
      </object>
    </property>
  </object>
</interface>
"""
        )

        self.assertEqual(
            ui_xml.icon_only_buttons(root),
            [
                "data/ui/fixture.ui:2: icon-only interactive element lacks accessible naming",
                "data/ui/fixture.ui:11: icon-only interactive element lacks accessible naming",
                "data/ui/fixture.ui:25: icon-only interactive element lacks accessible naming",
            ],
        )

    def test_malformed_xml_fails_closed_with_path_and_location(self) -> None:
        root = self._root_with_ui("<interface>\n  <object class='GtkButton'>\n</interface>\n")

        translation_errors = ui_xml.translatable_property_errors(root)
        accessibility_errors = ui_xml.icon_only_buttons(root)

        self.assertEqual(len(translation_errors), 1)
        self.assertIn("data/ui/fixture.ui:3: invalid XML", translation_errors[0])
        self.assertEqual(accessibility_errors, translation_errors)
        surface_errors: list[str] = []
        self.assertEqual(ui_xml.ui_surface_hits(root, surface_errors), [])
        self.assertEqual(surface_errors, translation_errors)
        stderr = StringIO()
        with redirect_stderr(stderr), self.assertRaises(SystemExit):
            ui_xml.ui_surface_hits(root)
        self.assertIn(translation_errors[0], stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
