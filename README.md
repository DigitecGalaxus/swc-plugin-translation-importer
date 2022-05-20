# swc-plugin-i18n

This is the SWC version of `@tools/i18n-plugin`.

## Build

Get the [Rust toolchain](https://www.rust-lang.org/learn/get-started) and the
right target with `rustup target add wasm32-wasi`. Then you're just a `npm run
build` away and can find the result `swc_plugin_i18n.wasm`.

## Release

1. Build
2. Update version number in `package.json`
3. `npm install`
4. `npm publish`
