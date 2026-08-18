# Ark brand assets

`src/assets/brand/ark-mark.svg` is the canonical Ark mark. The frontend imports that file through `ArkBrand`; platform icons are generated derivatives and must not be edited independently.

Regenerate every Tauri, Windows, macOS, iOS, and Android icon after changing the source artwork:

```text
pnpm brand:icons
```

The canonical mark uses an opaque neutral hexagon and high-contrast light letterform so it remains legible on light, dark, and transparent platform surfaces. Keep all SVG content local and declarative: no script, external URL, embedded font, or raster payload.
