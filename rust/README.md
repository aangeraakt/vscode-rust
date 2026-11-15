# Lathe

Lathe is a native Rust port of the behavior of [microsoft/vscode](https://github.com/microsoft/vscode).
It is an independent project and is not affiliated with or endorsed by Microsoft.

The TypeScript source in this repository is the read-only behavioral reference (see
`../docs/decisions/0001-project-name-and-reference.md`). Start with `../docs/HANDOFF.md`,
`../COORDINATION.md` and `../docs/ARCHITECTURE.md`.

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all --check
```

## License

Lathe is derived from microsoft/vscode and is distributed under the same MIT License; see
`../LICENSE.txt` (Copyright (c) 2015 - present Microsoft Corporation). Portions written for
Lathe are also MIT licensed.
