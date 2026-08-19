# atauri

A Tauri + Rust desktop harness for human–AI collaboration, built before the
system it turned into. Published as the ancestor it is.

## Why this repository exists

The research and the current environment describe a set of behavioural rules and
a shared human–model memory. Read on their own they look designed. They were
not — they were arrived at. This is one of the places they were arrived at, and
the source tree says so more plainly than any write-up could:

```
shared/src/barycentrum.rs    the centre of mass between human and AI
shared/src/ghost.rs          a system's interaction history as its identity
shared/src/context.rs        context continuity across sessions
shared/src/snowglobe.rs      bounded deliberation before acting
shared/src/quinn.rs          the QA persona, as a module
shared/src/router.rs         routing between a local and a cloud model
shared/src/qdrant.rs         the vector layer
```

Those names are not decoration. They are the Codex Symbiosis principles —
written as prose first, then as skills, and here compiled. `barycentrum.rs`
carries the definition in its own doc comment:

> An emergent insight is something that: neither the human nor the AI had
> explicitly stated; arose from the interaction between them; represents new
> understanding, not just combination. The Barycentrum is the space between —
> where emergence happens.

## What it does

A desktop application that runs a local model and a cloud model behind one
router, keeps a persistent context and a vector store, and records emergent
insights rather than only transcripts. Rust for the shared core, Tauri for the
shell.

## Status

Superseded. The ideas here continued into a larger system; this one stopped
where it stopped. It is public because the lineage matters more than the polish:
the current work makes claims about how a practice formed, and this is the
evidence for one link in that chain.

The prose version of the same principles: **[claude-skills](https://github.com/misomalcek/claude-skills)**.

## Building

```bash
cargo build          # workspace: shared core + Tauri app
npm install && npm run tauri dev
```

## Licence

MIT.
