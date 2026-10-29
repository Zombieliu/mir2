# Manifest PNG SHA review

Read-only audit of `apps/web/public/original-ui/manifest.generated.json` against the current repository files and HEAD blobs. No files were edited.

- Current manifest has 11,937 `pngSha256` entries; HEAD had 11,936.
- 145 existing entries changed SHA values; every changed entry hashes exactly to its current corresponding PNG bytes.
- The sole new manifest entry is `libraries/Title/frames/309`, path `/original-ui/Title/785.png`; that new PNG also hashes exactly to the manifest value.
- Git status shows only one PNG path change under this tree: untracked `Title/785.png`. No tracked PNG is modified relative to its HEAD blob (0 content mutations across the changed PNG status set).
- The other manifest changes are metadata/hash corrections; there is no evidence that existing PNG pixels were mutated in this worktree.

The audit does not expose file contents or private data.
