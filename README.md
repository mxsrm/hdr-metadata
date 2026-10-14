# HDR metadata extractor

Rust 2024 utility using in-process FFmpeg libraries via `ffmpeg-next`, with no `ffprobe` subprocess.

## Build

Requires Rust 1.85+, FFmpeg development headers/libraries compatible with `ffmpeg-next` 9, `pkg-config`, and Clang/libclang (for FFmpeg bindings generation).

On Debian/Ubuntu, install the corresponding FFmpeg development packages, `pkg-config`, `clang`, and `libclang-dev`.

```sh
cargo build --release
cargo test
./target/release/hdr-metadata movie.mkv 30
```

The optional second argument limits the number of decoded frames searched (default 30). Decoding stops early once both static HDR10 metadata records are found.

## Command line

```sh
hdr-metadata --help
hdr-metadata --version
hdr-metadata movie.mkv
hdr-metadata movie.mkv 100
hdr-metadata --max-frames 100 movie.mkv
hdr-metadata -- -movie.mkv
```

`-h` and `-V` are aliases for help and version. With no arguments, help is shown.
Help and version exit successfully without opening media or initializing FFmpeg.
Frame limits must be positive integers; use either a positional limit or
`--max-frames`, not both. `--` ends option parsing for filenames starting with a
hyphen. Invalid arguments produce an error and exit status 2. With no arguments,
Clap displays help and exits with status 2; explicit help/version requests exit
with status 0. Probing errors exit with status 1.

`src/main.rs` defines the CLI with Clap's builder API. Default features are
disabled; only `std`, `help`, `usage`, and `error-context` are enabled. No custom
parser or derive macros are used. The displayed version comes from `Cargo.toml`.

The previous lockfile was removed because the dependency graph changed and
Cargo is unavailable in the editing environment. Run `cargo generate-lockfile`
to resolve dependencies and create a new lockfile before using `--locked`.

`src/probe.rs` handles demuxing and decoding; `src/metadata.rs` parses FFmpeg side data; `src/encoder.rs` formats encoder parameters; `src/main.rs` handles CLI.

This utility reads static HDR10 mastering-display and content-light metadata, not dynamic HDR10+/Dolby Vision data. It has not been compiled in the generation environment (the Rust toolchain and FFmpeg development packages are absent).
