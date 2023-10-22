# plato-opds
A fetcher hook for the [Plato](https://github.com/baskerville/plato) document reader that syncs
an e-reader with an [OPDS](https://en.wikipedia.org/wiki/Open_Publication_Distribution_System) catalogue.

## Motivation
I wanted to be able to sync my e-reader with my [calibre-web](https://github.com/janeczku/calibre-web) library so that
any new books/documents I add are downloaded automatically.

This project makes no guarantees to be OPDS spec-compliant and your milage may vary.

## Usage

1. Download/build a `opds_fetcher` binary and create a folder in Plato's bin directory for it (usually `/mnt/onboard/.adds/plato/bin/opds`)
2. Rename `Settings-sample.toml` to `Settings.toml` and place it alongside the binary. Configure your servers and tweak
settings how you like.
3. Add a hook to Plato's own `Settings.toml` that looks like the following:
```toml
[[libraries.hooks]]
path = "OPDS"
program = "bin/opds/opds_fetcher"
sort-method = "added"
first-column = "title-and-author"
second-column = "progress"
```
4. Whenever the `OPDS` folder is opened, this hook will check if there are any books that haven't been downloaded
and will fetch them if need be.

## Building
The easiest way to build this project is to use [cross](https://github.com/cross-rs/cross) to cross-compile with Docker.
Once setup it's as simple as running:
```shell
cross build --release --target=arm-unknown-linux-gnueabihf
```

### Building without Docker
After setting up the [Linaro toolchain](https://releases.linaro.org/components/toolchain/binaries/4.9-2017.01/arm-linux-gnueabihf/) on your system you can
compile the project with:
```shell
rustup target add arm-unknown-linux-gnueabihf
cargo build --release --target=arm-unknown-linux-gnueabihf
```

# Acknowledgements
This hook is based on the work done for the built-in [Plato article fetcher](https://github.com/baskerville/plato/blob/master/crates/fetcher/src/main.rs).
The license of which is included below.

```
Plato -- Document reader for the Kobo e-ink devices.
Copyright (C) 2017 Bastien Dejean.

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU Affero General Public License as
published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License
along with this program. If not, see <http://www.gnu.org/licenses/>.
```