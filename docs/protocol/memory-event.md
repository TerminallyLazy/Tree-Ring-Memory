# Memory Event Protocol

`MemoryEvent` is the portable unit of Tree Ring Memory.

The event is not a transcript line. It is a meaningful memory statement with scope, ring, source evidence, confidence, salience, sensitivity, retention, and review state.

## Rings

- `cambium`: fresh active memory
- `outer`: recent summarized memory
- `inner`: older compressed memory
- `heartwood`: durable truths
- `scar`: important negative lessons
- `seed`: unresolved future possibilities

## Recall Defaults

Recall excludes sensitive and superseded memory unless explicitly requested. Results should include source evidence and ranking explanation when `explain_ranking` is true.

## Privacy Defaults

Secrets are blocked by default. Sensitive memory is excluded from recall and export by default.
