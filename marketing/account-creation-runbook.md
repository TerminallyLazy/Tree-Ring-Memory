# Account Creation Runbook

Status: ready for owner-side signup

Last updated: 2026-07-08

This runbook turns `account-setup.md` into an execution order. It intentionally
does not store passwords, recovery codes, phone numbers, email aliases, OAuth
tokens, or private contact details.

Public unauthenticated handle checks are recorded in
`social-handle-sweep-2026-07-08.md`. Use them as a signup starting point, not
as proof that a platform will reserve the handle after verification.

## Owner-Side Ground Rules

- Create accounts from an owner-controlled browser session.
- Use a password manager and unique generated passwords.
- Complete email, CAPTCHA, phone, identity, and SSO verification directly.
- Enable two-factor authentication where available.
- Save recovery codes in the password manager only.
- After each account exists, record only the public profile URL in
  `marketing/social-profiles.json`.

## Priority Order

### 1. YouTube

- URL: `https://studio.youtube.com/`
- Channel name: `Tree Ring Memory`
- Handle target: `@TreeRingMemory`
- Fallback handles: `@TreeRingMem`, `@UseTreeRing`, `@TreeRingAI`
- Profile image: `marketing/assets/social-square-logo-1080x1080.png`
- Banner: `marketing/assets/youtube-channel-banner-2560x1440.png`
- Description source: `marketing/youtube/description.md`
- Upload source: `outputs/marketing/youtube-demo/tree-ring-memory-demo.mp4`
- Validation packet: `marketing/youtube-upload-validation.md`
- First action: upload unlisted, verify title, description, captions, tags,
  thumbnail, and end-screen link, then publish.

### 2. Hacker News

- URL: `https://news.ycombinator.com/login`
- Username target: `TreeRingMemory`
- Fallback usernames: `TreeRingMem`, `UseTreeRing`, `RingMemoryAI`
- About text source: `marketing/account-setup.md`
- Launch copy source: `marketing/channel-playbook.md`
- First action: submit the Show HN only from a credible maker or project
  account and disclose affiliation in the first comment.
- Do not ask for votes or coordinate voting.

### 3. Reddit

- URL: `https://www.reddit.com/login/`
- Username target: `TreeRingMemory`
- Fallback usernames: `TreeRingMem`, `UseTreeRing`, `TreeRingAI`
- Profile image: `marketing/assets/social-square-logo-1080x1080.png`
- Link card: `marketing/assets/reddit-link-card-1600x900.png`
- Bio source: `marketing/account-setup.md`
- First actions: create the profile, optionally reserve `r/TreeRingMemory`,
  then read current rules for each target subreddit before posting.
- Target communities: `r/rust`, `r/LocalLLaMA`, `r/opensource`,
  `r/commandline`, `r/AI_Agents`.

### 4. X

- URL: `https://x.com/i/flow/signup`
- Username target: `TreeRingMemory`
- Fallback usernames: `TreeRingMem`, `TreeRingAI`, `UseTreeRing`
- Display name: `Tree Ring Memory`
- Profile image: `marketing/assets/x-profile-400x400.png`
- Header: `marketing/assets/x-header-1500x500.png`
- Bio source: `marketing/account-setup.md`
- First action: post and pin the launch thread from `marketing/launch-kit.md`.

### 5. Bluesky

- URL: `https://bsky.app/`
- Handle target: `treeringmemory.bsky.social`
- Fallback handles: `treeringmem.bsky.social`, `usetreering.bsky.social`
- Display name: `Tree Ring Memory`
- Profile image: `marketing/assets/social-square-logo-1080x1080.png`
- First action: post the launch note from `marketing/launch-kit.md`.

### 6. Mastodon

- URL: `https://hachyderm.io/auth/sign_up`
- Handle target: `TreeRingMemory`
- Fallback handles: `TreeRingMem`, `UseTreeRing`
- Display name: `Tree Ring Memory`
- Profile image: `marketing/assets/social-square-logo-1080x1080.png`
- First action: post the launch note from `marketing/launch-kit.md`.

### 7. LinkedIn

- URL: `https://www.linkedin.com/company/setup/new/`
- Page name: `Tree Ring Memory`
- Public URL target: `tree-ring-memory`
- Logo: `marketing/assets/social-square-logo-1080x1080.png`
- Tagline: `Framework-agnostic memory lifecycle for AI agents.`
- Description source: `marketing/account-setup.md`
- First action: post the launch update from `marketing/launch-kit.md`.

### 8. Developer Blogs

- Dev.to URL: `https://dev.to/enter`
- Hashnode URL: `https://hashnode.com/onboard`
- Medium URL: `https://medium.com/`
- Preferred username: `TreeRingMemory`
- Fallback usernames: `TreeRingMem`, `UseTreeRing`
- Profile image: `marketing/assets/social-square-logo-1080x1080.png`
- First post: `Why AI agent memory should age like tree rings`
- Copy source: `docs/launch/rust-native-agent-memory-cli.md` plus
  `marketing/channel-playbook.md#developer-blogs`

## After Each Account Exists

1. Add the public URL to `marketing/social-profiles.json`.
2. Update the matching `marketing/outreach-crm.csv` row from `ready` to
   `live` after the first profile or post is public.
3. Update `marketing/submission-ledger.csv` with the public URL and evidence.
4. Do not commit screenshots containing private browser state, personal email,
   recovery codes, billing pages, phone numbers, or private notifications.
