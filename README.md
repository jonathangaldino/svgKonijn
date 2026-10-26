# svgKonijn

A desktop app that turns raster images (PNG, JPEG, WebP, GIF, BMP, TIFF, ICO) into SVG vector graphics. Built with [Tauri v2](https://tauri.app). All the image processing runs in Rust, on your machine, with nothing uploaded anywhere.

Runs on macOS, Windows and Linux.

## Features

- **Open images any way**: file picker, drag and drop, or paste from the clipboard.
- **Color or monochrome output**: color mode traces every posterized color as its own layer; monochrome traces the dark areas only.
- **Posterize**: set the number of levels per channel (red, green, blue, alpha) to control how many colors end up in the SVG.
- **Image adjustments before tracing**: brightness, contrast, grayscale, hue rotation, invert, opacity, saturation, sepia, plus scale and rotation.
- **Tracing controls**: suppress speckles, tune the corner, segment and splice thresholds, pick smooth (spline), polygon or pixel-exact curves, add a stroke to close hairline gaps between layers, drop very short paths, and set how many decimal places the coordinates keep.
- **Live preview**: the SVG re-traces as you move a slider. Scroll to zoom, drag to pan, double-click to reset the view.
- **Export**: save to a `.svg` file or copy the SVG markup to the clipboard.
- Settings are remembered between launches, and each group has its own reset button.

## Tips

- For logos, icons and illustrations, the defaults usually work well.
- For photos, lower the **Scale** first. A large photo can produce an SVG of tens of megabytes and take seconds per trace.
- Fewer posterize steps mean fewer colors, smaller files and a more graphic look.
- In monochrome mode, use **Brightness** and **Contrast** to move the line between what is traced and what is dropped.
- Visible seams between color layers go away with a **Stroke width** of 0.5–1 px.

## Development

Requirements:

- [Rust](https://rustup.rs) (stable)
- Node.js 22+
- The [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your OS (on Linux: `libwebkit2gtk-4.1-dev` and friends)

```sh
make dev    # run the app with hot reload
make test   # run the Rust unit tests
```

The frontend in `src/` is plain HTML, CSS and JavaScript with no build step. Edits reload immediately.

## Building

Every build target copies its installers into `dist/`.

| Command | Output | Notes |
| --- | --- | --- |
| `make build-macos` | `.dmg` | Run on macOS |
| `make build-windows` | NSIS `.exe` | Cross-compiles from macOS. Needs `brew install nsis llvm lld`, `cargo install cargo-xwin` and `rustup target add x86_64-pc-windows-msvc` |
| `make build-linux` | `.deb`, `.rpm`, `.AppImage` | Builds in Docker for the host architecture. Add `LINUX_PLATFORM=linux/amd64` for x86_64 (emulated, slow) |
| `make build-all` | all of the above | |
| `make clean` | | Removes `dist/` and Rust build output |

On any OS you can also build natively with `npx tauri build`.

GitHub Actions (`.github/workflows/build.yml`) builds native bundles for all three platforms on every push to `main` and uploads them as workflow artifacts. That includes a universal macOS build, a Windows `.msi`, and x86_64 Linux packages.

The builds are not code-signed. macOS Gatekeeper and Windows SmartScreen will show a warning the first time the app is opened.

## How it works

The pipeline lives in [`src-tauri/src/trace.rs`](src-tauri/src/trace.rs):

1. **Preprocess**: scale and rotate the image, then apply the color filters (the same math as the CSS filter functions).
2. **Posterize**: snap each channel to the chosen number of levels.
3. **Split by color**: group pixels by exact color into layers. Monochrome mode keeps only one layer, the dark pixels.
4. **Trace**: every layer is split into connected regions, and each region is converted into a path with [visioncortex](https://github.com/visioncortex/visioncortex). Layers are traced in parallel with [rayon](https://github.com/rayon-rs/rayon).
5. **Assemble**: one `<g>` per color, with the largest layers first so details are painted on top.

[`src-tauri/src/lib.rs`](src-tauri/src/lib.rs) exposes four commands to the UI:

- `load_image` decodes the raw file bytes once and keeps them in memory.
- `trace` runs the pipeline with the current settings.
- `save_svg` writes the SVG through a native save dialog.
- `copy_svg` copies the SVG to the clipboard.

## Project layout

```
src/                  UI (HTML, CSS, JS)
src-tauri/src/        Rust: commands (lib.rs) and the tracing pipeline (trace.rs)
src-tauri/tauri.conf.json
scripts/build-linux.sh  Linux build run inside Docker
Makefile
```
