// Native CLI entrypoint. Two modes:
//
//   solve     -- does this fixed list of algs solve the ZBLL case set?
//   optimize  -- what's the cheapest subset of this candidate pool that
//                (with its mirrors/inverses and duplex pairing) solves it?

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use serde_json::{json, Value};

use crate::alg::Alg;
use crate::candidates;
use crate::cube::Cube;
use crate::enumerate::{self, Case};
use crate::optimize::{self, Objective};
use crate::search::{self, Solution};

#[derive(Parser)]
#[command(
    name = "duplex-search",
    version,
    about = "Search candidate algs for ZBLL solutions"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check whether a fixed list of candidate algs solves the ZBLL case set.
    Solve(SolveArgs),
    /// Search a candidate pool for the cheapest subset that solves the ZBLL
    /// case set, once mirrors/inverses and duplex pairing are accounted for.
    Optimize(OptimizeArgs),
}

/// Candidate algorithm list, shared by both subcommands. Either a .csv with
/// one alg per line as `moves,mirror,invert` (mirror is one of fb/lr/no,
/// invert is yes/no -- the format used by duplexalgs.csv), or a .json array
/// of {name, moves, mirror: "FB"|"LR"|null, invert} objects (the format the
/// web UI's alg list uses).
#[derive(Args)]
struct SolveArgs {
    #[arg(short, long)]
    algs: PathBuf,

    /// How many candidate algs to chain per case: 1 checks each alg alone,
    /// 2 also tries every AUF-separated pair ("duplexes") for cases no
    /// single alg solves.
    #[arg(short, long, default_value_t = 2)]
    depth: usize,

    /// Write the full per-case solution report as JSON to this path.
    #[arg(short, long)]
    out: Option<PathBuf>,

    /// Print every unsolved case's LL diagram to stdout.
    #[arg(long)]
    show_unsolved: bool,

    /// Suppress warnings about candidate lines that failed to parse.
    #[arg(long)]
    quiet: bool,

    #[arg(long, default_value = "zbll")]
    case_set: CaseSet,
}

#[derive(Args)]
struct OptimizeArgs {
    /// Candidate pool to pick a subset from (same .csv/.json formats as `solve`).
    #[arg(short, long)]
    algs: PathBuf,

    /// How many candidate algs to chain per case when evaluating coverage
    /// (see `solve --help`).
    #[arg(short, long, default_value_t = 2)]
    depth: usize,

    /// Which last-layer case set to cover: `zbll` (default) or `all` (1LLL).
    #[arg(long, default_value = "zbll")]
    case_set: CaseSet,

    /// Cost added per move in a candidate's base alg -- the default (1.0)
    /// makes the greedy search minimize total moves across the chosen set.
    #[arg(long, default_value_t = 1.0)]
    per_move_weight: f64,

    /// Cost added per candidate picked, regardless of its length -- set this
    /// (and --per-move-weight 0) to minimize the number of algs instead.
    #[arg(long, default_value_t = 0.0)]
    per_alg_weight: f64,

    /// Stop after picking this many algs even if some cases stay uncovered.
    #[arg(long)]
    max_algs: Option<usize>,

    /// Algs you already know (same .csv/.json format as --algs) -- included
    /// for free before the search starts, so it only looks for what to add
    /// on top. Use this to ask "what's the best next alg for a basis I
    /// already have?".
    #[arg(long)]
    seed: Option<PathBuf>,

    /// Write the chosen set and full per-case solution report as JSON here.
    #[arg(short, long)]
    out: Option<PathBuf>,

    /// Suppress warnings about candidate lines that failed to parse.
    #[arg(long)]
    quiet: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaseSet {
    Zbll,
    All,
}

impl CaseSet {
    fn label(self) -> &'static str {
        match self {
            CaseSet::Zbll => "ZBLL",
            CaseSet::All => "1LLL",
        }
    }

    fn filter(self, all_cases: Vec<Case>) -> Vec<Case> {
        match self {
            CaseSet::Zbll => all_cases.into_iter().filter(Case::is_zbll).collect(),
            CaseSet::All => all_cases,
        }
    }
}

impl std::str::FromStr for CaseSet {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "zbll" => Ok(CaseSet::Zbll),
            "all" | "1lll" => Ok(CaseSet::All),
            other => Err(format!("unknown case set {:?} (expected zbll or all)", other)),
        }
    }
}

pub fn run() {
    match Cli::parse().command {
        Command::Solve(args) => run_solve(args),
        Command::Optimize(args) => run_optimize(args),
    }
}

fn run_solve(args: SolveArgs) {
    let (candidates, errors) = candidates::load(&args.algs);
    warn(&errors, args.quiet);
    let algs: Vec<Alg> = candidates.into_iter().flat_map(|c| c.variants).collect();
    if algs.is_empty() {
        eprintln!(
            "error: no usable algorithms loaded from {}",
            args.algs.display()
        );
        std::process::exit(1);
    }
    println!(
        "loaded {} alg variants from {} ({} entries skipped)",
        algs.len(),
        args.algs.display(),
        errors.len(),
    );

    let cases = args.case_set.filter(enumerate::get_cases());
    println!("enumerated {} {} cases", cases.len(), args.case_set.label());

    let depth = clamp_depth(args.depth);
    let solutions = search::search(&algs, &cases, depth);

    report_coverage(&cases, &solutions, args.show_unsolved);

    if let Some(out) = &args.out {
        let content = json!({ "cases": case_entries(&cases, &solutions) });
        write_json(out, &content);
        println!("wrote full report to {}", out.display());
    }
}

fn run_optimize(args: OptimizeArgs) {
    let (candidates, errors) = candidates::load(&args.algs);
    warn(&errors, args.quiet);
    if candidates.is_empty() {
        eprintln!(
            "error: no usable algorithms loaded from {}",
            args.algs.display()
        );
        std::process::exit(1);
    }
    println!(
        "loaded {} candidate algs from {} ({} entries skipped)",
        candidates.len(),
        args.algs.display(),
        errors.len(),
    );

    let cases = args.case_set.filter(enumerate::get_cases());
    println!("enumerated {} {} cases", cases.len(), args.case_set.label());

    let depth = clamp_depth(args.depth);
    let objective = Objective {
        per_alg: args.per_alg_weight,
        per_move: args.per_move_weight,
    };

    let seed_variants: Vec<Alg> = match &args.seed {
        Some(path) => {
            let (seed_candidates, seed_errors) = candidates::load(path);
            warn(&seed_errors, args.quiet);
            println!(
                "seeded with {} alg(s) from {}",
                seed_candidates.len(),
                path.display()
            );
            seed_candidates.into_iter().flat_map(|c| c.variants).collect()
        }
        None => Vec::new(),
    };

    let report = optimize::greedy_cover(
        &candidates,
        &cases,
        depth,
        &objective,
        args.max_algs,
        &seed_variants,
    );

    println!();
    for pick in &report.picked {
        println!(
            "+ {} ({} moves) -> {} new case(s)",
            pick.label, pick.cost_moves, pick.newly_covered
        );
    }

    let total_moves: usize = report.picked.iter().map(|p| p.cost_moves).sum();
    println!();
    println!(
        "chose {} alg(s), {} total moves, covering {} / {} cases ({:.1}%)",
        report.picked.len(),
        total_moves,
        report.covered,
        report.total,
        100.0 * report.covered as f64 / report.total as f64,
    );
    if report.covered < report.total {
        println!(
            "(stopped {} case(s) short -- try a bigger --algs pool, --depth 2, or raise/drop --max-algs)",
            report.total - report.covered,
        );
    }

    if let Some(out) = &args.out {
        let picked: Vec<Value> = report
            .picked
            .iter()
            .map(|p| {
                json!({
                    "label": p.label,
                    "cost_moves": p.cost_moves,
                    "newly_covered": p.newly_covered,
                })
            })
            .collect();
        let content = json!({
            "picked": picked,
            "covered": report.covered,
            "total": report.total,
            "cases": case_entries(&cases, &report.solutions),
        });
        write_json(out, &content);
        println!("wrote full report to {}", out.display());
    }
}

fn warn(errors: &[String], quiet: bool) {
    if !quiet {
        for err in errors {
            eprintln!("warning: {}", err);
        }
    }
}

fn clamp_depth(depth: usize) -> usize {
    let clamped = depth.clamp(1, 2);
    if clamped != depth {
        eprintln!("note: --depth clamped to {} (only 1 or 2 are supported)", clamped);
    }
    clamped
}

fn report_coverage(cases: &[Case], solutions: &HashMap<u64, Vec<Solution>>, show_unsolved: bool) {
    let total = cases.len();
    let mut solved_depth1 = 0;
    let mut solved_depth2_only = 0;
    let mut unsolved: Vec<&Case> = Vec::new();

    for case in cases {
        match solutions.get(&case.ll_index) {
            Some(sols) if sols.iter().any(|s| s.depth == 1) => solved_depth1 += 1,
            Some(sols) if !sols.is_empty() => solved_depth2_only += 1,
            _ => unsolved.push(case),
        }
    }

    let solved = solved_depth1 + solved_depth2_only;
    println!();
    println!(
        "solved {} / {} cases ({:.1}%)",
        solved,
        total,
        100.0 * solved as f64 / total as f64
    );
    println!("  by a single alg: {}", solved_depth1);
    println!("  needing a duplex (2 algs): {}", solved_depth2_only);
    println!("  unsolved: {}", unsolved.len());

    if show_unsolved && !unsolved.is_empty() {
        println!();
        println!("-- unsolved cases --");
        for case in &unsolved {
            println!("case {} (ll_index {}):", case.index, case.ll_index);
            println!("{}", case_to_cube(case));
        }
    }
}

fn case_to_cube(case: &Case) -> Cube {
    let mut cube = Cube::new();
    for i in 0..4 {
        cube.edges[i] = case.edges[i].clone();
        cube.corners[i] = case.corners[i].clone();
    }
    cube
}

fn case_entries(cases: &[Case], solutions: &HashMap<u64, Vec<Solution>>) -> Vec<Value> {
    cases
        .iter()
        .map(|case| {
            let sols = solutions.get(&case.ll_index);
            let best = sols.and_then(|s| s.first());
            json!({
                "case": case.index,
                "ll_index": case.ll_index,
                "solved": sols.map(|s| !s.is_empty()).unwrap_or(false),
                "best": best.map(|s| json!({
                    "algs": s.alg_names,
                    "sequence": s.sequence,
                    "move_count": s.move_count,
                    "depth": s.depth,
                })),
                "alternatives": sols.map(|s| s.len()).unwrap_or(0),
            })
        })
        .collect()
}

fn write_json(path: &Path, content: &Value) {
    let text = serde_json::to_string_pretty(content).expect("report is always serializable");
    fs::write(path, text).unwrap_or_else(|err| {
        eprintln!("error: couldn't write {}: {}", path.display(), err);
        std::process::exit(1);
    });
}
