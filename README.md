# Fruit — Route Experimental Results

> **FRUIT = WHAT ROUTE MAY BECOME / WAS USED TO BUILD.**
> This branch contains **experimental results produced through Route dogfood**.
> It is **not** the Route product release and **not** a stable release.

```
MAIN    = WHAT ROUTE IS.          (Route v1.0 beta — stable/public product)
FRUIT   = EXPERIMENTAL RESULTS.   (this branch)
SANDBOX = HOW IT WAS DEVELOPED.   (development process / evidence / history)
```

> **Warning:** This branch is experimental and may be rearranged at any time.
> **For Route itself, use `main`.** The block above is a pointer: if you want the
> Route product, check out `main`, not this branch.

## What lives here

- `tool/route/` — Route's standalone product source (merged dogfood of the product).
- `tools/patchbench/` — **PatchBench**, a standalone patch verification and
  benchmark utility produced through Route dogfood.

## PatchBench

**A small standalone patch verification tool for AI-assisted development.**

A developer or an AI working group changes a repo, produces a `task.json` spec,
then PatchBench deterministically verifies the patch against it — changed-file
scope (`allowed_paths` / `forbidden_paths`), really-executed checks, and
claims-vs-evidence. It does not run an AI; any host (Claude, Codex, Gemini, a
human, Route, Yuich) can write the spec. PatchBench only executes it.

See [tools/patchbench/README.md](tools/patchbench/README.md).

## Branch policy

- Experimental code / results → `fruit`.
- Development record / process → `sandbox`.
- Public confirmed Route → `main`.
- Promotion to `main` happens only with verified evidence, never by an AI
  Worker self-deciding.

## License

See the individual tool/artifact license. Fruit is experimental dogfood output,
not the Route product release.