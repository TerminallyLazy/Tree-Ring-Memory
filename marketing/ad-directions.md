# Tree Ring Memory Ad Directions

These are deterministic campaign directions based on the existing brand assets:
warm cream background, deep navy type, teal/coral/orange/gold accents, visible
tree-ring metaphor, and terminal-native proof. Use the files in
`marketing/assets/` as the source of truth for dimensions.

## Structured Prompt Pack

The Creative Production Ads Explorer prompt pack was prepared without image
generation so the campaign can generate a full 12-family software-product ad
review later if needed.

Rebuild command:

```bash
python3 /Users/lazy/.codex/plugins/cache/openai-curated-remote/creative-production/0.1.24/skills/ads-explorer/scripts/build_ads_explorer.py \
  --ad-name "Tree Ring Memory Framework" \
  --pack digital-product-core-ad-prompts \
  --subject-kind digital-product \
  --ad-brief "Developer-tool launch for Tree Ring Memory Framework. Product: framework-agnostic, local-first memory lifecycle layer for AI agents. Proof object: Rust CLI and TUI, SQLite/FTS recall, forgetting, audit, consolidation, DOX/Revolve adapters, Homebrew install, GitHub repo, Pages landing, release v0.11.0. Audience: AI-agent builders, Rust CLI developers, local-first developers, coding-agent power users. Channels: Hacker News Show HN, Reddit r/rust, Reddit r/LocalLLaMA, X launch thread, YouTube thumbnail, developer directories. Visual style: use existing @assets style and color scheme: tree rings, dark terminal surface, warm amber/gold accents, rust/orange highlights, bark/wood texture references, precise CLI proof, not generic AI chatbot imagery. Copy constraints: use only supplied phrases: 'Tree Ring Memory', 'Local-first memory for AI agents', 'Recall. Forget. Audit.', 'Rust CLI + TUI', 'SQLite/FTS'. Avoid fake metrics, fake reviews, fake customer logos, fake awards, fake pricing, or invented integrations." \
  --out-dir outputs/marketing/tree-ring-memory-digital-product-ads \
  --force
```

Current local output:

- `outputs/marketing/tree-ring-memory-digital-product-ads/prompts-manifest.json`
- `outputs/marketing/tree-ring-memory-digital-product-ads/jobs.jsonl`
- `outputs/marketing/tree-ring-memory-digital-product-ads/review-board.html`
- `outputs/marketing/tree-ring-memory-digital-product-ads/moodboard-widget-payload.json`

Strongest first-pass families from the 12-direction prompt wall:

- `product-proof-crop`: Show HN, directory listings, and PR comments where
  the terminal/storage proof matters more than atmosphere.
- `feature-mechanic-spotlight`: Reddit and Rust-community posts around recall,
  forgetting, audit, and consolidation.
- `digital-product-launch-hero`: X, LinkedIn, Product Hunt, and launch-page
  social cards.
- `screen-detail-macro`: YouTube thumbnail variants and short clips where the
  CLI/TUI needs to be legible.

## Checked-In Campaign Cards

The current launch campaign also includes deterministic cards generated from
the checked-in brand assets with:

```bash
python3 marketing/scripts/build-campaign-cards.py
```

- `marketing/assets/homebrew-install-card-1200x675.png`: Homebrew tap and install.
- `marketing/assets/rust-article-card-1200x675.png`: Rust-native CLI article.
- `marketing/assets/twir-submission-card-1200x675.png`: This Week in Rust PR.
- `marketing/assets/not-transcript-dump-card-1080x1080.png`: square social proof card.
- `marketing/assets/terminal-trove-preview-1200x675.png`: Terminal Trove tool
  submission preview card.

## Direction 1: Agent Memory Should Age

- Format: launch hero, X card, Product Hunt gallery frame.
- Headline: `Agent memory should age.`
- Support: `Fresh context, compressed rings, scars, heartwood, and seeds.`
- Visual: tree-ring hero with a small terminal strip showing `tree-ring recall`.
- CTA: `Try the Rust CLI`
- Asset base: `marketing/assets/open-graph-1200x675.png`

## Direction 2: Not A Transcript Dump

- Format: HN image preview, LinkedIn image, Reddit link card.
- Headline: `Not a transcript dump.`
- Support: `A lifecycle layer for AI agent memory.`
- Visual: split composition: noisy transcript stack on the left, clean ring
  layers on the right.
- CTA: `See the launch preview`
- Asset base: `marketing/assets/reddit-link-card-1600x900.png`

## Direction 3: Scars Prevent Regressions

- Format: short social post image.
- Headline: `Scars are memory too.`
- Support: `Keep the failures that prevent repeated mistakes.`
- Visual: dark navy terminal block over a ring scar mark in coral.
- CTA: `Record evidence`
- Asset base: `marketing/assets/social-square-banner-1080x1080.png`

## Direction 4: Heartwood For Durable Truths

- Format: blog header, newsletter image.
- Headline: `Heartwood for durable truths.`
- Support: `Promoted evidence becomes memory agents can trust later.`
- Visual: central gold/navy heartwood ring with a small evidence command.
- CTA: `Read the model`
- Asset base: `marketing/assets/open-graph-1200x675.png`

## Direction 5: Local-First Agent Memory

- Format: Reddit and local-AI communities.
- Headline: `Local-first agent memory.`
- Support: `SQLite/FTS, explicit writes, audit, redaction, and forgetting.`
- Visual: laptop terminal in navy on cream, with teal filesystem ring marks.
- CTA: `Run it locally`
- Asset base: `marketing/assets/reddit-link-card-1600x900.png`

## Direction 6: Explainable Recall

- Format: developer blog inset, X thread image.
- Headline: `Recall should explain itself.`
- Support: `Ring, scope, confidence, ranking signals.`
- Visual: recall result rows arranged like tree rings, no fake metrics.
- CTA: `Inspect memory`
- Asset base: `marketing/assets/social-square-banner-1080x1080.png`

## Direction 7: Rust-Native Runtime

- Format: r/rust supporting image, newsletter preview.
- Headline: `Rust-native memory runtime.`
- Support: `CLI, crates, local storage, and terminal TUI.`
- Visual: crisp terminal command grid with orange and teal ring accents.
- CTA: `Install v0.11.0`
- Asset base: `marketing/assets/open-graph-1200x675.png`

## Direction 8: Forgetting Is A Feature

- Format: privacy/local-first image.
- Headline: `Forgetting is a feature.`
- Support: `Delete, redact, supersede, audit, and consolidate.`
- Visual: ring segment being cleanly pruned, not erased messily.
- CTA: `Audit memory`
- Asset base: `marketing/assets/social-square-banner-1080x1080.png`

## Direction 9: Bridge The Agent Stack

- Format: integration announcement.
- Headline: `Memory that travels between agent tools.`
- Support: `DOX, Revolve, framework discovery, and adapter-first design.`
- Visual: tree-ring core with small connector labels around it.
- CTA: `Request an adapter`
- Asset base: `marketing/assets/open-graph-1200x675.png`

## Direction 10: Terminal Operator Console

- Format: YouTube thumbnail variant.
- Headline: `Agent memory, visible.`
- Support: `Recall, audit, evidence, maintenance, TUI.`
- Visual: high-contrast terminal console over the tree-ring background.
- CTA: none; thumbnail should stay uncluttered.
- Asset base: `marketing/assets/youtube-thumbnail-1920x1080.png`

## Direction 11: Show HN Proof

- Format: HN support image for X/LinkedIn after posting.
- Headline: `Show HN: Tree Ring Memory`
- Support: `Memory lifecycle for AI agents.`
- Visual: simple launch card with repo URL and v0.11.0 badge.
- CTA: `Join the discussion`
- Asset base: `marketing/assets/open-graph-1200x675.png`

## Direction 12: Product Hunt Gallery

- Format: Product Hunt image set.
- Headline set:
  - `Memory should age`
  - `Local-first Rust CLI`
  - `Explainable recall`
  - `Audit and forgetting`
  - `Adapters for agent workflows`
- Visual: five-frame gallery using consistent ring/terminal motif.
- CTA: `Try Tree Ring Memory`
- Asset base: `marketing/assets/open-graph-1200x675.png`

## Copy Safety

- Do not claim production stability beyond `protocol-preview` or `launch
  preview`.
- Do not imply hosted sync or automatic transcript capture.
- Do not invent benchmark numbers, user counts, endorsements, awards, or
  compatibility guarantees.
- Keep final type deterministic. Generated images may be used only as
  background or visual direction unless text is replaced manually.
