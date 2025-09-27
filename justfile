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
    cargo build {{FLAGS}}

run *FLAGS:
    cargo run {{FLAGS}}

test *FLAGS:
    cargo test {{FLAGS}}

bench *FLAGS:
    cargo bench {{FLAGS}}

fmt *FLAGS:
    cargo fmt {{FLAGS}}

clippy *FLAGS:
    cargo clippy {{FLAGS}}

sanity *FLAGS:
    cargo clippy --all --all-targets {{FLAGS}} -- -W clippy::all -W clippy::pedantic -W clippy::nursery

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
