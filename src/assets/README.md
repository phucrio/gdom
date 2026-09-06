# GDOM brand assets

`gdom-icon.svg` is the editable source of truth for the approved **Primary / G handoff** concept: a pearl-white folded G, a periwinkle right-pointing arrow, and an indigo rounded tile. It is a vector reconstruction of the selected concept, not a crop of the presentation board. The SVG uses local paths and gradients only; it has no fonts, scripts, embedded raster images, or remote resources.

The app header and browser favicon use this same SVG with Vite's `?no-inline` suffix. Keeping it as an emitted file avoids a `data:` image URI, which the existing Tauri image CSP does not allow. Do not relax the CSP to display the logo.

All packaged desktop icons in `src-tauri/icons/` are generated from the SVG, including Windows ICO, macOS ICNS, PNG sizes, and the Square/Store logos. Keeping ICNS assets does not add macOS runtime support.

## Regenerate

From the repository root, with dependencies installed using the lockfile:

```sh
pnpm install --frozen-lockfile
pnpm icons
pnpm icons:check
pnpm test
```

The generator invokes the installed Tauri CLI without a shell, stages output in a temporary directory, and copies only desktop icon files. ICNS representation chunks are sorted before writing so container ordering does not create spurious diffs. Mobile outputs are discarded. Commit the SVG and regenerated desktop icons together. Do not hand-edit individual PNG/ICO/ICNS files or add another copy of the master SVG.

`pnpm icons:check` regenerates into a temporary directory and checks the committed files byte-for-byte. It fails rather than rewriting stale assets. Windows CI runs this check alongside tests for PNG format and dimensions, configured paths, ICO resolutions, ICNS structure, and CSP-safe asset references. The filesystem-based asset tests use `.mjs` so Node types are not added to the browser TypeScript project.

## Review

Check the icon at 16, 24, 32, 48, 64, 128, and 256 pixels on light and dark backgrounds. The transparent outer corners and open counter should remain clear. The adjacent GDOM wordmark provides the accessible name in the header, so the decorative image has an empty `alt`.

Before release, also inspect a Windows development window and a packaged installer, including Explorer, taskbar, and Start menu. Windows may cache an older icon after replacing a previously installed build; automated asset checks do not validate the shell cache.
