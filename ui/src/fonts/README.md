# Bundled typefaces

Both are variable fonts under the SIL Open Font License 1.1, whose full text
sits beside them here. The OFL permits bundling and redistribution inside a
larger work; it requires the licence to travel with the font, which is why
these files are in the repository rather than only in the build.

| Family | Used for | Licence |
| --- | --- | --- |
| [Instrument Sans](https://github.com/google/fonts/tree/main/ofl/instrumentsans) | the interface | `OFL-InstrumentSans.txt` |
| [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono) | paths, keys, tabular figures | `OFL-JetBrainsMono.txt` |

Only the Latin and Latin Extended subsets are here — 84 KB for all four files.
Usernames and paths on this network arrive in every script there is, and
`--font` falls through to the system stack for the rest rather than carrying
megabytes to cover it.

They are served from the app's own origin, never a CDN. A webfont CDN would
announce the user's address to a third party on every launch, and the app's
CSP (`default-src 'self'`, no `font-src`) would refuse the request anyway.

To update one, take the `woff2` the Google Fonts CSS endpoint serves for the
`latin` and `latin-ext` subsets and replace the file in place; the
`unicode-range` declarations in `app.css` are the ones that endpoint publishes.
