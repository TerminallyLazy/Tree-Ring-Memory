# IndexNow Submission

Target: <https://api.indexnow.org/indexnow>

Status: Submitted on 2026-07-07.

Evidence:
<https://github.com/TerminallyLazy/Tree-Ring-Memory/issues/26#issuecomment-4909613838>

## Spec Basis

- IndexNow documentation: <https://www.indexnow.org/documentation>
- Bing setup guide: <https://www.bing.com/indexnow/getstarted>

IndexNow allows bulk URL submission with `host`, `key`, optional
`keyLocation`, and `urlList`. Because Tree Ring Memory is served from a
GitHub Pages project path, the key file is hosted inside the same host and the
request includes `keyLocation`.

## Key File

Key:
`0bfd1a9f8a68e38d590817b8ea1ed1e2`

Live key URL:
<https://terminallylazy.github.io/Tree-Ring-Memory/0bfd1a9f8a68e38d590817b8ea1ed1e2.txt>

Validation before submission:

- The key URL returned HTTP `200`.
- The response body matched the key.

## Submitted Payload

Endpoint response: HTTP `202`.

IndexNow documents HTTP `202` as URL received with key validation pending.

Submitted URLs:

- <https://terminallylazy.github.io/Tree-Ring-Memory/>
- <https://terminallylazy.github.io/Tree-Ring-Memory/launch/tree-ring-memory-framework.md>
- <https://terminallylazy.github.io/Tree-Ring-Memory/launch/rust-native-agent-memory-cli.md>
- <https://terminallylazy.github.io/Tree-Ring-Memory/press-kit.md>
- <https://terminallylazy.github.io/Tree-Ring-Memory/llms.txt>
- <https://terminallylazy.github.io/Tree-Ring-Memory/feed.xml>
- <https://terminallylazy.github.io/Tree-Ring-Memory/sitemap.xml>

## Follow-Up

- Monitor crawl and indexing in Bing Webmaster Tools if the owner connects the
  property.
- Re-submit only when launch URLs change or new public campaign pages are
  published.
