# notes-app

A tiny knowledge app. Feature list:

- [x] create note
- [x] list notes
- [x] tag notes            <- done (flat set design; prefix question deferred to search)
- [ ] search notes

Data model: note = { id, title, body }

## Tags design (session 1 decision, 2026-08-16)

- Tags are a flat set on the note: `tags: set<string>`
- Rationale: hierarchical tags were rejected — flat set keeps search
  simple; can be extended later without migration (add parent field).
- OPEN: whether search should match tag prefixes (`#pro` matches
  `#project`) — deferred to the search-notes task.
