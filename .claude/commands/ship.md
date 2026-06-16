---
description: Commit current changes straight to trunk via tbdflow
argument-hint: [type] [subject]
---

Commit the current working changes to trunk using the **tbdflow** skill (never raw git).

1. Run `tbdflow --toon context` to confirm tree state and check radar `overlaps[]`.
   If there are overlaps, surface them and pause for confirmation.
2. Choose a Conventional Commit `type` and a `subject` (≤72 chars, lowercase first
   letter, no trailing period, imperative). If the user passed them in `$ARGUMENTS`, use those.
3. Commit: `tbdflow --non-interactive --toon commit -t <type> -m "<subject>"`
   (add `-s <scope>`, `--issue KEY-123`, or `-b` as appropriate). For a multi-line body,
   write it to a file and pass `--body-file <path>` instead of `--body`.
4. Parse the TOON result. On `ok: false`, branch on the `code` field and fix the input —
   do not fall back to raw git. Report the resulting `sha`, `signed`, and `pushed`.
