---
title: "Riteed Markdown V1 showcase"
purpose: "Temporary manual preview test"
expected_preview: "Native GTK preview with diagnostics for blocked or unsupported input"
---

# Riteed Markdown V1 Showcase

Denne tmp-fil er en manuel testfil til Riteeds Markdown Preview V1.
Åbn filen i Riteed, vælg `Toggle Markdown Preview` på fanen, og sammenlign
source med preview.

Hvis previewet viser diagnostics øverst, er det forventet. Denne fil tester
også raw HTML, billeder og ikke-V1 Markdown, så appen kan vise, at den blokerer
eller nedgraderer de ting sikkert.

---

## 1. YAML Frontmatter

Markdown-type: YAML frontmatter i starten af dokumentet.

Kort forklaring: Blokken over denne heading skal vises som metadata i preview,
men den skal ikke rendere som body-indhold.

Markdown:

```yaml
---
title: "Riteed Markdown V1 showcase"
purpose: "Temporary manual preview test"
---
```

Test: Previewet skal have en metadata-sektion, og bodyen skal starte med denne
showcase-heading, ikke med `---`.

---

## 2. Headings

Markdown-type: CommonMark ATX headings og setext headings.

Kort forklaring: ATX headings bruger `#`. Setext headings bruger en underline
med `=` eller `-`.

Markdown:

```markdown
# H1
## H2
### H3

Setext H1
=========

Setext H2
---------
```

# Renderet H1-test

## Renderet H2-test

### Renderet H3-test

Renderet setext H1-test
=======================

Renderet setext H2-test
-----------------------

---

## 3. Paragraphs And Line Breaks

Markdown-type: Paragraphs, soft breaks og hard breaks.

Kort forklaring: Tom linje skiller paragraphs. Soft break og hard break bliver
til linjeskift i Riteeds preview.

Markdown:

```markdown
Paragraph one has normal text.

Paragraph two has a soft
line break.

Hard break with a backslash\
continues on the next line.
```

Paragraph one has normal text.

Paragraph two has a soft
line break.

Hard break with a backslash\
continues on the next line.

---

## 4. Emphasis, Strong, Escapes, And Entities

Markdown-type: CommonMark inline formatting.

Kort forklaring: `*emphasis*`, `**strong**`, escapes og entities parses af
CommonMark. Code spans forbliver literal tekst.

Markdown:

```markdown
This has *emphasis*, **strong**, and ***strong emphasis***.
Escaped markers: \*not emphasis\* and \# not a heading.
Entity test: AT&amp;T should display with an ampersand.
Inline code: `let value = "*literal*";`
```

This has *emphasis*, **strong**, and ***strong emphasis***.
Escaped markers: \*not emphasis\* and \# not a heading.
Entity test: AT&amp;T should display with an ampersand.
Inline code: `let value = "*literal*";`

---

## 5. Links

Markdown-type: Inline links, reference links, autolinks og email autolinks.

Kort forklaring: Links vises klikbare. Riteed åbner kun efter brugerhandling,
og previewet henter ikke link metadata.

Markdown:

```markdown
[Inline HTTPS link](https://example.test/inline "Optional title")
[Reference link][showcase-ref]
<https://example.test/autolink>
<showcase@example.test>

[showcase-ref]: https://example.test/reference "Reference title"
```

[Inline HTTPS link](https://example.test/inline "Optional title")
[Reference link][showcase-ref]
<https://example.test/autolink>
<showcase@example.test>

[showcase-ref]: https://example.test/reference "Reference title"

Test: Linktekst skal styles som link. Intet skal åbne automatisk.

---

## 6. Images As Placeholders

Markdown-type: CommonMark image syntax.

Kort forklaring: Billeder parses, men V1 loader ikke remote billeder, lokale
filer, `file://` URIer eller `data:` URIer. De vises som placeholders med
diagnostics.

Markdown:

```markdown
![Remote placeholder](https://example.test/image.png "Remote title")
![Local placeholder](relative/path/image.png)
![File URI placeholder](file:///tmp/secret.png)
![Data URI placeholder](data:image/png;base64,AA==)
```

![Remote placeholder](https://example.test/image.png "Remote title")
![Local placeholder](relative/path/image.png)
![File URI placeholder](file:///tmp/secret.png)
![Data URI placeholder](data:image/png;base64,AA==)

Test: Previewet skal vise `Markdown Image Placeholder` for hver image node.

---

## 7. Lists

Markdown-type: Ordered, unordered og nested CommonMark lists.

Kort forklaring: V1 lader CommonMark parseren håndtere listestruktur,
continuation og nesting.

Markdown:

```markdown
- First unordered item
- Second unordered item
  - Nested unordered item

1. First ordered item
2. Second ordered item
   1. Nested ordered item
```

- First unordered item
- Second unordered item
  - Nested unordered item

1. First ordered item
2. Second ordered item
   1. Nested ordered item

---

## 8. Blockquotes

Markdown-type: CommonMark blockquotes.

Kort forklaring: Blockquotes kan indeholde paragraphs, nested quotes og anden
Markdown.

Markdown:

```markdown
> Quote paragraph.
>
> > Nested quote.
>
> - Quote list item
```

> Quote paragraph.
>
> > Nested quote.
>
> - Quote list item

---

## 9. Code Blocks

Markdown-type: Fenced code blocks og indented code blocks.

Kort forklaring: Code blocks renderes som literal monospace tekst. Language hint
gemmes og vises, men V1 laver ikke syntax highlighting i Markdown preview.

Markdown:

````markdown
```rust
fn main() {
    println!("markdown preview");
}
```

    indented code block
    stays literal
````

```rust
fn main() {
    println!("markdown preview");
}
```

    indented code block
    stays literal

---

## 10. Thematic Breaks

Markdown-type: CommonMark thematic breaks.

Kort forklaring: `---`, `***` og `___` bliver til en simpel separator i
preview.

Markdown:

```markdown
---
***
___
```

---

***

___

---

## 11. Raw HTML As Literal Text

Markdown-type: CommonMark raw HTML blocks og inline HTML.

Kort forklaring: Raw HTML er tilladt CommonMark input, men Riteed eksekverer
det ikke og bygger ingen DOM. Det vises som literal tekst med diagnostic.

Markdown:

```markdown
<div class="notice">
Raw HTML block should stay literal.
</div>

Inline HTML stays literal too: <span>not DOM</span>.
```

<div class="notice">
Raw HTML block should stay literal.
</div>

Inline HTML stays literal too: <span>not DOM</span>.

---

## 12. Ikke-V1 Syntax Diagnostics

Markdown-type: Extended Markdown markers som V1 bevidst ikke aktiverer.

Kort forklaring: Disse linjer skal ikke blive til rich Markdown features i V1.
De er med for at teste diagnostics for tables, task lists, footnotes,
strikethrough, math, heading attributes, wikilinks, definition lists,
subscript/superscript og GFM admonitions.

Markdown:

```markdown
| A | B |
|---|---|
| 1 | 2 |
- [x] Task list marker
~~Strikethrough marker~~
Footnote marker[^one]
$$x = 1$$
# Heading attribute test {#custom-id}
[[Wiki link]]
Term
: definition
~subscript~
^superscript^
> [!NOTE]
> GFM alert marker
```

| A | B |
|---|---|
| 1 | 2 |
- [x] Task list marker
~~Strikethrough marker~~
Footnote marker[^one]
$$x = 1$$
# Heading attribute test {#custom-id}
[[Wiki link]]
Term
: definition
~subscript~
^superscript^
> [!NOTE]
> GFM alert marker

[^one]: Footnote definition stays non-V1 diagnostic material.

---

## 13. Manual Test Checklist

Markdown-type: Plain checklist for this showcase file.

Kort forklaring: Brug denne sektion som hurtig accepttest efter åbning.

Test:

1. Open `docs/tmp-markdown-v1-showcase.md` in Riteed.
2. Toggle Markdown Preview from the tab menu.
3. Confirm the frontmatter appears as metadata, not normal body text.
4. Confirm headings, paragraphs, emphasis, strong, links, lists, blockquotes,
   code blocks and thematic breaks render visibly.
5. Confirm images are placeholders and do not load external or local resources.
6. Confirm raw HTML appears as literal text.
7. Confirm diagnostics are shown for raw HTML, images and non-V1 syntax.
8. Confirm links do not open unless clicked.
9. Exit preview and confirm the source text is unchanged.
