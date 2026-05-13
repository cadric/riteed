# Temporary Markdown Preview Comparison Notes

Purpose: working notes for comparing Riteed Markdown Preview V1 against
markdownviewer.org and Apostrophe section by section.

Comparison order:

1. Riteed
2. markdownviewer.org
3. Apostrophe

These notes are temporary and should be updated as each showcase section is
reviewed.

## Implementation Pass After Comparison

Fixed in the first renderer follow-up:

- Tight-list item text is preserved instead of dropping item contents.
- Soft breaks render as spaces; hard breaks remain explicit line breaks.
- Fenced code blocks no longer show fence delimiters, and code blocks get a
  shaded native TextView presentation.
- Thematic breaks render as a styled separator instead of literal dash text.
- Blockquotes get a visible quote affordance and muted quote styling.
- Links and inline code have more distinct styling.

Fixed in the second renderer follow-up after the `docs/test.md` Riteed vs
Apostrophe comparison:

- Diagnostics are grouped into compact preview notices instead of repeating a
  long diagnostics heading and severity prefix for every unsupported extension.
- Unordered lists render with bullet-style preview markers rather than literal
  dash text.
- Markdown preview content is clamped to a readable column width instead of
  stretching edge to edge.
- Fenced-code language hints are no longer emitted as visible code-block text.
- Code blocks and blockquotes use calmer TextView styling and less ASCII-like
  quote markers.

Still open:

- Per-code-block syntax highlighting needs a larger preview renderer change,
  because the current preview is one `gtk4::TextBuffer` while GtkSourceView
  language highlighting is buffer-wide.
- Image placeholders and raw HTML diagnostics are still informative but could be
  presented with a calmer native placeholder/chip treatment later.
- Heading rhythm and overall spacing may still need a final visual pass after
  the block renderer fixes are visually tested.

---

## 1. YAML Frontmatter

Observed difference summary:

- Riteed renders the fenced code block start marker as visible text:
  ```` ```yaml ````.
- markdownviewer.org and Apostrophe hide fence markers and render only the code
  block content.
- Riteed code blocks are currently plain monospace text without a distinct
  block background.
- markdownviewer.org uses a full code-block surface with background, language
  label, copy button, and syntax highlighting.
- Apostrophe uses a simpler shaded code block with syntax highlighting.
- Riteed paragraph spacing is more compact than both comparison apps.
- Riteed inline code appears as plain/monospace text, while markdownviewer.org
  gives inline code a visible pill-like background.
- Riteed renders thematic breaks as `-----` text, while the comparison apps
  render them as a horizontal rule.

Likely follow-up candidates:

- Hide fenced-code delimiters in preview output.
- Render code blocks with a native GTK background or block-style tag.
- Consider native syntax highlighting for fenced code blocks in a later
  Markdown follow-up.
- Improve inline-code styling.
- Render thematic breaks as a line-like visual instead of literal dashes.
- Add more paragraph spacing in preview if it still feels too dense after code
  block styling is fixed.

---

## 2. Headings

Observed difference summary:

- Riteed parses ATX headings and setext headings correctly: the rendered H1,
  H2, H3, setext H1, and setext H2 all appear as headings.
- Riteed heading styling is more basic than markdownviewer.org and Apostrophe.
  It uses size and weight, but does not draw the light divider/bottom border
  that the comparison apps show under major headings.
- markdownviewer.org and Apostrophe make H1 significantly larger and more
  page-like, with stronger section spacing around headings.
- Riteed has tighter vertical rhythm. The explanatory paragraphs, code sample,
  and rendered heading examples sit closer together than in both comparison
  apps.
- Riteed still shows fenced-code delimiters as visible text in the sample
  block, while markdownviewer.org and Apostrophe render a proper code block
  surface.
- Riteed inline code markers such as `#`, `=`, and `-` appear plain or nearly
  plain, while markdownviewer.org gives them a distinct inline-code background.
- Riteed renders the section separator after the setext H2 as literal `-----`
  text. markdownviewer.org and Apostrophe render it as a horizontal rule.

Likely follow-up candidates:

- Add heading bottom-border styling for H1/H2 or otherwise match GNOME-native
  heading separation more closely.
- Revisit heading scale and spacing so H1/H2/H3 hierarchy reads closer to other
  Markdown previewers without becoming oversized.
- Keep setext parsing as-is; behavior is correct, styling is the gap.
- Share fixes from section 1 for code blocks, inline code, paragraph spacing,
  and thematic breaks.

---

## 3. Paragraphs And Line Breaks

Observed difference summary:

- Riteed renders paragraphs much more tightly than markdownviewer.org and
  Apostrophe. Separate paragraphs appear almost line-adjacent instead of having
  clear paragraph margin.
- Riteed renders CommonMark soft breaks as visible line breaks. The comparison
  apps render the soft break as normal in-paragraph spacing, so "Paragraph two
  has a soft line break." stays on one visual line when width allows.
- Riteed renders hard breaks similarly to the comparison apps: the backslash is
  not shown in the rendered paragraph, and the following text starts on the next
  line.
- Riteed still shows the fenced-code start marker as visible text and uses no
  code-block background. markdownviewer.org and Apostrophe render a shaded code
  block and hide fence markers.
- Riteed heading and paragraph vertical rhythm remains denser than both
  comparison apps in this section.
- Riteed renders the section separator as literal `-----` text. The comparison
  apps render it as a horizontal rule.

Likely follow-up candidates:

- Treat `SoftBreak` as a space or configurable soft-wrap separator in preview,
  while preserving `HardBreak` as an explicit newline.
- Add real paragraph spacing between Markdown paragraphs in the native preview.
- Keep hard-break handling as-is; behavior matches the comparison apps.
- Share fixes from previous sections for fenced code blocks, heading rhythm, and
  thematic breaks.

---

## 4. Emphasis, Strong, Escapes, And Entities

Observed difference summary:

- Riteed renders emphasis, strong, and strong emphasis correctly in the rendered
  output.
- Riteed handles escaped markers correctly in rendered output: escaped `*` and
  `#` remain literal text and do not become emphasis or heading syntax.
- Riteed decodes the entity correctly in rendered output: `AT&amp;T` becomes
  `AT&T`.
- Riteed renders inline code as monospace text, but without the visible
  inline-code background used by markdownviewer.org. Apostrophe is closer to
  Riteed here and uses subtler inline-code styling in the rendered paragraph.
- Riteed treats each source newline in the rendered sample paragraph as a
  visible line break. markdownviewer.org and Apostrophe collapse those soft
  breaks to spaces, so the rendered sample becomes one paragraph that wraps
  naturally.
- Riteed still shows fenced-code delimiters and lacks the code-block surface.
  markdownviewer.org and Apostrophe hide the delimiters and syntax-highlight the
  fenced Markdown sample.
- Riteed uses denser paragraph and heading rhythm than both comparison apps.
- Riteed renders the section separator as literal `-----` text. The comparison
  apps render it as a horizontal rule.

Likely follow-up candidates:

- Keep inline emphasis, strong, escape, and entity parsing behavior as-is; the
  Markdown semantics match the comparison apps.
- Fix `SoftBreak` handling globally so source newlines inside a paragraph
  become spaces instead of forced visual line breaks.
- Improve inline-code styling, especially a subtle background for rendered code
  spans.
- Share fixes from previous sections for fenced code blocks, syntax/block
  styling, paragraph spacing, and thematic breaks.

---

## 5. Links

Observed difference summary:

- Riteed parses the visible link forms correctly: inline link, reference link,
  URI autolink, and email autolink all render as link text.
- Riteed link styling is weaker than both comparison apps. Links are
  underlined, but appear close to normal dark text rather than using the blue or
  accent-colored link treatment shown in markdownviewer.org and Apostrophe.
- Riteed renders each source newline in the link paragraph as a visible line
  break. markdownviewer.org and Apostrophe collapse those soft breaks to spaces,
  so all four rendered links appear in one flowing paragraph.
- The user-action behavior described by the test is not visually verifiable from
  the screenshots, but the rendered output does not show link metadata previews
  in any of the three apps.
- Riteed still shows fenced-code delimiters and lacks the shaded code-block
  surface. markdownviewer.org and Apostrophe hide the delimiters and
  syntax-highlight the fenced Markdown sample.
- Riteed heading, paragraph, and code-sample rhythm is denser than both
  comparison apps.
- The section separator is visible as a horizontal rule in markdownviewer.org
  and Apostrophe. It is not visible in the cropped Riteed screenshot for this
  section, but previous sections show Riteed currently rendering separators as
  literal dash text.

Likely follow-up candidates:

- Keep link parsing behavior as-is; inline, reference, URI autolink, and email
  autolink are all recognized.
- Improve rendered link styling so links use a recognizable accent/link color in
  addition to underline.
- Keep the portal/user-action opening model as-is; it is policy-appropriate.
- Fix `SoftBreak` handling globally so adjacent links in one paragraph flow like
  the comparison apps.
- Share fixes from previous sections for fenced code blocks, code styling,
  paragraph spacing, and thematic breaks.

---

## 6. Images As Placeholders

Observed difference summary:

- Riteed is intentionally more explicit than both comparison apps for this V1
  behavior. It does not load remote, local, `file://`, or `data:` images and
  renders a diagnostic placeholder for each image node.
- This is policy-aligned for Riteed: the preview avoids automatic network,
  filesystem, and data-URI image loading while still exposing the image alt text
  and target URI to the user.
- markdownviewer.org renders the image alt text as plain inline text. It does
  not show a diagnostic reason or the target URI in the rendered preview.
- Apostrophe appears to attempt image rendering and shows broken image icons for
  the sample images. It does not show readable alt text or target URI in the
  rendered output.
- Riteed's placeholder content is more informative than both comparison apps,
  but it is visually noisy: long bracketed italic text makes the preview feel
  less polished.
- Riteed renders each image source line as its own visual line. The comparison
  apps keep the rendered output in one flowing paragraph or one horizontal row
  of image placeholders/icons.
- Riteed still shows fenced-code delimiters and lacks the shaded code-block
  surface. markdownviewer.org and Apostrophe hide the delimiters and
  syntax-highlight the fenced Markdown sample.
- Riteed heading and paragraph rhythm remains denser than both comparison apps.
- Riteed renders the section separator as literal `-----` text. The comparison
  apps render it as a horizontal rule.

Likely follow-up candidates:

- Keep the V1 no-image-loading behavior; it is the safest and clearest default
  for remote, local, `file://`, and `data:` image sources.
- Replace the long bracketed italic placeholder text with a native placeholder
  presentation that still includes alt text, target URI, and a clear "not
  loaded by preview" reason.
- Consider whether image placeholders should intentionally break lines even if
  global `SoftBreak` handling changes to spaces. Per-image visual separation may
  be more useful than strict paragraph flow here.
- Share fixes from previous sections for fenced code blocks, code styling,
  paragraph spacing, and thematic breaks.

---

## 7. Lists

Observed difference summary:

- Riteed does not appear to render list item content correctly in this section.
  The rendered output shows list markers such as `-` and `1.`, but the item text
  itself is missing.
- Riteed does not show a usable visual list structure: unordered bullets,
  ordered numbering, indentation, and nested list levels are not represented in
  the rendered output.
- markdownviewer.org renders the unordered list, nested unordered item, ordered
  list, and nested ordered item clearly with item text and indentation.
- Apostrophe also renders the list structure clearly. Its nested ordered list
  uses a roman marker (`i.`), while markdownviewer.org keeps a numeric marker
  (`1.`). Both are acceptable presentation choices; the important behavior is
  that nesting and item text are preserved.
- This is a functional rendering gap in Riteed, not just a visual style gap.
- Riteed still shows fenced-code delimiters and lacks the shaded code-block
  surface. markdownviewer.org and Apostrophe hide the delimiters and
  syntax-highlight the fenced Markdown sample.
- Riteed heading and paragraph rhythm remains denser than both comparison apps.
- Riteed renders the section separator as literal `-----` text. The comparison
  apps render it as a horizontal rule.

Likely follow-up candidates:

- Fix list event rendering so `Item` contents are emitted and associated with
  the correct list marker and nesting level.
- Add native preview styling for unordered list bullets, ordered markers,
  indentation, and nested list spacing.
- Decide whether nested ordered lists should keep numeric markers or use roman
  markers; either is acceptable as long as the hierarchy is readable.
- Add focused renderer tests for unordered, ordered, and nested list content so
  marker-only regressions are caught.
- Share fixes from previous sections for fenced code blocks, paragraph spacing,
  and thematic breaks.

---

## 8. Blockquotes

Observed difference summary:

- Riteed parses and emits the quote text, but does not render a recognizable
  blockquote container. There is no left quote bar, indentation, or muted quote
  color like the comparison apps show.
- Riteed appears to flatten the quote structure visually. The nested quote is
  shown as text, but the nesting level is not represented with a second quote
  bar or additional indentation.
- Riteed italicizes or otherwise styles the quote text, but that styling alone
  is not enough to communicate blockquote structure.
- The quoted list item exposes the same functional list problem seen in section
  7: Riteed renders only the `-` marker and loses the list item text.
- markdownviewer.org and Apostrophe both render blockquotes with a left border,
  indentation, muted color, nested quote structure, and a readable quoted list
  item.
- This is partly a visual styling gap for blockquotes and partly a functional
  rendering gap for lists inside blockquotes.
- Riteed still shows fenced-code delimiters and lacks the shaded code-block
  surface. markdownviewer.org and Apostrophe hide the delimiters and
  syntax-highlight the fenced Markdown sample.
- Riteed heading and paragraph rhythm remains denser than both comparison apps.
- Riteed renders the section separator as literal `-----` text. The comparison
  apps render it as a horizontal rule.

Likely follow-up candidates:

- Add native blockquote styling: left border, indentation, muted foreground, and
  nested quote indentation/borders.
- Fix list rendering first or alongside blockquotes, because quoted list items
  currently lose their text.
- Add renderer tests for blockquotes containing paragraphs, nested blockquotes,
  and lists.
- Share fixes from previous sections for fenced code blocks, paragraph spacing,
  and thematic breaks.

---

## 9. Code Blocks

Observed difference summary:

- Riteed does not render fenced code blocks as distinct block elements. The
  fence markers and language hint are still visible in rendered output, so the
  Rust fenced block reads like literal Markdown source instead of a code block.
- Riteed renders code text in monospace and preserves indentation, but it does
  not provide a shaded code-block surface, language header, or syntax
  highlighting.
- Riteed renders the indented code block as literal monospace text, but again
  without a distinct block background.
- markdownviewer.org renders separate code-block surfaces with language/header
  rows, copy controls, and syntax highlighting for the Rust block.
- Apostrophe renders separate shaded code-block surfaces and syntax-highlights
  the Rust block. Its indented code block is a plain code surface without a
  language header.
- The core functional issue is fenced block boundary handling: preview output
  should hide fences and emit only code content, while preserving the language
  hint for styling/metadata.
- The high-value polish issue is syntax rendering. Riteed should preferably
  reuse the existing local syntax rendering/highlighting path already used by
  the app, rather than adding a separate Markdown-preview-specific highlighter.
- Riteed heading and paragraph rhythm remains denser than both comparison apps.
- Riteed renders the section separator as literal `-----` text. The comparison
  apps render it as a horizontal rule.

Likely follow-up candidates:

- Fix fenced code block rendering so opening/closing fences are not visible in
  preview output.
- Render fenced and indented code as native block surfaces with monospace text,
  padding, and Adwaita-compatible background.
- Preserve the language hint from fenced blocks and pass it to the existing
  syntax highlighting/rendering pipeline if feasible.
- Keep syntax highlighting offline and local; do not introduce runtime
  downloads or remote language metadata.
- Consider a small language label and copy affordance later, but make correct
  block rendering and syntax reuse the first target.
- Add renderer tests for fenced code, fenced code with language info, and
  indented code blocks.

---

## 10. Thematic Breaks

Observed difference summary:

- Riteed does not render CommonMark thematic breaks as separators. The rendered
  preview shows literal dash text such as `-----` instead of horizontal rules.
- markdownviewer.org and Apostrophe both render `---`, `***`, and `___` as
  horizontal separators.
- Riteed has repeated this issue in previous sections where the showcase section
  separator was rendered as literal dash text.
- Inline code-like markers in the explanatory paragraph are less distinct in
  Riteed. markdownviewer.org gives `---`, `***`, and `___` visible inline-code
  pills, while Apostrophe is more subtle.
- Riteed still shows fenced-code delimiters and lacks the shaded code-block
  surface. markdownviewer.org and Apostrophe hide the delimiters and render the
  sample Markdown in a code block.
- The comparison apps render several horizontal rules with consistent spacing.
  Riteed produces a vertical stack of literal dash lines, which reads as broken
  Markdown rather than separators.

Likely follow-up candidates:

- Handle `Rule` events by inserting a native horizontal separator instead of
  literal dash text.
- Style the separator with Adwaita-compatible muted color, full available
  width, and sensible vertical spacing.
- Add renderer tests for `---`, `***`, `___`, and spaced thematic-break
  variants.
- Share fixes from previous sections for fenced code blocks, inline code
  styling, and paragraph rhythm.

---

## 11. Raw HTML As Literal Text

Observed difference summary:

- Riteed renders raw HTML block and inline HTML as literal text, including the
  tags. It does not build DOM output from the HTML.
- markdownviewer.org and Apostrophe both interpret/render the HTML: the `<div>`
  and `<span>` tags disappear from rendered output, leaving only their text
  content.
- This is an intentional product/security difference rather than a reference-app
  behavior Riteed should blindly copy. Rendering raw HTML as DOM would expand
  the preview surface and needs a separate security/design decision.
- Riteed is more transparent for this V1 rule because the user can see the raw
  tags that are present in the document.
- The showcase text says raw HTML is shown with a diagnostic, but the screenshot
  only shows literal HTML text. If a diagnostic is required, it is not visually
  obvious in the current preview.
- The comparison apps syntax-highlight the raw HTML inside the source code
  sample. Riteed still shows fenced-code delimiters and lacks the shaded
  code-block surface/syntax highlighting.
- Riteed still renders the section separator as literal `-----` text. The
  comparison apps render it as a horizontal rule.

Likely follow-up candidates:

- Keep raw HTML non-executing and non-DOM-rendered by default.
- Decide whether raw HTML should remain literal text in normal flow or be shown
  with a clearer native diagnostic/chip so users understand it was intentionally
  not executed.
- Add renderer tests for block HTML and inline HTML to lock down the non-DOM
  behavior.
- Share fixes from previous sections for fenced code blocks, syntax rendering,
  thematic breaks, and paragraph rhythm.

---

## 13. Manual Test Checklist

Note: section 12 was intentionally skipped in this comparison pass.

Observed difference summary:

- Riteed does not render the ordered checklist correctly. The rendered output
  shows only the ordered markers `1. 2. 3. 4. 5. 6. 7. 8. 9.` inline, while all
  checklist item text is missing.
- markdownviewer.org and Apostrophe both render the checklist as a proper
  ordered list with one item per row, readable item text, indentation, and
  wrapping for long items.
- This confirms the same functional ordered-list rendering bug seen in section
  7. It affects practical manual-test content, not only synthetic list samples.
- Because Riteed drops the item text, inline code inside item 1 cannot be
  evaluated visually in Riteed. The comparison apps show the path
  `docs/tmp-markdown-v1-showcase.md` as inline code, with markdownviewer.org
  using a stronger inline-code pill and Apostrophe using subtler monospace
  styling.
- Riteed heading and paragraph rhythm is again much denser than both comparison
  apps.

Likely follow-up candidates:

- Prioritize ordered-list item rendering; losing checklist text makes manual
  test instructions unusable.
- Add renderer tests for ordered lists with multiple items, long wrapped item
  text, and inline code inside list items.
- Re-test section 13 after fixing list rendering, because it is a useful compact
  acceptance check for preview usability.
- Share fixes from previous sections for inline-code styling and paragraph/list
  spacing.
