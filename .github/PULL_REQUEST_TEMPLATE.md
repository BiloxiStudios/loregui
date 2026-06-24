<!--
Thanks for contributing to LoreGUI! Please fill out the sections below.
Keep PRs focused — one op = one file per layer; don't touch files outside the
scope of your change (PRs that reformat unrelated files are bounced).
-->

## Summary

<!-- What does this PR do and why? Lead with the Jira key if there is one,
     e.g. "SBAI-XXXX: branch merge_resolve_theirs". -->

## Related issue / ticket

<!-- Closes #123  /  SBAI-XXXX -->

## Type of change

- [ ] Bug fix
- [ ] New lore op / feature
- [ ] UI / UX
- [ ] Docs only
- [ ] Build / CI / tooling
- [ ] Upstream `lore` pin bump (manager-owned — see CONTRIBUTING.md)

## Checklist

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy -p lore-vm -- -D warnings` passes
- [ ] `cargo test -p lore-vm` passes (and integration tests if `crates/lore-vm/**` changed)
- [ ] `cargo check -p loregui` passes
- [ ] `npm --prefix frontend run build` passes
- [ ] `node frontend/scripts/palette-parity.mjs` passes (if an op was added/exposed)
- [ ] New/changed ops land in the app coherently (surface decided, semantic
      theme tokens used, help/description added) — see the coherence mandate in
      `CLAUDE.md`
- [ ] No files outside the scope of this change were touched/reformatted
- [ ] Docs updated if behavior or workflow changed

## Notes for reviewers

<!-- Screenshots for UI changes, design-review notes, anything reviewers should
     know. Redact any credentials or repo paths. -->
