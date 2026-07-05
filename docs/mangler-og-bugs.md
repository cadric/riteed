# Mangler og bugs

Dette dokument samler observerede mangler og fejl, som skal vurderes senere.
Det er en praktisk huskeliste, ikke en implementeringsplan.

## Editor, compare og font-state

- **Observation:** Editor og diff/compare opleves som om de ikke bruger samme
  font-setup. Hvis fonten skiftes mens man er i diff/compare, virker det ikke
  tydeligt som om editor og compare følger hinanden konsekvent.
- **Forventning:** Editor, compare og andre SourceView-baserede
  tekstoverflader bør dele samme editor-font preference. Window-local zoom må
  gerne være separat, men den effektive font bør beregnes fra samme gemte
  editor-font.
- **Mulig retning:** Gennemgå `EditorZoomController`, compare view setup og
  preference-callbacks, så der er én klar kilde til editor-font og én ensartet
  apply-path for editor og compare. Minimap kan stadig bruge samme font-family
  med fast minimap-size.

## Markdown preview og print

- **Observation:** Når Markdown-visning/preview er aktiv, printer appen stadig
  rå Markdown i stedet for det formaterede preview.
- **Forventning:** Hvis brugeren står i Markdown preview, bør print og print
  preview enten printe den formaterede Markdown-visning eller tydeligt tilbyde
  valg mellem "rå kilde" og "formateret preview".
- **Mulig retning:** Afklar printmodellen for Markdown: enten route print
  gennem preview-renderingen, eller tilføj en eksplicit print-mode for Markdown.

## Source Control compare åbner duplikeret dokument

- **Observation:** Hvis en fil åbnes fra Source Control og samtidig fra Files,
  kan man ende med to faner for samme dokument: en almindelig editorfane og en
  compare/diff-fane.
- **Forventning:** Appen bør undgå at samme fil føles som to uafhængige åbne
  dokumenter, når den ene blot er en compare-state for den samme fil.
- **Mulig retning:** Kombiner states smartere: genbrug eksisterende filfane når
  Source Control åbner compare, eller marker compare som en mode på den samme
  fane fremfor en separat duplikat. Hvis en separat fane bevares, skal UI'en
  gøre relationen tydelig.

## Source Control aktiv fil-markering

- **Observation:** I Source Control er filen ikke markeret som aktiv/current på
  samme måde som i Files.
- **Forventning:** Source Control bør vise hvilken fil der svarer til den
  aktive editor-/compare-fane.
- **Mulig retning:** Genbrug eller spejl den eksisterende selected/current
  file-markering fra Files, så navigationskonteksten er konsistent.

## Ikke-gemte filer i Files

- **Observation:** Files viser ikke en lille markering for filer/faner med
  ikke-gemte ændringer.
- **Forventning:** Der bør være en diskret markering, for eksempel en lille dot,
  ved filer der aktuelt har usaved changes.
- **Mulig retning:** Bind Files-rækken til dokumentets dirty-state og vis en
  lille Adwaita-venlig statusindikator uden at gøre file tree'et visuelt tungt.
