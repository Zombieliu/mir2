# Submodule Local Patch Handoff

These patches preserve local, uncommitted edits that existed inside submodules
when the outer `mir2` workspace repository was created.

The outer repository records submodule commits only. A fresh clone with
`--recurse-submodules` will not automatically include these dirty submodule
working-tree changes.

Apply them only if those local edits are still needed:

```sh
git -C Crystal apply ../handoff/submodule-patches/Crystal.local-dirty.patch
git -C refactor-pwa apply ../handoff/submodule-patches/refactor-pwa.local-dirty.patch
```

`Crystal.untracked-files.txt` lists untracked files that were present under the
Crystal submodule and are not represented by the patch.
