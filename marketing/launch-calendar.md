# Tree Ring Memory Launch Calendar

This is the execution sequence for the first public campaign. Keep the release,
website, discussion, and feedback issue as the canonical links.

## Day 0: Public Surface Hardening

Status: complete.

- GitHub repository metadata and topics live.
- GitHub Pages launch page live.
- v0.11.0 release live.
- Launch feedback issue live.
- GitHub Discussion #27 live.
- Press kit, `llms.txt`, sitemap, robots, and Atom feed live or staged.
- YouTube upload package built locally.

## Day 1: Technical Launch

Goal: attract builders who will critique the model and try the CLI.

1. Post Hacker News Show HN.
2. Pin the X launch thread.
3. Publish the YouTube demo.
4. Reply to every substantive HN/X/YouTube comment for two hours.
5. Log public URLs in `marketing/submission-ledger.csv`.
6. Route bugs and concrete requests to issue #26 or Discussions #27.

Success signals:

- At least 5 technical comments.
- At least 2 install attempts or bug reports.
- At least 1 adapter request with concrete workflow details.

## Day 2: Rust And Local-Agent Communities

Goal: get implementation feedback rather than broad AI hype.

1. Post to `r/rust` if rules allow.
2. Post to `r/LocalLLaMA` if rules allow.
3. Post to `r/opensource` if rules allow.
4. Share a short Bluesky/Mastodon launch note.
5. Keep replies specific to SQLite/FTS, Rust CLI, privacy, and adapters.

Success signals:

- Rust critique on storage and CLI shape.
- Local-agent feedback on privacy and recall ergonomics.
- One issue or discussion opened by someone else.

## Day 3: Durable Explainers

Goal: create searchable long-form surfaces.

1. Publish "Why AI Agent Memory Should Age Like Tree Rings" on DEV.
2. Mirror to Hashnode or Medium with canonical links where possible.
3. Share the article back to X/Bluesky/Mastodon.
4. Submit short pitches to Rust and AI engineering newsletters.

Success signals:

- One newsletter or directory response.
- Searchable long-form URL live.
- Article links back to release and discussion.

## Day 4: Direct Outreach

Goal: seed the project into relevant lists and maintainers' awareness.

1. Identify active awesome-agent, Rust CLI, and local-first lists.
2. Open small PRs or issues using `marketing/newsletter-pitches.md`.
3. DM or email maintainers only where contact is public and relevant.
4. Avoid asking for votes; ask for fit or feedback.

Success signals:

- Three directory/list submissions.
- Two maintainer replies.
- One accepted listing or actionable rejection.

## Day 5: Product Hunt Prep

Goal: turn the campaign into a broader launch only after proof exists.

1. Confirm YouTube demo is public.
2. Prepare Product Hunt gallery from existing assets.
3. Use the maker comment in `marketing/channel-playbook.md`.
4. Draft first comment and FAQ answers from `marketing/reply-bank.md`.
5. Launch only when the owner can monitor comments.

Success signals:

- Product page draft ready.
- Maker account verified.
- Public video and release links attached.

## Daily Maintenance

- Update `marketing/submission-ledger.csv` after every live post.
- Add public profile URLs to `marketing/social-profiles.json`.
- Keep issue #26 for concrete bugs and feature requests.
- Keep Discussion #27 for broader launch conversation.
- Convert repeated objections into README/FAQ improvements.
- Do not store account secrets, backup codes, or contact details in the repo.
