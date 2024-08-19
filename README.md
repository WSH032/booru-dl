# Booru-Dl

A command line tool to download images and tags txt from [booru](https://booru.org/).

[![Crates.io][Crates-Badge]][Crates-Url]
[![Docs.rs][Docs-Badge]][Docs-Url]
[![License][License-Badge]][License-Url]

[Crates-Badge]: https://img.shields.io/crates/v/booru-dl.svg
[Crates-Url]: https://crates.io/crates/booru-dl
[Docs-Badge]: https://docs.rs/booru-dl/badge.svg
[Docs-Url]: https://docs.rs/booru-dl
[License-Badge]: https://img.shields.io/crates/l/booru-dl.svg
[License-Url]: LICENSE

Currently, we only support downloads from [Gelbooru](https://gelbooru.com/).

This is the Rust rewrite of [Gelbooru-API-Downloader](https://github.com/WSH032/Gelbooru-API-Downloader). If you need the Python version, you can refer to it.

## Credit

> [!WARNING]
> Attention! It's probably against [Gelbooru's TOS](https://gelbooru.com/tos.php)!
>
> We are not responsible for any consequences of using this tool, use it at your own risk.

## Requirements

See: [docs.rs](https://docs.rs/booru-dl#requirements)

## Installation

**IMPORTANT: `cli` feature is required for this command line program.**

See [docs.rs](https://docs.rs/booru-dl#feature-flags) for more feature.

```bash
cargo install booru-dl --features="cli"
```

## Usage

The following command will open a editor to ask for arguments; after you save and close the editor, the program will start downloading images.

```bash
booru-dl
```

Or use the following command to see more options:

```bash
booru-dl --help
```

## What does this name mean?

`Booru-Dl` is short for Booru Downloader.

## License

This project is licensed under the terms of the *Apache License 2.0*.
