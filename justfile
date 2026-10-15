# Justfile

fmt:
    cargo +nightly fmt --all

clippy:
    cargo +nightly clippy --locked --all-targets -- -D warnings

test:
    cargo test --locked --all-targets

build:
    cargo build --release --locked

release:
    cargo smart-release --no-push --no-changelog-preview --allow-dirty --no-publish --execute
