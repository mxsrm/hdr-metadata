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

`src/probe.rs` handles demuxing and decoding; `src/metadata.rs` parses FFmpeg side data; `src/encoder.rs` formats encoder parameters; `src/main.rs` handles CLI.

This utility reads static HDR10 mastering-display and content-light metadata, not dynamic HDR10+/Dolby Vision data. It has not been compiled in the generation environment (the Rust toolchain and FFmpeg development packages are absent).
