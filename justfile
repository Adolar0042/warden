set quiet

alias b := build
alias r := run
alias f := fmt
alias t := test
alias c := clippy
alias s := sanity

default:
    @just --list

build *FLAGS:
    cargo build 

run *FLAGS:
    cargo run 

test *FLAGS:
    cargo test 

bench *FLAGS:
    cargo bench 

fmt *FLAGS:
    cargo fmt 

clippy *FLAGS:
    cargo clippy 

sanity:
    cargo clippy --all --all-targets -- -W clippy::all -W clippy::pedantic -W clippy::nursery

release:
    cargo check
    cargo fmt --check
    cargo test --workspace
    cargo build --release --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    typos
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --document-private-items

install:
    cargo install --locked --path .

clean:
    cargo clean
