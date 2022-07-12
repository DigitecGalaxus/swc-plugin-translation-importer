# swc-plugin-constant-importer

A simple plugin that replaces translation key usages with imports from a file.

## Build

Get the [Rust toolchain](https://www.rust-lang.org/learn/get-started) and the
right target with `rustup target add wasm32-wasi`. Then you're just a `npm run
build` away and can find the result `swc_plugin_constant_importer.wasm`.

## Release

1. `npm install` if never done before
2. Update version number in `package.json`
3. `npm run build`
4. `npm publish`

## License

Licensed under either of

- [Apache License, Version 2.0](LICENSE-APACHE)
- [MIT license](LICENSE-MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
