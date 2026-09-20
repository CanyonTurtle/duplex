duplex is an experimental speedcubing method

## dev environment

This repo has a flake. `direnv allow` (or `nix develop`) gets you Rust with
the `wasm32-unknown-unknown` target, plus Node/Yarn for the frontend.

## CLI: searching candidate algs against ZBLL

`src/cli.rs` is a native (non-wasm) entrypoint that reuses the same cube
simulation as the web app. It has two subcommands.

`--algs` (both subcommands) accepts either:
- a `.csv` with one alg per line as `moves,mirror,invert` (mirror is
  `fb`/`lr`/`no`, invert is `yes`/`no`) — the format used by
  [duplexalgs.csv](../ctduplexer/duplexalgs.csv)
- a `.json` array of `{name, moves, mirror: "FB"|"LR"|null, invert}` objects
  — the format the web UI's alg list uses

### `solve` — does this alg list solve ZBLL?

Checks whether a fixed list of candidate algorithms solves the ZBLL case set
(the ~493 last-layer cases where edges are already oriented — only corner
permutation/orientation and edge permutation remain). For each candidate it
tries every AUF in front of it against every ZBLL case. If `--depth 2` (the
default), it also tries every AUF-separated pair of candidates — a "duplex"
— so a small learned set can cover cases no single alg solves alone.

```sh
cargo run --release -- solve --algs path/to/algs.csv
```

Useful flags:
- `--depth 1` — only check single algs, skip duplex pairing (faster)
- `--case-set all` — check against the full 1LLL case set instead of ZBLL
- `--out report.json` — write the full per-case solution list
- `--show-unsolved` — print a diagram of every case nothing solves

### `optimize` — what's the tersest subset that solves ZBLL?

Given a larger candidate *pool*, greedily picks the cheapest subset that
(with mirrors/inverses and duplex pairing) still covers the ZBLL case set —
each round it adds whichever unpicked candidate covers the most new cases
per unit cost, until everything's covered or the pool is exhausted. Mirror
and invert variants ride along with the base alg they came from — learning
one row buys all of its variants, so the cost model is charged per row.

```sh
cargo run --release -- optimize --algs path/to/algs.csv
```

The cost of a candidate is `per_alg_weight + per_move_weight * move_count`:
- default (`--per-move-weight 1 --per-alg-weight 0`) minimizes total moves
  learned across the chosen set
- `--per-alg-weight 1 --per-move-weight 0` minimizes the number of algs
  instead, ignoring length
- blend both for something in between

Other flags: `--max-algs N` caps how many algs it's allowed to pick,
`--out set.json` writes the chosen set plus the full per-case report.

Run `cargo run --release -- solve --help` / `optimize --help` for the rest.

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
