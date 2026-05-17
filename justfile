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
