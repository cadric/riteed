# `markdown_plan.md`

> Note: Superseded by ROADMAP.md V14 milestone. Kept for design history.

## 1. Formål

Editoren skal kunne parse og rendere `.md` / `.markdown` filer som preview, uden at ændre den rå tekst. Programmet er stadig først og fremmest en simpel teksteditor med almindelig diff og kode-diff. Markdown-rendering er et ekstra visningslag, ikke en transformering af filen.

V1 skal ramme **CommonMark 0.31.2 + YAML frontmatter**. CommonMark er valgt som normativ standard, fordi den er en præcis og testbar Markdown-specifikation; version 0.31.2 er publiceret 28. januar 2024. Markdownlang-cheatsheetet bruges som feature-checkliste, men ikke som eneste standard, fordi det også viser elementer som tables, math, strikethrough og andre udvidelser, som ikke er V1. ([CommonMark Spec][1])

## 2. Hårde constraints

Disse krav er ikke valgfrie:

1. **WebKit, WebKitGTK, WebView, embedded browser, DOM-rendering og JavaScript-baseret preview er forbudt.**
2. Markdown preview skal renderes med **native GTK4**.
3. Preview må ikke hente noget fra nettet.
4. Preview må ikke automatisk læse lokale filer, der kun refereres fra Markdown, medmindre brugeren eksplicit har givet adgang.
5. Remote billeder må aldrig hentes.
6. Links må ikke åbnes automatisk.
7. Raw HTML må ikke eksekveres.
8. Diff skal fortsat være baseret på rå source text.
9. Extended Markdown er ikke V1.
10. Frontmatter må parses og vises som metadata, men må ikke eksekvere templates, Liquid, shortcodes eller lignende.

GTK4 er relevant som native rendering stack. `GtkTextBuffer` kan lagre tekst og attributter til visning i `GtkTextView`, og `GtkTextView` understøtter tekstvisning, styling, scrolling, selection og child anchors; det passer bedre til en let offline editor end en embedded browser. ([https://docs.gtk.org][2])

## 3. V1-scope

### Med i V1

| Feature                           | Status | Krav                                                                        |
| --------------------------------- | -----: | --------------------------------------------------------------------------- |
| `.md` og `.markdown`              |    Med | Åbnes som tekstfiler med Markdown-preview tilgængelig.                      |
| CommonMark block parsing          |    Med | Headings, paragraphs, lists, blockquotes, code blocks, thematic breaks osv. |
| CommonMark inline parsing         |    Med | Emphasis, strong, code spans, links, images, escapes, entities osv.         |
| YAML frontmatter                  |    Med | Kun i starten af dokumentet. Renderes ikke som body.                        |
| Native GTK preview                |    Med | Ingen WebKit, ingen browser engine.                                         |
| Source diff                       |    Med | Eksisterende diff bevares som rå tekst-diff.                                |
| Image placeholders                |    Med | Images parses, men billeder hentes ikke automatisk.                         |
| Link rendering                    |    Med | Links vises, men åbnes kun efter brugerhandling.                            |
| Raw HTML som sikker/literal tekst |    Med | HTML må ikke eksekveres eller blive DOM.                                    |

### Ikke med i V1

| Feature                        |                               V1-status |
| ------------------------------ | --------------------------------------: |
| Tables                         |                                 Ikke V1 |
| Task lists                     |                                 Ikke V1 |
| Strikethrough                  |                                 Ikke V1 |
| Footnotes                      |                                 Ikke V1 |
| Math / TeX                     |                                 Ikke V1 |
| Heading attributes             |                                 Ikke V1 |
| Wikilinks                      |                                 Ikke V1 |
| Definition lists               |                                 Ikke V1 |
| Subscript / superscript        |                                 Ikke V1 |
| GFM admonitions / alerts       |                                 Ikke V1 |
| Smart punctuation              |                                 Ikke V1 |
| Rendered diff                  |                                 Ikke V1 |
| Semantic Markdown diff         |                                 Ikke V1 |
| Syntax-highlighted code blocks |               Ikke V1, kan være V1.1/V2 |
| Local image folder grant       |                  Ikke V1, kan være V1.1 |
| HTML rendering                 |                                 Ikke V1 |
| WebKit/WebKitGTK               | Permanent forbudt i dette feature-scope |

`pulldown-cmark` er en Rust pull parser for CommonMark, hvor default kun aktiverer CommonMark-features; udvidelser som tables, footnotes og task lists kræver eksplicitte `Options` flags. Det passer godt til V1, fordi udvidelser kan holdes deaktiverede. ([Docs.rs][3])

## 4. Normativ syntax-standard

V1 skal følge **CommonMark 0.31.2** for Markdown body. CommonMark definerer blocks og inlines, og parsing kan tænkes som to faser: først blockstruktur, derefter inline-struktur. ([CommonMark Spec][1])

V1 må gerne være tæt på Markdownlang-cheatsheetet for basisfeatures: headings, text formatting, paragraphs, line breaks, lists, links, images, blockquotes, code og horizontal rules. Men hvor cheatsheetet viser extended eller advanced syntax, skal V1 ikke implementere det. ([Markdown Language][4])

## 5. Frontmatter

Frontmatter er en ekstra feature oven på CommonMark-bodyen.

### 5.1 Syntax

Frontmatter genkendes kun, hvis dokumentet starter med en `---` delimiter på første linje, eventuelt efter UTF-8 BOM.

```markdown
---
title: "Release notes"
date: 2026-05-12
tags:
  - editor
  - markdown
---

# Release notes
```

Jekylls frontmatter-konvention kræver, at frontmatter er det første i filen og består af valid YAML mellem triple-dash-linjer. ([jekyllrb.com][5])

### 5.2 V1-regler

| Situation                                  | Adfærd                                                                        |
| ------------------------------------------ | ----------------------------------------------------------------------------- |
| Fil starter med `---` og har closing `---` | Parse som frontmatter.                                                        |
| Fil starter med `---` og har closing `...` | Kan accepteres, hvis I ønsker kompatibilitet med YAML-style metadata blocks.  |
| Fil starter ikke med `---`                 | Ingen frontmatter.                                                            |
| `---` står senere i dokumentet             | Almindelig CommonMark thematic break eller setext-konflikt, ikke frontmatter. |
| Frontmatter er invalid YAML                | Vis diagnostic, men render body alligevel.                                    |
| Frontmatter mangler closing delimiter      | Behandl helst som almindelig Markdown + diagnostic.                           |
| Frontmatter indeholder Liquid/templates    | Parse ikke og eksekver ikke. Vis som metadata-tekst.                          |

`pulldown-cmark` har en option for YAML-style metadata blocks, hvor metadata starter med `---` og slutter med `---` eller `...`, men V1 bør stadig bruge en manuel pre-parser, fordi frontmatter skal være separat metadata og ikke blandes med render-AST. ([Docs.rs][6])

### 5.3 Datastruktur

```rust
struct MarkdownDocument {
    frontmatter: Option<Frontmatter>,
    body: MarkdownBody,
    diagnostics: Vec<Diagnostic>,
}

struct Frontmatter {
    raw: String,
    parsed: Option<YamlValue>,
    source_range: std::ops::Range<usize>,
    diagnostics: Vec<Diagnostic>,
}

struct MarkdownBody {
    raw: String,
    ast: Vec<MdBlock>,
}
```

YAML parsing må være tolerant. Fejl i frontmatter må aldrig forhindre Markdown body rendering.

## 6. Parserarkitektur

### 6.1 Pipeline

```text
full document text
  -> normalize/read line endings for parser only
  -> detect optional frontmatter
  -> parse frontmatter metadata
  -> parse remaining body as CommonMark
  -> convert parser events to internal render model
  -> attach source ranges
  -> collect diagnostics
  -> render native GTK preview
```

### 6.2 Parservalg

Anbefalet parser:

```rust
use pulldown_cmark::{Options, Parser};

let options = Options::empty();
let parser = Parser::new_ext(markdown_body, options);
```

`Options::empty()` skal være V1-standard. Følgende må ikke aktiveres i V1:

```rust
Options::ENABLE_TABLES
Options::ENABLE_FOOTNOTES
Options::ENABLE_STRIKETHROUGH
Options::ENABLE_TASKLISTS
Options::ENABLE_SMART_PUNCTUATION
Options::ENABLE_HEADING_ATTRIBUTES
Options::ENABLE_MATH
Options::ENABLE_GFM
Options::ENABLE_DEFINITION_LIST
Options::ENABLE_SUBSCRIPT
Options::ENABLE_SUPERSCRIPT
Options::ENABLE_WIKILINKS
```

`pulldown-cmark` dokumenterer disse options som ekstra features uden for CommonMark, herunder tables, footnotes, strikethrough, task lists, smart punctuation, heading attributes, math, GFM, subscript, superscript og wikilinks. ([Docs.rs][6])

### 6.3 HTML-output forbud

Selvom `pulldown-cmark` har et HTML-rendering module, må det ikke være production-rendering path i denne app. Parser-events må bruges, men output skal konverteres til native GTK preview. `pulldown-cmark` dokumenterer HTML-modulet som en renderer fra event iterator til HTML; netop den vej skal ikke bruges i appens V1-preview. ([Docs.rs][3])

Tilladt:

```text
Markdown text -> pulldown-cmark events -> app AST -> native GTK widgets/tags
```

Forbudt:

```text
Markdown text -> HTML -> WebKit/WebView
Markdown text -> HTML -> DOM
Markdown text -> HTML -> JavaScript renderer
Markdown text -> HTML -> browser preview
```

## 7. Intern AST/render-model

Appen bør ikke rendere direkte fra `pulldown-cmark` events. Byg en simpel intern model, så preview, diagnostics, selection mapping og diff-integration kan arbejde stabilt.

```rust
enum MdBlock {
    Heading {
        level: u8,
        children: Vec<MdInline>,
        range: Range<usize>,
    },
    Paragraph {
        children: Vec<MdInline>,
        range: Range<usize>,
    },
    BlockQuote {
        children: Vec<MdBlock>,
        range: Range<usize>,
    },
    List {
        ordered: bool,
        start: Option<u64>,
        tight: bool,
        items: Vec<ListItem>,
        range: Range<usize>,
    },
    CodeBlock {
        language_hint: Option<String>,
        text: String,
        range: Range<usize>,
    },
    ThematicBreak {
        range: Range<usize>,
    },
    HtmlBlock {
        raw: String,
        range: Range<usize>,
    },
}

struct ListItem {
    children: Vec<MdBlock>,
    range: Range<usize>,
}

enum MdInline {
    Text(String),
    Emphasis(Vec<MdInline>),
    Strong(Vec<MdInline>),
    Code(String),
    Link {
        url: String,
        title: Option<String>,
        children: Vec<MdInline>,
        range: Range<usize>,
    },
    Image {
        src: String,
        title: Option<String>,
        alt: String,
        range: Range<usize>,
    },
    Html(String),
    SoftBreak,
    HardBreak,
}
```

`pulldown-cmark` har en `OffsetIter`, som er relevant, fordi den kan give Markdown events med source ranges. Source ranges er vigtige for scroll-sync, diagnostics og klik fra preview tilbage til source. ([Docs.rs][3])

## 8. CommonMark block syntax i V1

### 8.1 Headings

V1 skal understøtte ATX headings:

```markdown
# H1
## H2
### H3
#### H4
##### H5
###### H6
```

Regler:

* 1–6 `#`.
* Op til 3 spaces indentation.
* 4 spaces betyder ikke heading.
* Der skal være space/tab efter `#`, medmindre heading er tom.
* Optional closing `#` er tilladt.
* Heading content parses som inline content.

CommonMark definerer ATX headings som 1–6 unescaped `#`, hvor opening sequence skal følges af space/tab eller linjeafslutning; heading level svarer til antallet af opening `#`. ([CommonMark Spec][1])

V1 skal også understøtte setext headings:

```markdown
Heading 1
=========

Heading 2
---------
```

Regler:

* `=` giver H1.
* `-` giver H2.
* Underline må have op til 3 spaces indentation.
* Setext heading må ikke være tom.
* Hvis en dashed line både kan være setext underline og thematic break, skal setext-reglen vinde, når forrige linje er paragraph-indhold.

CommonMark specificerer, at setext headings bruger `=` eller `-` underlines, og at heading-indholdet parses som inline content. ([CommonMark Spec][1])

### 8.2 Paragraphs og line breaks

V1 skal understøtte almindelige paragraphs:

```markdown
This is paragraph one.

This is paragraph two.
```

Regler:

* Blank line adskiller paragraphs.
* Soft line break vises som normal tekstombrydning.
* Hard line break via trailing spaces eller backslash.
* Markdown parser ikke “fejlretter” brugerens tekst.

CommonMark definerer hard line break via backslash ved linjeslutning og skelner mellem soft og hard line breaks. ([CommonMark Spec][1])

### 8.3 Thematic breaks

```markdown
---
***
___
- - -
```

Regler:

* Mindst tre ens `-`, `_` eller `*`.
* Op til 3 spaces indentation.
* Spaces/tabs mellem markører er tilladt.
* Andre tegn på linjen gør det ikke til thematic break.
* Hvis linjen også kan være setext underline, vinder setext.

CommonMark beskriver thematic breaks som tre eller flere matchende `-`, `_` eller `*`, eventuelt separeret af spaces/tabs, med op til 3 spaces indentation. ([CommonMark Spec][1])

### 8.4 Blockquotes

```markdown
> Quote
>
> Second paragraph
> > Nested quote
```

Regler:

* `>` starter blockquote.
* Optional space efter `>`.
* Nested blockquotes skal understøttes.
* Blockquote kan indeholde paragraphs, headings, lists, code blocks, thematic breaks og andre blockquotes.
* Preview viser nesting med margin/border, ikke HTML.

CommonMark tillader empty blockquotes, nested structures og blank lines inden for blockquotes. ([CommonMark Spec][1])

### 8.5 Lists

Unordered:

```markdown
- Item
+ Item
* Item
```

Ordered:

```markdown
1. First
2. Second

1) First
2) Second
```

Regler:

* Bullet marker: `-`, `+`, `*`.
* Ordered marker: digits efterfulgt af `.` eller `)`.
* Nested lists skal understøttes.
* Tight/loose list rendering skal følge parser-resultatet.
* List item continuation indentation skal følge CommonMark.
* Lazy continuation lines skal understøttes.

CommonMark beskriver list item continuation, laziness og sublist indentation; V1 skal lade parseren håndtere disse detaljer i stedet for at implementere heuristikker i UI-laget. ([CommonMark Spec][1])

### 8.6 Code blocks

Indented code:

```markdown
    code here
```

Regler:

* 4 spaces eller tab-stop-ækvivalent.
* Indhold parses ikke som Markdown.
* Indented code kan ikke interrupt’e en paragraph.

CommonMark angiver, at code block content er literal text, ikke parses som Markdown, og at indented code blocks ikke kan interrupt’e paragraphs. ([CommonMark Spec][1])

Fenced code:

````markdown
```rust
fn main() {}
```
````

Regler:

* Fence er mindst 3 backticks eller tildes.
* Backticks og tildes må ikke blandes.
* Op til 3 spaces indentation.
* Closing fence skal være samme type og mindst samme længde.
* Info string gemmes som `language_hint`.
* Syntax highlighting er ikke V1.
* Hvis closing fence mangler, fortsætter code block til container/document end.

CommonMark definerer fenced code blocks med mindst tre backticks eller tildes, optional info string og closing fence af samme type. ([CommonMark Spec][1])

### 8.7 Link reference definitions

```markdown
[docs]: https://example.com "Title"

See [docs].
```

Regler:

* Reference definitions renderes ikke som synligt paragraph-indhold.
* Definitions kan komme før eller efter links.
* Matching er case-insensitive.
* Første definition vinder.
* Destination må ikke hentes eller verificeres.

CommonMark specificerer, at links kan komme før deres definition, at første matchende definition vinder, og at label matching er case-insensitive. ([CommonMark Spec][1])

### 8.8 Raw HTML blocks

CommonMark har raw HTML som syntax, men denne app må ikke eksekvere eller DOM-rendere HTML. CommonMark beskriver HTML blocks, herunder særlige regler for tags som `pre`, `script`, `style` og `textarea`; i denne app skal sådanne nodes behandles som sikker tekst eller skjules med diagnostic, ikke som HTML. ([CommonMark Spec][1])

V1-regel:

```text
Raw HTML parsed -> render as literal escaped text or safe placeholder.
Raw HTML must not become DOM.
Raw HTML must not run scripts.
Raw HTML must not load CSS.
Raw HTML must not load external resources.
```

OWASP beskriver output encoding som en måde at vise untrusted input som data i stedet for eksekverbar kode; det princip bør bruges, når raw HTML vises som tekst. ([OWASP Cheat Sheet Series][7])

## 9. CommonMark inline syntax i V1

### 9.1 Emphasis og strong

```markdown
*italic*
_italic_

**bold**
__bold__

***bold italic***
```

Regler:

* `*` og `_` skal følge CommonMark delimiter/flanking-regler.
* `*` kan give intraword emphasis.
* `_` må ikke give emphasis inde i ord.
* Nested emphasis skal følge parser-resultatet.

CommonMark viser eksplicit, at intraword emphasis med `*` er tilladt, mens `_` ikke giver emphasis inde i ord. ([CommonMark Spec][1])

### 9.2 Inline code

```markdown
Use `code`.
Use `` code with ` inside ``.
```

Regler:

* Markdown parses ikke inde i code spans.
* Backslashes er literal inde i code spans.
* Entity references behandles som literal i code spans.
* Renderer bruger monospace styling.

CommonMark angiver, at entity/numeric references er literal text i code spans og code blocks. ([CommonMark Spec][1])

### 9.3 Escapes og entities

```markdown
\*not italic\*
\# not heading
```

Regler:

* ASCII punctuation kan backslash-escapes.
* Backslash før ikke-escapable tegn vises som literal backslash.
* Escaped characters mister deres Markdown-betydning.
* Entity/numeric references dekodes i almindelig tekst, men må ikke skabe Markdown-struktur.

CommonMark definerer backslash escapes for ASCII punctuation og angiver, at escapes ikke virker i code blocks, code spans, autolinks eller raw HTML. ([CommonMark Spec][1])

### 9.4 Links

```markdown
[Text](https://example.com)
[Text](https://example.com "Title")
[Reference][id]
[id]: https://example.com "Title"
<https://example.com>
<email@example.com>
```

Regler:

* Inline links understøttes.
* Reference links understøttes.
* Collapsed og shortcut references understøttes.
* Link title understøttes.
* Links renderes klikbare/styled.
* Hover/tooltip/statusbar kan vise URL.
* Link må kun åbnes efter eksplicit brugerhandling.
* Ingen link preview.
* Ingen HEAD/GET/DNS eller metadata-fetch.

CommonMark beskriver inline links, reference links, precedence, case-insensitive matching og spacing-regler omkring link destination/title. ([CommonMark Spec][1])

### 9.5 Images

```markdown
![Alt text](image.jpg "Title")
```

Regler:

* Images parses som image nodes.
* `alt`, `src` og `title` gemmes.
* Remote images vises som placeholder.
* Local images vises som placeholder i V1.
* `file://` images må ikke læses automatisk.
* `data:` images bør ikke renderes i V1.
* Image alt text skal være synlig eller tilgængelig.

CommonMark definerer image syntax som link-lignende syntax med `![...]`, hvor image description typisk bruges som HTML `alt` attribute; appen skal dog ikke outputte HTML i preview. ([CommonMark Spec][1])

## 10. Renderer-spec: native GTK4 only

### 10.1 Forbudt rendering stack

Følgende er forbudt:

```text
WebKit
WebKitGTK
WebView
Embedded browser
HTML DOM renderer
JavaScript-based renderer
Remote CSS
Remote fonts
Remote images
HTML preview pipeline
```

Følgende dependency-typer må ikke tilføjes for Markdown-preview:

```text
webkit*
webview*
browser engine bindings
JS runtime for preview rendering
HTML-to-GTK renderer that relies on DOM/browser semantics
```

### 10.2 Tilladt rendering stack

Tilladt:

```text
GtkTextView + GtkTextBuffer + TextTags
GtkBox/GtkListBox with block widgets
GtkLabel with Pango attributes
GtkSeparator for thematic breaks
GtkFrame/GtkBox for blockquotes/code blocks
GtkButton/GtkLinkButton-like behavior for links
Custom lightweight GTK widgets
```

`GtkTextBuffer` lagrer tekst og attributter til display i `GtkTextView`, og `GtkTextView` har scrolling, selection, text actions og child widget support; det er nok til V1-rendering uden browser engine. ([https://docs.gtk.org][2])

### 10.3 Rendering mapping

| Markdown node  | Native GTK rendering                                    |
| -------------- | ------------------------------------------------------- |
| H1–H6          | Label/TextView range med større font, vægt og spacing.  |
| Paragraph      | Wrapped text med paragraph spacing.                     |
| Emphasis       | Italic text tag/Pango attribute.                        |
| Strong         | Bold text tag/Pango attribute.                          |
| Inline code    | Monospace text tag.                                     |
| Code block     | Monospace block, whitespace bevares.                    |
| Blockquote     | Indrykket container med border/margin.                  |
| Ordered list   | Renderer-genereret nummerering.                         |
| Unordered list | Renderer-genererede bullets.                            |
| Link           | Styled clickable text; åbnes kun ved brugerhandling.    |
| Image          | Placeholder med alt/src/title.                          |
| Thematic break | `GtkSeparator` eller custom line.                       |
| Raw HTML       | Escaped literal text eller placeholder.                 |
| Frontmatter    | Metadata panel, collapsible block eller hidden section. |

### 10.4 Renderer må ikke mutere source

Markdown-preview må aldrig:

* ændre source text,
* rette Markdown syntax,
* normalisere brugerens fil ved render,
* omskrive links,
* downloade resources,
* injecte HTML,
* gemme rendered content tilbage i filen.

## 11. Flatpak og filadgang

Flatpak sandboxing giver som udgangspunkt meget begrænset adgang til host-miljøet, herunder ingen adgang til host-filer ud over runtime/app/app-data paths og ingen netværksadgang som default. Det passer med appens krav om minimal filsystemadgang og ingen netadgang. ([Flatpak][8])

### 11.1 Manifest-regler

V1-manifest bør ikke have:

```text
--share=network
--filesystem=home
--filesystem=host
--filesystem=/
```

V1 bør bruge portals til open/save. XDG FileChooser-portalen gør det muligt for sandboxed apps at bede brugeren om adgang til filer uden bred filsystemadgang. ([Flatpak][9])

GTK/Flatpak-dokumentationen anbefaler toolkit/portal-integration til filer og URIs; for moderne GTK4 bør I bruge den aktuelle GTK4 file dialog API, hvor bindings og runtime understøtter det, og ellers sikre at portal-baseret file open/save stadig bruges. GTK4-dokumentationen viser, at `FileDialog` er den nyere asynkrone file chooser API fra 4.10, mens `FileChooserNative` stadig dokumenterer portal-adfærd i sandboxede miljøer, men er markeret deprecated i GTK4. ([https://docs.gtk.org][10])

### 11.2 Image file access

| Image source                | V1-adfærd                                                        |
| --------------------------- | ---------------------------------------------------------------- |
| `https://example.com/a.png` | Placeholder. Ingen fetch.                                        |
| `http://example.com/a.png`  | Placeholder. Ingen fetch.                                        |
| `./image.png`               | Placeholder. Ingen auto-read.                                    |
| `images/a.png`              | Placeholder. Ingen auto-read.                                    |
| `/home/user/a.png`          | Placeholder. Ingen auto-read.                                    |
| `file:///home/user/a.png`   | Placeholder. Ingen auto-read.                                    |
| `data:image/png;base64,...` | Disabled eller placeholder.                                      |
| Already granted file        | Kan stadig være placeholder i V1; lokal rendering kan være V1.1. |

V1.1 kan tilføje “Grant folder access for local images” via portal, men det er ikke V1.

### 11.3 Link access

Links må vises, men ikke åbnes automatisk.

Tilladt:

```text
User clicks link -> app asks/opens through system URI handler.
```

Forbudt:

```text
Render link -> DNS lookup
Render link -> HTTP request
Render link -> metadata preview
Render link -> remote favicon
Render link -> screenshot preview
Render link -> tracking pixel
```

Flatpak portal support nævner URI opening via GTK/GIO-funktioner, men det skal stadig være brugerinitieret i denne app. ([Flatpak][11])

## 12. Diff-integration

Eksisterende diff skal ikke erstattes.

### 12.1 V1-diff

| Mode                   | Adfærd                                    |
| ---------------------- | ----------------------------------------- |
| Text diff              | Uændret.                                  |
| Code diff              | Uændret.                                  |
| Markdown file diff     | Source text diff.                         |
| Split Markdown preview | Valgfrit, men preview er ikke diff-kilde. |
| Rendered diff          | Ikke V1.                                  |
| Semantic Markdown diff | Ikke V1.                                  |

### 12.2 Source ranges

Hvis AST nodes har source ranges, kan UI senere understøtte:

* klik på preview-block -> scroll source til range,
* diagnostics på raw source,
* highlight af Markdown block i editor,
* preview sync ved scroll.

Dette er V1-nyttigt, men semantic diff er stadig ikke V1.

## 13. Diagnostics

Markdown har normalt ikke “syntax errors”, men appen skal give diagnostics for praktiske og sikkerhedsrelevante forhold.

| Diagnostic                     | Severity | Eksempel                                              |   |   |   |
| ------------------------------ | -------: | ----------------------------------------------------- | - | - | - |
| Invalid YAML frontmatter       |  Warning | `title: [unterminated`                                |   |   |   |
| Unclosed frontmatter candidate |  Warning | Fil starter med `---`, men mangler closing delimiter. |   |   |   |
| Remote image blocked           |     Info | `![x](https://...)`                                   |   |   |   |
| Local image unavailable        |     Info | `![x](./a.png)`                                       |   |   |   |
| `file://` image blocked        |  Warning | `![x](file:///...)`                                   |   |   |   |
| Raw HTML rendered as literal   |     Info | `<script>...</script>`                                |   |   |   |
| Unsupported table syntax       |     Info | `                                                     | A | B | ` |
| Unsupported task list          |     Info | `- [x] done`                                          |   |   |   |
| Unsupported footnote           |     Info | `[^1]`                                                |   |   |   |
| Unsupported math               |     Info | `$x^2$`                                               |   |   |   |
| Unsupported strikethrough      |     Info | `~~text~~`                                            |   |   |   |

Diagnostics må ikke ændre dokumentet.

## 14. Performance

Markdown-preview må ikke blokere UI-thread.

### 14.1 V1-performancekrav

* Debounce parsing, fx 150–300 ms efter sidste edit.
* Parse i worker/task, render update på GTK main thread.
* Cancel stale parse jobs.
* Cache AST pr. document revision.
* Begræns live preview for store filer.
* Ingen IO i render loop.
* Ingen netværksforsøg.
* Ingen image decoding i V1.

### 14.2 Store filer

Forslag:

|               Størrelse | Adfærd                                            |
| ----------------------: | ------------------------------------------------- |
|                  < 1 MB | Live preview.                                     |
|                 1–10 MB | Debounced preview, eventuelt langsommere refresh. |
|                 > 10 MB | Manual preview eller warning.                     |
| Meget store code blocks | Render med fallback eller virtualisering.         |

Grænser skal være konfigurerbare.

## 15. Sikkerhed

### 15.1 Threat model

Markdown-filen kan være untrusted.

Risici:

* raw HTML med script/style,
* remote image tracking,
* `file://` references,
* store `data:` URIs,
* malicious links,
* parser crash input,
* UI freeze via pathological documents.

### 15.2 V1-sikkerhedskrav

* Ingen WebKit/WebKitGTK.
* Ingen browser engine.
* Ingen JavaScript execution.
* Ingen HTML DOM.
* Ingen remote resource fetch.
* Ingen automatic local file read fra Markdown references.
* Ingen auto-open links.
* Ingen template execution.
* Ingen Liquid/Jekyll processing.
* Raw HTML vises som escaped/literal tekst eller placeholder.
* `data:` payloads begrænses eller blokeres.
* Parser-fejl må ikke crashe appen.
* Store dokumenter må ikke fryse UI.

## 16. Testspec

### 16.1 Testgrupper

```text
tests/
  markdown/
    headings.md
    setext-headings.md
    paragraphs.md
    emphasis.md
    strong.md
    links.md
    reference-links.md
    images.md
    blockquotes.md
    lists-tight.md
    lists-loose.md
    code-fenced.md
    code-indented.md
    thematic-breaks.md
    raw-html.md

  frontmatter/
    valid-yaml.md
    invalid-yaml.md
    empty-frontmatter.md
    unclosed-frontmatter.md
    not-at-top.md

  unsupported/
    table.md
    task-list.md
    strikethrough.md
    footnote.md
    math.md
    heading-attributes.md
    wikilinks.md

  security/
    raw-script.md
    raw-style.md
    remote-image.md
    file-uri-image.md
    data-uri-image.md
    malicious-link.md

  flatpak/
    no-network.md
    no-host-files.md
    portal-open-save.md
```

### 16.2 Acceptance criteria

V1 er klar, når:

* `.md` og `.markdown` åbnes som tekstfiler med preview.
* CommonMark basisfeatures renderes korrekt nok til daglig brug.
* Frontmatter parses separat og renderes ikke som body.
* Extended syntax ikke aktiveres.
* WebKit/WebKitGTK ikke findes i dependency tree.
* Preview bruger native GTK.
* Remote images hentes ikke.
* Local referenced images læses ikke automatisk.
* Links åbnes kun efter brugerhandling.
* Raw HTML eksekveres ikke.
* Diff forbliver source-text-baseret.
* Flatpak-manifest har ingen network permission og ingen bred filesystem permission.
* Store filer fryser ikke UI.

## 17. Implementeringsfaser

### Fase 1 — Scope freeze

* Fastlæg CommonMark 0.31.2 som syntax baseline.
* Fastlæg YAML frontmatter som eneste ekstra feature.
* Fastlæg WebKit/WebKitGTK som forbudt.
* Fastlæg native GTK4 renderer.
* Fastlæg remote image blocking.
* Fastlæg source diff som eneste V1-diff.

### Fase 2 — Parser

* Tilføj `.md` / `.markdown` detection.
* Implementér frontmatter split.
* Implementér tolerant YAML parse.
* Parse body med `pulldown-cmark` og `Options::empty()`.
* Konverter events til intern AST.
* Tilføj source ranges.
* Tilføj diagnostics.

### Fase 3 — Native preview

* Implementér headings.
* Implementér paragraphs.
* Implementér emphasis/strong.
* Implementér inline code.
* Implementér code blocks.
* Implementér lists.
* Implementér blockquotes.
* Implementér links.
* Implementér image placeholders.
* Implementér thematic breaks.
* Implementér raw HTML literal rendering.
* Implementér frontmatter metadata panel.

### Fase 4 — Editor integration

* Source / Preview / Split modes.
* Debounced preview refresh.
* Scroll sync, hvis source ranges er klar.
* Link click handler.
* Diagnostics panel/status.

### Fase 5 — Diff integration

* Bevar eksisterende source diff.
* Markdown filer bruger samme diff engine.
* Tilføj eventuelt preview per side, men ikke rendered diff.
* Ingen semantic Markdown diff i V1.

### Fase 6 — Flatpak hardening

* Fjern/undgå network permission.
* Fjern/undgå broad filesystem permission.
* Brug portal-baseret open/save.
* Test remote image blocking.
* Test `file://` blocking.
* Test at preview ikke laver network IO.

### Fase 7 — Release tests

* CommonMark fixture tests.
* Frontmatter tests.
* Unsupported extension tests.
* Security tests.
* Large file tests.
* Dependency audit: ingen WebKit/WebKitGTK.

## 18. Kilder

* CommonMark 0.31.2: normativ Markdown-spec for V1. ([CommonMark Spec][1])
* Markdownlang cheatsheet: feature-checkliste, men ikke alene normativ standard. ([Markdown Language][4])
* `pulldown-cmark`: Rust CommonMark parser; default er CommonMark, extensions kræver options. ([Docs.rs][3])
* `pulldown-cmark Options`: tables, footnotes, task lists, strikethrough, math, heading attributes osv. er options/udvidelser. ([Docs.rs][6])
* Jekyll frontmatter: YAML mellem triple-dashed lines i starten af filen. ([jekyllrb.com][5])
* GTK4 `TextBuffer` / `TextView`: native text rendering med attributes, selection, scrolling og widgets. ([https://docs.gtk.org][2])
* Flatpak sandbox permissions og portals. ([Flatpak][8])
* XDG FileChooser portal: sandboxed file access via user selection. ([Flatpak][9])
* OWASP XSS prevention: untrusted input bør vises som data via korrekt output encoding, ikke som eksekverbar kode. ([OWASP Cheat Sheet Series][7])

---

# Starter prompt, max 4.000 characters

```text
Du er softwarearkitekt og senior Rust/GTK4-udvikler. Læs `docs/markdown_plan.md` og implementér Markdown V1 i editoren ud fra planen.

Primære krav:
- Følg `docs/markdown_plan.md` som kontrakt. Afvig ikke uden at dokumentere hvorfor.
- Syntaxmål er CommonMark V1 + YAML frontmatter i starten af dokumentet.
- Extended Markdown er ikke V1: tables, task lists, footnotes, strikethrough, math, heading attributes, wikilinks, definition lists, subscript/superscript og GFM-admonitions skal ikke aktiveres.
- WebKit, WebKitGTK, WebView, embedded browser, DOM-rendering og JavaScript-baseret preview er eksplicit forbudt. Brug kun native GTK4-rendering.
- Appen har ingen netadgang og minimal filsystemadgang. Preview må aldrig hente remote billeder, link metadata eller eksterne ressourcer.
- Appen pakkes som Flatpak. Brug portals til open/save og undgå brede permissions.

Implementeringsretning:
1. Tilføj Markdown-detektion for `.md` og `.markdown`.
2. Split dokumentet i optional YAML frontmatter og Markdown body.
3. Parse body med `pulldown-cmark` med CommonMark-only options. Brug ikke extension flags.
4. Konverter parser-events til intern AST/render-model med source ranges.
5. Render preview som native GTK4-widgets eller `GtkTextView`/`GtkTextBuffer` med tags.
6. Raw HTML skal vises som literal/sikker tekst, ikke eksekveres og ikke fortolkes som DOM.
7. Images skal parses, men vises som placeholder med alt text/src, medmindre eksplicit lokal adgang senere implementeres.
8. Links skal vises klikbare, men må kun åbnes efter brugerhandling.
9. Diff skal fortsat være source-text-baseret. Rendered/semantic Markdown diff er ikke V1.
10. Tilføj diagnostics for invalid frontmatter, unsupported syntax, remote image blocked, local image unavailable og raw HTML hidden/literal.

Acceptkriterier:
- Ingen WebKit/WebKitGTK dependency findes i build, manifest, feature flags eller runtime path.
- CommonMark-basics renderer korrekt: headings, paragraphs, emphasis, strong, links, images-as-placeholders, lists, blockquotes, code og thematic breaks.
- Frontmatter renderes ikke som body.
- Ingen netværkskald sker under preview.
- Remote images og `file://` references læses ikke automatisk.
- Store filer håndteres med debounce/cancel eller fallback.
- Tests dækker parser, renderer, frontmatter, unsupported extensions, Flatpak-permissions og sikkerhedscases.

Start med at læse `docs/markdown_plan.md`, foreslå en modulstruktur, og implementér derefter den mindste V1-kerne først: frontmatter split, parser pipeline, native preview-renderer og tests.
```

[1]: https://spec.commonmark.org/0.31.2/ "CommonMark Spec"
[2]: https://docs.gtk.org/gtk4/class.TextBuffer.html "Gtk.TextBuffer"
[3]: https://docs.rs/pulldown-cmark "pulldown_cmark - Rust"
[4]: https://www.markdownlang.com/cheatsheet/ "Markdown Cheat Sheet, Markdown Cheat Sheet Syntax - Markdown Documentation"
[5]: https://jekyllrb.com/docs/front-matter/ "Front Matter | Jekyll • Simple, blog-aware, static sites"
[6]: https://docs.rs/pulldown-cmark/latest/pulldown_cmark/struct.Options.html "Options in pulldown_cmark - Rust"
[7]: https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html?utm_source=chatgpt.com "Cross Site Scripting Prevention Cheat Sheet"
[8]: https://docs.flatpak.org/en/latest/sandbox-permissions.html "Sandbox Permissions - Flatpak documentation"
[9]: https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileChooser.html "File Chooser - XDG Desktop Portal documentation"
[10]: https://docs.gtk.org/gtk4/ "Gtk – 4.0"
[11]: https://docs.flatpak.org/en/latest/portals.html "Portal support in GTK - Flatpak documentation"
