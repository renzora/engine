# AI Policy

**Renzora is AI-friendly.** Use an assistant to write code, explore the
workspace, draft docs, write tests, or explain a subsystem you've never touched.
We are not going to ask you to prove a patch was typed by hand, and we do not
treat "an AI helped" as a mark against a contribution.

What we insist on is the other half: **you own what you submit.** Everything in a
pull request must be read, understood, audited, and tested by a human before it
gets there — and any contribution an AI materially helped produce has to say
which model and version was used.

Those two rules exist for one reason. A model that has never built this
workspace writes code that *looks* exactly like code that works. The cost of
that lands on reviewers and on everyone who ships a game on top of the result.
Auditing is the price of the speed-up, not an optional extra.

## The rules

1. **Use AI freely.** No approval needed, no percentage cap, no separate lane
   for AI-assisted PRs. They're reviewed on the same terms as everything else.
2. **Audit every line before you submit it.** You must be able to explain, in
   review, why each change is there and what it does. "The model wrote it" is
   not an answer to a review question.
3. **Validate it by running it.** Build it, launch it, exercise the thing you
   changed. A patch that has only ever been read is not validated.
4. **Write tests.** New logic ships with tests that would fail without the
   change. See [Testing](testing.md).
5. **Disclose the model and version.** See
   [Disclosing AI assistance](#disclosing-ai-assistance) for the exact format.
6. **Never open a PR you haven't read in full.** Piping a model's output
   straight into a branch and pushing it is the one thing this policy forbids
   outright.

## What "audited" means here

Auditing is not skimming for plausibility. Concretely, before you submit:

- **Every API the patch calls actually exists**, with that signature, in the
  pinned versions this workspace uses (**Bevy 0.19**, **Rust 1.95.0**). Models
  confidently invent Bevy APIs, and they blend versions — a 0.16-era method name
  in 0.19 code is the single most common failure we see.
- **You've read the surrounding module.** Renzora has a lot of hard-won,
  non-obvious shape to it, and a model working from a small window will
  cheerfully undo it. If your patch removes or contradicts a `//!`/`///` comment
  explaining *why* something is the way it is, find out what broke last time
  before you take it out.
- **You've checked it against the project's actual constraints**, not the
  assistant's guess at them. `CLAUDE.md` at the repo root is the authoritative
  guide, and the traps below are the ones AI-assisted patches hit most often.
- **No dead scaffolding.** Unused helpers, speculative config knobs, `TODO`s for
  work nobody asked for, and defensive `unwrap()`s are all easy for a model to
  emit and easy for a human to delete before pushing.
- **Comments say *why*, not *what*.** Restating the code in prose is the house
  style of most assistants and the opposite of ours — see
  [Code Style](code-style.md).

### Traps AI-assisted patches hit most often

| Trap | What actually applies |
|---|---|
| Running a bare `cargo build` / `cargo test` | Every cargo command in this repo takes **`--profile dist`**. A `dev`-profile build creates a second full artefact tree; ours once hit 314 GB and filled the disk, and a full disk shows up as nonsense compile errors in crates nobody touched. |
| "The plugin ABI depends on a shared `bevy_dylib` hash" | Stale. In-workspace plugins are statically linked `rlib`s wired in by a build-time generator; third-party plugins are standalone C-ABI cdylibs that link no Bevy. Neither has a hash to keep in sync. |
| Registering a plugin in a runtime registry | `renzora::add!(...)` is parsed **as text** at build time into the committed `plugins.rs` lists. Keep the declaration on one line at the top level. |
| Inventing a scripting function | The API is what's declared in the domain crate's `ScriptExtension` plus the language plugin's `register_api()`. If a function doesn't exist, extend the API properly — don't write a script against a hallucinated one. |
| Editing frozen docs | Only `docs/r1-alpha7/` is live. `r1-alpha6` and older are frozen releases. |
| Skipping the docs update | A feature without its `docs/r1-alpha7/` page update is unfinished. |

## Tests are not optional

The whole point of the audit rule is that plausible-looking code has to be
*proven*, and a test is the only proof that survives after review. So:

- **New logic ships with a test that fails without the patch.** If you can't
  make it fail on the old code, the test isn't testing your change.
- **Bug fixes ship with a regression test** reproducing the bug.
- **Test the edges, not the happy path.** Empty input, one element, the
  saturating case, the deserialize-round-trip. A model asked for "some tests"
  will write three assertions on the case that obviously works; that's padding,
  and reviewers will read it as padding.
- **Run them.** `cargo test --profile dist -p <crate>` links and runs natively;
  `renzora test` reproduces CI exactly.
- **CI is a floor, not a finish line.** `renzora check` (clippy, warnings
  denied) passing tells you the code compiles and lints. It says nothing about
  whether it's correct.

Where a test genuinely can't be written — a rendering change, an editor
interaction, a device-specific path — say so in the PR and describe how you
verified it by hand, with a screenshot or a capture where that helps.

## Disclosing AI assistance

**If a model materially contributed to a change, name it and its version.**

"Materially" means it wrote or substantially rewrote code, docs, or tests that
ended up in the diff. Autocomplete of a variable name doesn't count; neither
does asking an assistant to explain a crate you then wrote yourself. When
unsure, disclose — it costs one line and nobody has ever been penalised for it.

Add a trailer to the commit, and mention it in the PR description:

```text
feat(physics): cylinder collider with radius and height

Assisted-by: Claude Opus 5 (claude-opus-5)
```

The format is `Assisted-by: <name> <version>`, with the exact model
identifier in parentheses when the tool exposes one. Version matters as much as
name — model behaviour, and the failure modes we look for in review, differ
sharply between releases of the same family. `Assisted-by: an LLM` is not a
disclosure. Multiple models get multiple lines.

```text
Assisted-by: Claude Opus 5 (claude-opus-5)
Assisted-by: Some Model 3.1 8B (some-model-3.1-8b-instruct)
```

Note that this is `Assisted-by:`, not `Co-authored-by:`. Co-authorship
attributes the work to a person who can be asked about it later; a model can't
be. **The human who opens the PR is the sole author and is fully responsible for
the contribution**, whatever produced the first draft.

Why we ask at all, given that we don't mind: it tells reviewers which failure
modes to look for, it gives us real data on which tools produce contributions
that hold up, and it keeps the provenance of the tree honest. It is not a
warning label.

## Licensing and provenance

By opening a PR you certify you have the right to contribute the code under
**MIT OR Apache-2.0** — the same certification as any other contribution. AI
assistance does not change it, and it does not dilute it: if a model reproduced
a substantial chunk of someone else's licensed code into your patch, that's a
licence problem you're responsible for, not the tool's.

In practice: be wary of a model producing a long, self-contained,
suspiciously-complete implementation of a known algorithm or a well-known crate's
internals. Recognise it, check where it came from, and either vendor it properly
with its licence intact or write your own.

## Issues, reviews, and discussion

- **Bug reports must be real.** File what you actually hit, with the steps you
  actually ran. A model's speculation about a bug that might exist is not a bug
  report, and reproducing it costs a maintainer real time.
- **Don't paste unverified AI analysis into an issue** as if it were a
  diagnosis. If an assistant helped you find the cause, verify it first, then
  report the verified cause.
- **Reviewing with AI is fine; posting its output raw is not.** Review comments
  should be things you'd defend yourself.
- **Bulk AI-generated PRs will be closed.** A run of drive-by patches nobody
  read is spam regardless of how it was produced, and it's the fastest way to
  make maintainers distrust every AI-assisted PR that follows.

## Generated assets

This policy covers the engine repository. Assets published to the Renzora
marketplace — including AI-generated models, textures, and audio — are governed
by the marketplace terms, not by this page. See
[Publishing Assets](../marketplace/publishing.md).

## If it goes wrong

Nobody's getting banned for a patch that turned out to be wrong — that's what
review is for, and it happens to hand-written code too. What we act on is the
pattern this policy exists to prevent: submissions the contributor plainly never
read, repeated after being told. Maintainers may close those and ask for a
verified resubmission.

Get it right and none of this is friction. Use the tools, read the output, prove
it works, say what you used.

## What's next?

- [Contributing Guide](guide.md) — workflow, CI, and the PR checklist
- [Testing](testing.md) — how to write and run the suite
- [Code Style](code-style.md) — formatting, naming, and comment conventions
