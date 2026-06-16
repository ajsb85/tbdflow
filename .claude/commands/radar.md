---
description: Scan for overlapping work that may cause merge conflicts
---

Run `tbdflow --toon radar` and report the `overlaps[]{branch,author,file,kind}` array —
who else is modifying the same files — before pushing. For a lighter check that also
includes branch/CI state, use `tbdflow --toon context` instead.

If overlaps exist, suggest coordinating with the listed author(s) or syncing more
frequently. Never push over an unacknowledged `line-overlap`.
