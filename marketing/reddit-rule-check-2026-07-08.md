# Reddit Rule Check - 2026-07-08

Status: owner-side posting gate

Checked from public Old Reddit rule pages on 2026-07-08. The modern Reddit
JSON rules endpoints returned HTTP `403` from this environment, so this note
uses the public Old Reddit rule pages linked below.

Do not paste the prepared copy mechanically. Several target communities
explicitly restrict low-effort or AI-generated submissions. Treat the launch
kit as a brief that the owner should rewrite in their own voice after reading
the current rules from an authenticated session.

## r/rust

- Rules URL: `https://old.reddit.com/r/rust/about/rules/`
- Posting status: use caution; owner should human-rewrite before posting.
- Relevant constraints:
  - Posts must reference Rust or explain Rust relevance in a text post.
  - Titles should include useful context.
  - Low-effort content is not allowed.
  - Submissions appearing to contain AI-generated content may be removed at
    moderator discretion.
- Recommended adjustment: keep the Rust CLI/storage details, use a text post,
  and ask for concrete CLI/storage feedback rather than a launch announcement.

## r/LocalLLaMA

- Rules URL: `https://old.reddit.com/r/LocalLLaMA/about/rules/`
- Posting status: possible only from a credible account with disclosure.
- Relevant constraints:
  - Posts must relate to Llama or LLMs.
  - Low-effort posts may be removed.
  - Primarily LLM-generated copy is not allowed.
  - Self-promotion should follow the 1/10 guideline.
  - Affiliation must be disclosed.
- Recommended adjustment: owner should rewrite the local/private agent workflow
  angle in their own voice and disclose maintainer affiliation.

## r/opensource

- Rules URL: `https://old.reddit.com/r/opensource/about/rules/`
- Posting status: possible only with owner rewrite and correct flair.
- Relevant constraints:
  - Excessive self-promotion is not allowed.
  - Posts must be directly relevant to open source.
  - Linked code/repositories must have an OSI-listed open-source license.
  - No drive-by posting or karma farming.
  - All AI-generated content is treated as low-effort and ban-worthy.
  - Use the correct flair; promotional posts should use Promotional.
- Recommended adjustment: owner should rewrite the post, use the repository
  link, mention the MIT license, use the correct flair, and stay in comments.

## r/commandline

- Rules URL: `https://old.reddit.com/r/commandline/about/rules/`
- Posting status: hold.
- Relevant constraints:
  - Posters must agree to subreddit rules from new Reddit before posting.
  - CLI/TUI relevance is required.
  - Post text or titles generated with AI are strictly prohibited.
  - Projects newer than 30 days or with only a few commits may be removed.
  - Generative-AI-related projects are not allowed unless they are popular
    projects such as Ollama or GGML.
  - Similar or alternative software should be listed when applicable.
- Recommended adjustment: do not post Tree Ring Memory there during this launch
  window. Revisit after the project is older than 30 days and has stronger
  external adoption, if the generative-AI restriction changes or moderators
  explicitly approve.

## r/AI_Agents

- Rules URL: `https://old.reddit.com/r/AI_Agents/about/rules/`
- Posting status: possible only in the allowed format.
- Relevant constraints:
  - No spam.
  - Put links in comments, not posts.
  - Project show-offs should link in the weekly project display thread.
  - Self-promotion should follow a one-out-of-ten guideline.
  - Posts need enough context and should not only drive traffic elsewhere.
- Recommended adjustment: use a text-only discussion post or weekly project
  thread comment. Put the repo/website links in a comment, not the post body,
  and disclose maintainer affiliation.
