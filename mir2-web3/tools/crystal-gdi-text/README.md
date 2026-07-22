# Crystal GDI text exporter

This Windows-only tool exports complete strings with the same GDI path used by
Crystal's `MirLabel`. It is intentionally isolated from the Web client. It does
not modify application code and it does not bundle a font file.

## Reproduced `MirLabel` behavior

- `Arial`, Regular, 8pt on an explicit 96 DPI drawing surface.
- `TextRenderer.MeasureText(Graphics, text, font)` for auto size.
- Auto-sized outlined labels add 2 pixels to measured width and height.
- `Format32bppArgb` source bitmap, saved as a PNG with alpha.
- Crystal graphics settings: `AntiAlias`, `AntiAliasGridFit`,
  `HighQuality`, `NearestNeighbor`, `HighQuality`, and text contrast `0`.
- Outline draws black at `(1,0)`, `(0,1)`, `(2,1)`, and `(1,2)`, followed by
  foreground at `(1,1)`.
- Non-outlined text draws foreground at `(1,0)`.
- Each PNG is rendered as one complete string. Glyphs are not concatenated, so
  GDI kerning, prefix handling, line breaks, alignment, and measurement remain
  part of the result.

Crystal scales its requested font by `96 / Graphics.DpiX`. Rendering an 8pt
font onto this tool's fixed 96 DPI surface produces the equivalent pixel size
without depending on the desktop display scale.

## Requirements

- Windows.
- Windows PowerShell 5.1 or PowerShell 7 on Windows.
- `System.Windows.Forms` and `System.Drawing` desktop assemblies.
- A legally installed Arial Regular font at `%WINDIR%\Fonts\arial.ttf`.

The exporter fails if Arial is absent or if `System.Drawing` substitutes a
different family.

## Export

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File `
  .\tools\crystal-gdi-text\Export-CrystalGdiText.ps1 `
  -InputPath .\tools\crystal-gdi-text\fixtures\input.json `
  -OutputDirectory .\tools\crystal-gdi-text\fixtures\generated `
  -Force
```

`-Force` only replaces a directory carrying this tool's exact ownership marker.
It refuses unmarked directories, filesystem roots, and reparse-point outputs.
Without `-Force`, the output directory must not exist.

To refresh the committed fixture baseline:

```powershell
powershell -ExecutionPolicy Bypass -File `
  .\tools\crystal-gdi-text\Update-Fixtures.ps1
```

## Input schema

Input must be strict UTF-8 JSON. Unknown properties, malformed UTF-8, malformed
JSON, duplicate JSON properties (including escape-equivalent names), empty text,
unpaired UTF-16 surrogates, duplicate output keys, unsafe output keys, invalid
colours, unsupported flags, and unsafe dimensions are rejected before an output
directory is installed.

```json
{
  "schemaVersion": 1,
  "items": [
    {
      "key": "chat-online-players",
      "text": "Online Players: 1",
      "foreground": "#FF006400",
      "background": "#FFFFFFFF",
      "outline": false,
      "drawFormat": ["WordBreak"],
      "size": "auto"
    }
  ]
}
```

Rules:

- `key` is also the PNG filename stem and may contain only ASCII letters,
  digits, `.`, `_`, and `-`. It cannot contain path separators.
- Colours use uppercase `#AARRGGBB`, not CSS `#RRGGBB`.
- `outline` is a JSON boolean. Its colour and offsets are fixed to Crystal's
  implementation and cannot be overridden.
- `drawFormat` is a non-empty array of supported `TextFormatFlags` names.
- `size` is either `"auto"` or `{ "width": N, "height": N }`. Fixed size
  corresponds to a `MirLabel` whose `AutoSize` is false; no outline padding is
  added to a fixed control rectangle.
- Input is capped at 1 MiB, 1,024 records, and 65,535 UTF-16 code units per
  string. Output dimensions and pixel count also have allocation limits.

## Manifest

`manifest.json` is UTF-8 without a BOM. It records:

- Input SHA-256 and item count.
- Windows build, CLR version, and process architecture.
- Requested and resolved font family, size, DPI, `arial.ttf` filename and hash.
- GDI graphics settings and `Format32bppArgb` texture format.
- For every asset: text, foreground, background, outline recipe, draw flags,
  output key/path, measured/requested/output size, PNG SHA-256, canonical ARGB
  pixel SHA-256, and alpha-bucket counts.

The ARGB hash is calculated over top-to-bottom pixels in byte order A, R, G, B.
It therefore verifies decoded pixels independently of PNG container bytes.

## Self-test

```powershell
powershell -ExecutionPolicy Bypass -File `
  .\tools\crystal-gdi-text\Test-CrystalGdiText.ps1
```

The self-test:

1. Exports the valid fixture twice into tool-local temporary directories.
2. Requires every output file, including the manifest and PNG bytes, to match.
3. Decodes every PNG and verifies signature, dimensions, 96 DPI metadata,
   `Format32bppArgb`, PNG hash, canonical ARGB hash, and alpha counts.
4. Compares the new output byte-for-byte with `fixtures/generated`.
5. Confirms every JSON under `fixtures/invalid` fails closed and leaves no
   installed output directory.
6. Removes its temporary directories after success or failure.

## Reproducibility and licensing risks

GDI raster output is deterministic only while the relevant environment remains
stable. Windows/GDI updates, the exact Arial file, font-smoothing configuration,
process architecture, and desktop-runtime behavior can change pixels. The
manifest exposes the Windows build and font hash so such changes fail visibly
instead of being mistaken for a valid baseline. Run the self-test on the same
Windows image used to build production text assets.

Arial is proprietary software distributed with licensed Microsoft products.
This repository does not copy or redistribute `arial.ttf`; it only records the
hash of the locally installed file. Before distributing generated raster assets,
obtain legal review for the target product and distribution model. A replacement
font is not accepted by this tool because it would no longer reproduce Crystal.
