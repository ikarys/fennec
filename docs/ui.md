# The UI's design rules

fennec is a tool a developer keeps open all day: calm, dense, readable. The
look lives in two places only, so a screen can never drift from the others:

1. **The theme tokens** (`web/src/styles/app.css`): colours per theme, the
   spacing unit, radii. Components use the tokens (`bg-panel`, `text-muted`),
   never a literal colour.
2. **The primitives** (`web/src/components/ui/`): `Button`, `Badge`,
   `SectionLabel`, `Stat` (a run metric chip: icon, figure, the detail in its tooltip), `Tile` (a dashboard figure: tinted icon, label, figure, detail; a tone bar when it counts something), `Card`, `Modal`, `ConfirmDialog`, `Section`, `Panel`,
   `Segmented`, `Tabs` (a page's sections, vertical or horizontal), `Menu`,
   `ChoiceMenu` (one value among grouped choices, each group under a tinted
   band; never a native `<select>` for a list of choices: the browser draws
   its list itself and ignores the theme tokens), and the
   fields (`TextInput`, `Select`, `TextArea`, `FieldRow`). Every modal and
   confirmation is one of these: a dialog's shell is `Modal`, a "are you sure"
   is `ConfirmDialog`, never a raw `<dialog>` overlay or a browser `confirm()`.
   A feature component picks a variant and adds only
   layout classes (width, margins, `shrink-0`); it never restyles a primitive's
   colours, paddings or font size. A new look is a new variant, here.

## Never dull

Calm is not grey. A screen made only of grey text on a grey panel reads as
dead: every list and grid shows the human where they are and what reacts.

- A list or a grid sits on a surface (`Card`), with a tinted header row, never
  loose rows on the page background.
- A row or tile the human can open reacts to the pointer with the accent: a
  tinted background (`bg-accent-soft`), an accent edge or border, its title in
  `text-accent-strong`; the whole row opens it, its actions appear on hover.
- Labels that carry a meaning (an agent, a state) are tinted badges, not plain text.
- A figure (a count, a cost, a share) is a `Tile` with a tone, `active` when it
  calls for attention: one tone per meaning, kept on every screen that shows it
  (the risks' unreviewed `err`, untested `attention`, churn `info`; usage, cost).
- A launchable action shown as a card (an analysis) is a whole clickable card:
  an icon chip, a title, a line, its call to action at the bottom, accent on hover.
- Row actions are icons with a tooltip: open is `soft` (the eye), delete is
  `danger` and always visible; an action secondary to the row (remedy one
  module) appears on hover and keeps the row's click from firing.
- An empty list is still its `Card`, with one centred line, not bare text.

## Hierarchy

- One primary action per view (`Button variant="primary"`); the rest are
  `secondary`, or `ghost` in toolbars and lists, the one action of each row
  included (a column of tinted buttons is noise); `soft` (accent-tinted) for a
  list's add button. A row's secondary choices (its `Menu`) show on hover or focus. Destructive actions are
  `danger` and say what they destroy.
- Buttons are icons (`iconOnly`) with a tooltip (`title`) that says what
  they do; a text label only for a view's single primary action and a
  dialog's confirm. Text on every button makes the UI heavy.
- State is shown by form as well as words: a `Badge` tone (ok, err,
  attention for "needs you", accent for "running"), a dot that pulses while
  something runs. The accent is for action, `attention` (amber) for what
  waits on the human, never `err`: an error and a question read apart.
- Borders: `border-border` separates (decorative, low contrast),
  `border-border-strong` bounds a component (a field, a secondary button, a
  pill): it is the one at 3:1 in every theme. What floats over the thread
  (the palette, the composer, a tooltip) uses `shadow-float` and `rounded-lg`.
- Cards only for what stands apart (a plan, a dialog, a question waiting for
  the human) and for a page that holds groups: the Settings page is one card,
  its tabs and its groups inside. Groups are `Section`s: a `SectionLabel`
  (description, actions such as a `+`), then the body, no frame or band of
  their own. A group saved on its own takes `save`: a dot before its label
  while it has unsaved edits, and its own "Enregistrer · Au prochain tour".
  Lists are rows separated by space or a hairline (`border-border`); the
  entries of a group that are edited as a whole (a panel page, an MCP server)
  are `Card variant="inset"` blocks, never a bordered box inside a box.
- Only the accent gets a filled `Badge`; ok and err are coloured text.
  Text is sentence case; only `SectionLabel` is capitals with tracking: as an h2 (a page's group, bold, full text colour, over a hairline) it stands out from the fields; as an h3 (a panel, a card part) it stays small and muted.
- Radii are small (2 to 6px) and come from the tokens; no pills.
- Text: `text-sm` for reading, `text-xs` for controls and metadata,
  `SectionLabel` for group titles. Numbers that line up use `tabular-nums`.
- Provenance is visible (rule 3 of AGENTS.md): verified results and model
  output never share a look.
- Side columns are stacks of `Panel` blocks split by a hairline, each
  header (chevron, title, summary when folded) always visible.

## Sizes

- Controls are `sm` (28px) in toolbars, cards and panels; `md` (32px) for a
  page's main action. Icon buttons are square (`iconOnly`).
- Spacing comes from `gap` on flex/grid parents, not margins between
  siblings.
