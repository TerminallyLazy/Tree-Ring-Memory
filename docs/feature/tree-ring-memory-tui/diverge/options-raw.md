# Tree Ring Memory TUI Options

## HMW Question

How might we give humans and AI agents a fast, emotionally clear terminal view of memory health, ring composition, and lifecycle actions without turning Tree Ring Memory into a transcript browser?

## SCAMPER Options

### Option 1: Ring-First Observatory

**Core idea**: The user opens `tree-ring tui` and sees a live animated ASCII tree-ring core with secondary panes for search and details.
**Key mechanism**: Ratatui renders the ring visualization as the dominant state instrument, fed by store-watch and event-stream updates.
**Key assumption**: Users need fast confidence in memory health before they need dense editing workflows.
**SCAMPER origin**: Substitute.
**Closest competitor**: Terminal dashboards like `btop`, but memory-lifecycle specific.

### Option 2: Operator Console

**Core idea**: The user works primarily through search, details, and action panels while the ring visualization sits as a persistent status sidebar.
**Key mechanism**: Ratatui tables, detail panes, command palette, and confirmation modals carry most of the experience.
**Key assumption**: Most users enter the TUI to act on memories, not admire memory state.
**SCAMPER origin**: Combine.
**Closest competitor**: Database/browser TUIs that pair a list, details, and command actions.

### Option 3: Retro Ring Console

**Core idea**: The TUI borrows from retro roller-rink signage: saturated ring colors, pulsing ASCII arcs, and glowing memory states.
**Key mechanism**: Color and pulse intensity encode memory category, count, recency, and severity.
**Key assumption**: Emotional affordance improves operator understanding without sacrificing terminal efficiency.
**SCAMPER origin**: Adapt.
**Closest competitor**: Stylized TUIs like Lazygit themes, but driven by semantic memory state.

### Option 4: Exploded Ring Inspector

**Core idea**: A slash command breaks the live core into separated rings with floating data bubbles for counts, sensitivity, scars, seeds, and freshness.
**Key mechanism**: The app has a dedicated expanded visualization mode with animated offsets and focused ring panels.
**Key assumption**: Users sometimes need a memorable, inspectable cross-section more than a compact dashboard.
**SCAMPER origin**: Modify/Magnify.
**Closest competitor**: System topology views, adapted to temporal memory rings.

### Option 5: Agent Harness Overlay

**Core idea**: Agent frameworks can stream live memory lifecycle events into the TUI so the ring pulses as agents remember, recall, redact, or consolidate.
**Key mechanism**: A local framework-agnostic event stream is merged with persisted SQLite state.
**Key assumption**: AI-agent operators benefit from seeing memory behavior while work is happening, before every event has settled into storage.
**SCAMPER origin**: Put to other use.
**Closest competitor**: Live log/event consoles, but interpreted through memory lifecycle state.

### Option 6: Minimal Safe Actions

**Core idea**: Remove most write actions from v1 and keep only read/search plus redaction/forget.
**Key mechanism**: Lower action surface reduces safety risk.
**Key assumption**: Users prefer correctness over complete terminal operation.
**SCAMPER origin**: Eliminate.
**Closest competitor**: Read-mostly admin dashboards.

### Option 7: Memory Interview Mode

**Core idea**: The TUI asks the user what they want to do, then builds a guided memory operation backwards from intent.
**Key mechanism**: Command flow starts with desired outcome and only then picks recall, remember, redact, promote, or forget.
**Key assumption**: Users know the job they want done better than they know memory lifecycle commands.
**SCAMPER origin**: Reverse.
**Closest competitor**: Wizard-style terminal apps.

## Crazy 8s Supplements

### Option 8: Timeline Strip

**Core idea**: A horizontal time strip shows recent cambium and older rings as compressing segments.
**Key mechanism**: The ring visual is paired with a temporal sparkline.
**Key assumption**: Memory aging needs a time metaphor as well as a ring metaphor.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Git history timelines.

### Option 9: Memory Health HUD

**Core idea**: A compact heads-up display scores memory health: fresh, stable, scar-heavy, seed-heavy, sensitive, stale, or contradictory.
**Key mechanism**: Aggregated metrics become a concise live label above the ring core.
**Key assumption**: Users need one glanceable health summary before deeper drilldown.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: CI status dashboards.

### Option 10: Agent Pairing Console

**Core idea**: The TUI exposes one pane for human actions and one pane for agent event-stream activity.
**Key mechanism**: Store-watch and event-stream inputs are visually separated but reconciled into one ring state.
**Key assumption**: Human and agent memory actions need separate accountability in the same terminal.
**SCAMPER origin**: Crazy 8s supplement.
**Closest competitor**: Pair-programming terminal dashboards.

## Curated 6

1. **Ring-First Observatory**: Different mechanism, behavior assumption, and cost profile from the action-heavy options.
2. **Operator Console**: Dense action workflow and lower visual emphasis.
3. **Exploded Ring Inspector**: Dedicated visual inspection mode.
4. **Agent Harness Overlay**: Event-stream integration for framework-agnostic live updates.
5. **Memory Health HUD**: Glanceable state summary.
6. **Memory Interview Mode**: Guided action model for safer writes.

## Selected Direction

The selected direction is a **Dual-Mode Ring Console**:

- Default view combines Ring-First Observatory, Operator Console, Agent Harness Overlay, and Memory Health HUD.
- `/rings` opens the Exploded Ring Inspector.
- Write actions use a constrained version of Memory Interview Mode through guided confirmations.

## Eliminated Or Merged Options

- **Retro Ring Console** is not standalone; it becomes the visual language for all selected modes.
- **Minimal Safe Actions** conflicts with the user's selected full operator console, but its safety concern remains as confirmations for destructive, sensitive, or authority-changing actions.
- **Timeline Strip** is deferred to a later view after the ring dashboard, exploded rings, search, and actions are solid.
