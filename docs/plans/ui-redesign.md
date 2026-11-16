# UI/UX redesign plan

Status: proposal (2026-11-17), scheduled for the release after 0.2.0.
Scope: move the editor onto the Operator design system ([../../../operator-design-system](../../../operator-design-system)) and fix the UX problems found on the way, not a cosmetic reskin.

## 1. Findings

### 1.1 Node cards

* **Field width goes to the wrong column.**
  On input rows the inline value has a fixed width (`.inline-value` is `13 DU`, `ticknode-editor/styles/nodes.css`), while the label takes the rest of the row (`row_widths` in `ticknode-editor/src/geometry/layout/width.rs` hands the input row everything the output does not use).
  A ten-character label therefore grows while a long value stays clipped in a narrow field.
  Property rows behave differently (`.prop-row` label `flex: 0 1 auto`, field `flex: 1 0 72px`), so two rows of the same node do not align.
* **Resizing grows empty fields.**
  `NodeLayout::sized` (`ticknode-editor/src/geometry/layout.rs`) splits extra height in equal whole pixels over every growable String/Multiline row and every unconnected String input, whether it is empty or holds a long text.
* **Geometry is estimated, not measured.**
  Width comes from a character-advance constant (`CHAR_WIDTH = 12 * 1200 / 2048`) and hand-tuned chrome constants (`SELECT_CHROME`, `CHIP_CHROME`, `+ 28.0`, `+ 16.0`); heights come from Rust constants that must equal CSS values.
  This is why canvas fonts are raw `px` (79 of 109 `font-size` declarations in `styles/`), which breaks the design system's Type Unit rule and makes user text scaling impossible on the canvas.
* **Automatic width clamps to 240–480 px** and shrinks both pin columns proportionally at the cap instead of truncating the less important column.
* **One node, several layout systems**: property rows, pin rows, Match case blocks and template fields each have their own CSS and their own branch in `NodeLayout`.
  This matches the earlier "node as a stack of rows" idea that was postponed for discussion.

### 1.2 Editor shell

* `ticknode-editor/src/ui.rs` re-implements design-system components (`ActionButton`, `IconButton`, `TextField`, `Switch`, `Terminal`, the `ControlState` flash) instead of using `operator-ui`; they have already drifted from the specifications.
* Tokens are copied by hand into `styles/base.css` instead of being generated from [tokens.toml](../../../operator-design-system/tokens/tokens.toml).
* There are 163 raw `px` values in `styles/*.css`.
* The right inspector duplicates what the node already shows; the stated goal is to configure everything on the node itself.
* Pickers for large catalogs (20 000 HA entities) are custom, and the design-system Combobox cannot replace them yet (see 2).
* Keyboard and accessibility on the canvas are minimal (one `tabindex`, no ResizeObserver, no focus model for nodes and pins).
* Missing canvas conveniences: minimap, find node, zoom to selection, alignment guides.

## 2. Design system gap analysis

The design system targets application screens: its controls are `10–12 DU` tall (TextField `row_standard`, DropdownMenu options `row_control`), while node rows are `6 DU` with `4.5 DU` fields.
There is no density concept, which is the root gap.

### 2.1 Add to the design system core

These are generic and useful to any Operator app.

| Need | Today | Change |
| :--- | :--- | :--- |
| Dense controls | Fixed heights per component | A `density` foundation (`comfortable` 10 DU, `compact` 8 DU, `dense` 6 DU rows) honoured by every field, select, switch, chip and menu. |
| Anchored floating surfaces | Each popup positions itself | A `Popover` primitive (anchor, flip, viewport clamp, outside-press, focus return) reused by Menu, DropdownMenu, Combobox, Tooltip and completion lists. |
| Large pickers | Combobox takes `list of strings`, no icons, no virtualization | Rich options (icon, label, detail, group), virtualized list, caller-driven async filtering, typed free input. |
| Context menu | Menu exists | Submenus, shortcut column (`Kbd`), disabled reasons. |
| Number with unit | Missing | `NumberField` with optional unit select and step; covers quantities. |
| Duration, time, date-time | Missing | `DurationField` (signed option), `TimeField`, `DateTimeField` with a calendar popover. |
| Color | Missing | `ColorField` with a swatch palette (frame colors, type colors). |
| Multi-select | CheckboxGroup tiles are too large | `ChipGroup` / multi-select chips in dense density (weekdays). |
| Tree | Accordion only | `Tree` with lazy children, search highlight and keyboard navigation (resources, value trees, JSON). |
| Inline rename | Missing | `EditableText` (click or F2 to edit, Enter/Escape, pending/error states). |
| Navigation rail | Home page has its own rail | `NavigationRail` that becomes top tabs below the narrow breakpoint. |
| Command palette | Quick add is custom | `CommandPalette` (ranked search, recents, sections). |
| Collapsible side panel | SplitPane exists | Collapsible/overlay mode for the narrow layout. |
| Finish drafts | PropertyList, Toolbar, DataTable are Draft | Review them; PropertyList gets editable rows. |

### 2.2 Separate extension crates in the design system repository

Reusable by other apps, but too specialised for the core; each is an optional crate next to `web/operator-ui` that depends on it.

* **`operator-ui-code`**: a code editor field built on the existing CodeBlock highlighter (arborium): syntax layer, completion popover, lint underlines with hover messages, hover cards, undo integration.
  TickNode's template and formula fields become thin configurations of it.
* **`operator-ui-graph`**: node-graph editing: camera (pan, zoom, fit), grid, selection box, node card shell with the row stack (3.1), sockets, wires with route points and labels, frames, minimap, alignment guides.
  It knows nothing about TickNode types or the engine.

### 2.3 Stays in TickNode

Type colors and type badges, quantity/identifier formatting, the wire suggestion menu, Match condition editor content, HA and plugin-specific pickers, plugin settings forms generated from manifests.

## 3. Target design

### 3.1 Node card as a row stack

* A node is a vertical stack of rows described once: `Vec<RowSpec { kind, height: Fixed | Content { min, max } }>`.
  The same description drives rendering and socket positions, so Rust/CSS constant pairs disappear.
* Fixed rows (header, pins, selects, switches) use DU heights; content rows (text, templates, lists) report their measured height through a ResizeObserver, and socket positions follow.
  Measured heights also make Type Units usable on the canvas: larger text grows rows instead of breaking wires.
* Widths are measured with the loaded font (`measureText` or a measured probe) instead of character constants.
* Every row with a label and a field uses one two-column grid per node: the label column is `max-content` of the labels, capped (about 40% of the node or `30 DU`); the field column takes the rest.
  Extra width from resizing always goes to the field column.
* Extra height goes only to content rows that need it, proportionally to their content (wrapped lines), never to empty fields; a field can also be expanded on its own.
* Row kinds become shared components: pin row, property row, inline value, template, formula, list editor, case block, status row.
* Match and template nodes stop having separate layout branches.

### 3.2 Panels and pages

* Remove the right inspector; descriptions, last execution and actions move into node popovers (header info, trace on hover/click) and a compact bottom drawer for the selected node.
* Left panel (Nodes, Resources, Variables) uses Tabs, SearchField, the virtualized list and Tree.
* Home page: NavigationRail, Sheets as ListItem cards, Variables and Usage as DataTable, Plugins with PropertyList-based forms, Log on ConsoleTerminal.
* Results and errors of user actions follow the state model (Pending blocks, results do not, errors roll back) and use Toast for non-local feedback.

### 3.3 Canvas UX

Minimap, find node (Ctrl+F over titles and values), zoom to selection, alignment guides while dragging, keyboard focus for nodes and pins (Tab order, Enter to edit, arrows already move), touch gestures checked on tablets.

## 4. Phases

Each phase is a shippable release step; later phases depend on earlier ones.

1. **Foundation.** Depend on `operator-ui` and `operator-ui-tokens`; generate editor tokens instead of mirroring them; replace the `ui.rs` copies; convert shell CSS to DU/TU (canvas stays as is until phase 4).
2. **Design system core additions.** Density, Popover, rich virtualized Combobox, NumberField with unit, Duration/Time/DateTime, Color, ChipGroup, Tree, EditableText, NavigationRail, CommandPalette; finish PropertyList, Toolbar, DataTable.
   Each needs a spec, demo page and the maintainer's Draft → Reviewed check, so this phase runs in the design system repository in parallel with phase 3.
3. **Shell migration.** Home page, left panel, plugin cards, settings, console, dialogs and menus on design-system components; remove the inspector.
4. **Node row stack.** `RowSpec` layout with measured content rows, the two-column grid, content-proportional growth, TU fonts on the canvas; layout tests move from pixel constants to row descriptions.
   This is the largest UX win and the riskiest change.
5. **Extensions.** Extract `operator-ui-code` (template/formula fields) and `operator-ui-graph` (canvas, wires, frames, minimap) into the design system repository.
6. **Polish.** Keyboard and accessibility pass, touch, empty states, find/minimap/guides, the README screenshot.

## 5. Risks

* Wire positions depend on the node layout; phase 4 must keep the graph usable at every step (feature flag or one node family at a time).
* Hundreds of nodes with measured rows: batch ResizeObserver callbacks into one layout update per frame.
* Draft components need the maintainer's review before TickNode can rely on them.
* WASM size grows with `operator-ui`; enable only the needed component features.

## 6. Open questions

* Should the extension crates live in the design system repository (shared) or in TickNode until a second app needs them?
* Inspector removal: is a bottom drawer for the selected node acceptable, or should everything live in popovers?
* Canvas text scaling: follow the browser text size (TU) or only the canvas zoom?
