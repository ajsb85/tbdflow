---
description: Sync with trunk and report situational awareness
---

Get up to date with trunk using the **tbdflow** skill:

1. `tbdflow --non-interactive --toon sync`
2. `tbdflow --toon context`

Summarise from the TOON results: current branch, clean/dirty, ahead/behind, `trunk_ci`,
`stale[]` branches, and any radar `overlaps[]`. Flag anything that needs action before
committing (dirty tree, failing CI, overlapping work).
