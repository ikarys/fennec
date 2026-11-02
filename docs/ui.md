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

## Hierarchy

- One primary action per view (`Button variant="primary"`); the rest are
  `secondary`, or `ghost` in toolbars and lists, the one action of each row
  included (a column of tinted buttons is noise); `soft` (accent-tinted) for a
  list's add button. A row's secondary choices (its `Menu`) show on hover or focus. Destructive actions are
  `danger` and say what they destroy.
- Buttons are icons (`iconOnly`) with a tooltip (`title`) that says what
  they do; a text label only for a view's single primary action and a
  dialog's confirm. Text on every button makes the UI heavy.
- State is shown by form as well as words: a `Badge` tone (ok, err, accent
  for "needs you" or "running"), a dot that pulses while something runs.
  The accent is for action and attention, not decoration.
- Cards only for what stands apart (a plan, a dialog, a question waiting for
  the human). A settings page's groups are `Section`s: a bordered block with
  a tinted header band (title, description, actions such as a `+`). Inside,
  everything is flat: lists are rows separated by space or a hairline
  (`border-border`), never a box per item and never a box inside a box.
- Only the accent gets a filled `Badge`; ok and err are coloured text.
  Labels are sentence case, never uppercase with tracking.
- Radii are small (2 to 6px) and come from the tokens; no pills.
- Text: `text-sm` for reading, `text-xs` for controls and metadata,
  `SectionLabel` for group titles. Numbers that line up use `tabular-nums`.
- Provenance is visible (rule 3 of AGENTS.md): verified results and model
  output never share a look.
- Side columns are stacks of `Panel` blocks whose header is a tinted band, each
  header (chevron, title, summary when folded) always visible.

## Sizes

- Controls are `sm` (28px) in toolbars, cards and panels; `md` (32px) for a
  page's main action. Icon buttons are square (`iconOnly`).
- Spacing comes from `gap` on flex/grid parents, not margins between
  siblings.
