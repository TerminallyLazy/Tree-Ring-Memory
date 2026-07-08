# Social Handle Sweep - 2026-07-08

Status: owner-action input for account creation

Checked at: `2026-07-08T10:18:21Z`

This sweep checks only public profile endpoints or public handle-resolution
endpoints. It does not reserve handles, bypass login, solve CAPTCHA, accept
terms, or prove final signup availability. Treat `not found` as a good owner
signup target, not as a guarantee.

## Results

| Platform | Target Handle | Public Lookup | Result |
| --- | --- | --- | --- |
| Hacker News | `TreeRingMemory` | `https://news.ycombinator.com/user?id=TreeRingMemory` | Inconclusive: unauthenticated lookup returned HTTP `429`. |
| Reddit | `TreeRingMemory` | `https://www.reddit.com/user/TreeRingMemory/about.json` | Inconclusive: unauthenticated lookup returned HTTP `403`. |
| X | `TreeRingMemory` | `https://x.com/TreeRingMemory` | Public profile lookup returned HTTP `404`; attempt this handle first. |
| YouTube | `@TreeRingMemory` | `https://www.youtube.com/@TreeRingMemory` | Public handle lookup returned HTTP `404`; attempt this handle first. |
| Bluesky | `treeringmemory.bsky.social` | `https://public.api.bsky.app/xrpc/com.atproto.identity.resolveHandle?handle=treeringmemory.bsky.social` | Inconclusive: public resolver returned HTTP `400`; check in signup. |
| Mastodon / Hachyderm | `@TreeRingMemory@hachyderm.io` | `https://hachyderm.io/@TreeRingMemory` | Public profile lookup returned HTTP `404`; attempt this handle first. |
| LinkedIn | `tree-ring-memory` | `https://www.linkedin.com/company/tree-ring-memory/` | Public company-page lookup returned HTTP `404`; attempt this public URL first. |
| Product Hunt | `TreeRingMemory` | `https://www.producthunt.com/@TreeRingMemory` | Public profile lookup returned HTTP `404`; attempt this handle first. |
| Dev.to | `TreeRingMemory` | `https://dev.to/treeringmemory` | Public profile lookup returned HTTP `404`; attempt this handle first. |
| Hashnode | `treeringmemory` | `https://treeringmemory.hashnode.dev/` | Public publication lookup returned HTTP `404`; attempt this handle first. |
| Medium | `@TreeRingMemory` | `https://medium.com/@TreeRingMemory` | Inconclusive: unauthenticated lookup returned HTTP `403`. |
| Substack | `treeringmemory` | `https://treeringmemory.substack.com/` | Public publication lookup returned HTTP `404`; attempt this subdomain first. |

## Owner Signup Order

1. Reserve `TreeRingMemory` on X and `@TreeRingMemory` on YouTube first. Both
   returned public `404` responses and are high-visibility handles.
2. Reserve developer-community profiles next: Dev.to, Hashnode, Hachyderm, and
   Product Hunt.
3. Create the LinkedIn company page with public URL `tree-ring-memory`.
4. Check Hacker News, Reddit, Bluesky, and Medium inside their signup flows;
   public unauthenticated checks were inconclusive.
5. After each account is live, record only the public profile URL in
   `marketing/social-profiles.json`.

## Account Creation Boundary

The owner must complete signup directly for platforms that require email,
phone, CAPTCHA, identity verification, SSO, payment state, or terms acceptance.
Do not store passwords, recovery codes, OAuth tokens, phone numbers, or private
email addresses in this repository.
