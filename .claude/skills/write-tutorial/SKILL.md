---
name: write-tutorial
description: Write a guided how-to / in-app help for a multi-step LoreGUI flow (onboarding, connect-to-server, merge, host setup, etc.). Use to satisfy the help gate for non-trivial flows. Produces step-by-step copy placed in-app and/or in website docs.
---

# Write a tutorial / how-to

For a multi-step flow, produce help users can follow without prior knowledge.

## Steps

1. **Confirm the flow** with the domain expert: the exact op sequence, each step's
   inputs, what success/failure looks like, and any state (e.g. merge resolve loop).
2. **Write the steps** — numbered; each step = one concrete action + what to expect
   + the failure/retry note. Lead with the goal and prerequisites. Match lore
   terminology (define jargon once, then reuse).
3. **Place it:**
   - In-app: a help panel / first-run coachmarks / an info affordance next to the
     flow. Keep it in the same PR as the feature so it never drifts.
   - Optionally `website/` docs for the long-form version; link from in-app.
4. **Voice:** concise, imperative, concrete — per `docs/DESIGN-SYSTEM.md`. No
   marketing tone in-app.
5. **Verify** every instruction against the real UI/op — don't document aspirational
   behavior.

## Output
The tutorial content + where it's wired in, and a one-line entry for the op/flow's
manifest `description` if missing. Hand visuals to `loregui-ux-designer`.
