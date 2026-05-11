alias b := build
alias r := run
alias f := fmt
alias t := test
alias c := clippy
alias s := sanity

@default:
    just --list

@build *FLAGS:
    cargo build {{ FLAGS }}

@run *FLAGS:
    cargo run {{ FLAGS }}

@test *FLAGS:
    cargo test {{ FLAGS }}

@bench *FLAGS:
    cargo bench {{ FLAGS }}

@fmt *FLAGS:
    cargo fmt {{ FLAGS }}

@clippy *FLAGS:
    cargo clippy {{ FLAGS }}

@sanity *FLAGS:
    cargo clippy --all --all-targets {{ FLAGS }} -- -W clippy::all -W clippy::pedantic -W clippy::nursery

prep-release:
    cargo fmt --check
    cargo check --workspace
    cargo test --workspace
    cargo build --release --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    typos
    RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace --document-private-items

release VERSION:
    @[[ "{{ VERSION }}" =~ ^v ]] || (echo "==> Version must start with v" && exit 1)
    @cargo_version="$(grep -m1 '^\s*version\s*=' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')" || true; \
    if [[ "{{ VERSION }}" != "v$cargo_version" ]]; then \
        echo "==> Version in Cargo.toml does not match"; \
        exit 1; \
    fi
    @if [ -n "$(git tag -l {{ VERSION }})" ]; then \
        echo "==> Version '{{ VERSION }}' already exists"; \
        exit 1; \
    fi
    @echo "==> Signing tag '{{ VERSION }}'"
    git tag -s {{ VERSION }} -m "{{ VERSION }}"
    @echo "==> Tag '{{ VERSION }}' created"
    @echo "==> Push the tag with 'git push origin {{ VERSION }}'"

@install *FLAGS:
    cargo install --locked --path . {{ FLAGS }}

@clean:
    cargo clean
