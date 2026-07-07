# DevPages Submission

Source: <https://devpages.io/submit-a-tool>

DevPages is a curated developer tools directory. Its submit page asks for tool
name, website URL, description, category, pricing model, GitHub URL, and contact
email. The FAQ says community submissions are reviewed by the team.

Current status: submission email sent to `contact@devpages.io` with
`marketing/assets/open-graph-1200x675.png` attached. Await review response or
public listing.

## Submission Path Note

The public submit form was inspected on 2026-07-07. The loaded client bundle
shows the submit handler calls `preventDefault()` and then flips the local page
state to the thank-you view without a `fetch()` or API call. The submission was
therefore sent to the published contact address instead of relying on the form.

## Listing Fields Sent

Tool Name:

```text
Tree Ring Memory
```

Website URL:

```text
https://terminallylazy.github.io/Tree-Ring-Memory/
```

Category:

```text
AI/ML Tools or Generative AI
```

Pricing Model:

```text
Open Source
```

GitHub URL:

```text
https://github.com/TerminallyLazy/Tree-Ring-Memory
```

Description:

```text
Tree Ring Memory is a framework-agnostic, local-first memory lifecycle layer
for AI agents. It is Rust-native and ships with a CLI, SQLite/FTS recall,
audit, forgetting, deterministic consolidation, source-linked evidence,
DOX/Revolve adapters, framework discovery, and a Ratatui terminal console.
```

Useful links sent:

- Website: <https://terminallylazy.github.io/Tree-Ring-Memory/>
- Repo: <https://github.com/TerminallyLazy/Tree-Ring-Memory>
- Release: <https://github.com/TerminallyLazy/Tree-Ring-Memory/releases/tag/v0.11.0>
- Press kit: <https://terminallylazy.github.io/Tree-Ring-Memory/press-kit.md>
- Launch feedback: <https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26>
