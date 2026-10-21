# swc-plugin-translation-importer

A simple plugin that replaces translation key usages with imports from a file.

## Usage

In these examples the JSON file containing the translations that are then
imported is assumed to be `@tools/i18n-plugin/.cache/translations.i18n`.

For Next.js you can install the package and reference it directly from
`next.config.js`

```javascript
module.exports = {
  experimental: {
    swcPlugins: [
      [
        "@galaxus/swc-plugin-translation-importer",
        { translationCache: "@tools/i18n-plugin/.cache/translations.i18n" },
      ],
    ],
  },
};
```

If you use SWC standalone, you have to point it to the Wasm module by adding
something like the following to `.swcrc`

```json
{
  "jsc": {
    "experimental": {
      "plugins": [
        [
          "/path/to/swc_plugin_translation_importer.wasm",
          {
            "translationCache": "@tools/i18n-plugin/.cache/translations.i18n"
          }
        ]
      ]
    }
  }
}
```

## Build

Get the [Rust toolchain](https://www.rust-lang.org/learn/get-started) and the
right target with `rustup target add wasm32-wasip1`. Then you're just a `npm run
build` away and can find the result `swc_plugin_translation_importer.wasm`.

## Release

Releases use [changesets](https://github.com/changesets/changesets) and the
`Release` GitHub Actions workflow.

1. In the PR with your change, run `pnpm changeset`, pick patch, minor or major
   and describe the change. Commit the new file in `.changeset/`.
2. After the PR is merged, the workflow opens a "Version Packages" PR that bumps
   `package.json` and adds the entry to `CHANGELOG.md`.
3. Merging the "Version Packages" PR publishes to npm with provenance (npm
   trusted publishing, no token) and pushes the `vX.Y.Z` tag.

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
