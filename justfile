set shell := ["zsh", "-c"]
hostname := "magic"

default:
    @just --list
# Cargo
[group('cargo')]
push:
    cargo fmt -- --check
    cargo clippy -- -D warnings
    cargo test
    jj bookmark set main -r @-
    jj git push
[group('cargo')]
clip:
    cargo clippy --all-targets --all-features -- -D warnings -W clippy::pedantic -W clippy::nursery
[group('cargo')]
release version:
    cargo test
    cargo publish --dry-run
    jj tag create v{{version}} --revision main
    jj git push --tags
    cargo publish
