# Bundled fonts

| File | Source | Licence |
|---|---|---|
| `Inter-Regular.ttf` | Inter 4 (rsms/inter), as packaged in `@expo-google-fonts/inter` 0.2.3 | SIL Open Font License 1.1 (`LICENSE-inter-OFL.txt`) |
| `Inter-Bold.ttf` | the same package, `Inter_700Bold.ttf` | SIL Open Font License 1.1 (`LICENSE-inter-OFL.txt`) |
| `lucide.ttf` | Lucide icon font, `lucide-static` 0.468.0 | ISC (`LICENSE-lucide.txt`) |

Both are embedded with `include_bytes!` so rendering is identical on every platform
(SPEC section 4.6). Keep the licence files beside the fonts. Lucide codepoints are
referenced only through `src/icons.rs`, which is generated from Lucide's own
`info.json` for the icons the app uses.
