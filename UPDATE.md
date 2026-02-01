# Update (2026-02-01)

## Summary
- Added `embeddb` core APIs to list/describe tables, plus corresponding CLI commands.

## Verification
- `make check`

## PR instructions
If `gh` is installed + authenticated:
- `git checkout -b feat/table-introspection`
- `git push -u origin feat/table-introspection`
- `gh pr create --fill`

If `gh` is unavailable or unauthenticated:
- Push the branch as above, then open a PR on GitHub from `feat/table-introspection` into `main`.
