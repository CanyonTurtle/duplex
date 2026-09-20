duplex is an experimental speedcubing method

## dev environment

This repo has a flake. `direnv allow` (or `nix develop`) gets you Rust with
the `wasm32-unknown-unknown` target, plus Node/Yarn for the frontend.

## CLI: searching candidate algs against ZBLL

`src/cli.rs` is a native (non-wasm) entrypoint that reuses the same cube
simulation as the web app to check whether a list of candidate algorithms
solves the ZBLL case set (the ~493 last-layer cases where edges are already
oriented — only corner permutation/orientation and edge permutation remain).

For each candidate alg it tries every AUF in front of it against every ZBLL
case. If `--depth 2` (the default), it also tries every AUF-separated pair of
candidates — a "duplex" — so a small learned set can cover cases no single
alg solves alone.

```sh
cargo run --release -- --algs path/to/algs.csv
```

`--algs` accepts either:
- a `.csv` with one alg per line as `moves,mirror,invert` (mirror is
  `fb`/`lr`/`no`, invert is `yes`/`no`) — the format used by
  [duplexalgs.csv](../ctduplexer/duplexalgs.csv)
- a `.json` array of `{name, moves, mirror: "FB"|"LR"|null, invert}` objects
  — the format the web UI's alg list uses

Useful flags:
- `--depth 1` — only check single algs, skip duplex pairing (faster)
- `--case-set all` — check against the full 1LLL case set instead of ZBLL
- `--out report.json` — write the full per-case solution list
- `--show-unsolved` — print a diagram of every case nothing solves

Run `cargo run --release -- --help` for the rest.

TODO

    ability to hide unsolved cases
    add beginner method
    add custom subsets and save
    sort by algs & sticker patterns
    add 'not U' sticker
    prefer-sunes
    dont pick the same case twice in trainer
    retain trainer ticks
    vet alg list for mirrors/inverse
