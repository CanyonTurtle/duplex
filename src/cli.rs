// Native CLI entrypoint: load a candidate alg list, enumerate every ZBLL
// (last-layer) case with the existing cube core, and report which cases get
// solved (directly, or by chaining two algs together with an AUF between
// them -- a "duplex") along with the solving move sequence.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use serde_json::json;

use crate::alg::{create_algset, Alg};
use crate::cube::Cube;
use crate::enumerate::{self, Case};
use crate::search::{self, Solution};

#[derive(Parser)]
#[command(
    name = "duplex-search",
    version,
    about = "Check whether a list of candidate algs solves the ZBLL case set"
)]
struct Args {
    /// Candidate algorithm list. Either a .csv with one alg per line as
    /// `moves,mirror,invert` (mirror is one of fb/lr/no, invert is yes/no --
    /// the format used by duplexalgs.csv), or a .json array of
    /// {name, moves, mirror: "FB"|"LR"|null, invert} objects (the format the
    /// web UI's alg list uses).
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

    /// Which last-layer case set to check against: `zbll` is the ~493 cases
    /// where edges are already oriented (only corner O/P + edge P remain --
    /// what "ZBLL algs" are meant to solve); `all` is the full 1LLL case set
    /// (edges may also need orienting).
    #[arg(long, default_value = "zbll")]
    case_set: CaseSet,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaseSet {
    Zbll,
    All,
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
    let args = Args::parse();

    let (algs, errors) = load_algs(&args.algs);
    if !args.quiet {
        for err in &errors {
            eprintln!("warning: {}", err);
        }
    }
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

    let all_cases = enumerate::get_cases();
    let cases: Vec<Case> = match args.case_set {
        CaseSet::Zbll => all_cases.into_iter().filter(Case::is_zbll).collect(),
        CaseSet::All => all_cases,
    };
    println!(
        "enumerated {} {} cases",
        cases.len(),
        match args.case_set {
            CaseSet::Zbll => "ZBLL",
            CaseSet::All => "1LLL",
        }
    );

    let depth = args.depth.clamp(1, 2);
    if depth != args.depth {
        eprintln!("note: --depth clamped to {} (only 1 or 2 are supported)", depth);
    }

    let solutions = search::search(&algs, &cases, depth);

    report(&cases, &solutions, args.show_unsolved);

    if let Some(out) = &args.out {
        write_report(out, &cases, &solutions);
        println!("wrote full report to {}", out.display());
    }
}

fn load_algs(path: &Path) -> (Vec<Alg>, Vec<String>) {
    let content = fs::read_to_string(path).unwrap_or_else(|err| {
        eprintln!("error: couldn't read {}: {}", path.display(), err);
        std::process::exit(1);
    });

    let is_json = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false);

    if is_json {
        load_json_algs(&content)
    } else {
        load_csv_algs(&content)
    }
}

/// `moves,mirror,invert` per line -- mirror in {fb, lr, no}, invert in {yes, no}.
fn load_csv_algs(content: &str) -> (Vec<Alg>, Vec<String>) {
    let mut algs = Vec::new();
    let mut errors = Vec::new();

    for (i, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(|f| f.trim()).collect();
        if fields.len() < 3 {
            errors.push(format!(
                "line {}: expected `moves,mirror,invert`, got {:?}",
                i + 1,
                line
            ));
            continue;
        }

        let moves = fields[0];
        let mirror = match fields[1].to_lowercase().as_str() {
            "fb" => Some("FB"),
            "lr" => Some("LR"),
            "no" | "" => None,
            other => {
                errors.push(format!(
                    "line {}: unknown mirror type {:?} (expected fb/lr/no)",
                    i + 1,
                    other
                ));
                continue;
            }
        };
        let invert = match fields[2].to_lowercase().as_str() {
            "yes" => true,
            "no" | "" => false,
            other => {
                errors.push(format!(
                    "line {}: unknown invert flag {:?} (expected yes/no)",
                    i + 1,
                    other
                ));
                continue;
            }
        };

        let name: String = moves.chars().filter(|c| !c.is_whitespace()).collect();
        let entry = json!([{
            "name": name,
            "moves": moves,
            "mirror": mirror,
            "invert": invert,
        }]);

        match create_algset(entry.to_string()) {
            Ok(mut parsed) => algs.append(&mut parsed),
            Err(err) => errors.push(format!("line {}: {}", i + 1, err)),
        }
    }

    (algs, errors)
}

/// Array of {name, moves, mirror: "FB"|"LR"|null, invert} objects.
fn load_json_algs(content: &str) -> (Vec<Alg>, Vec<String>) {
    let mut algs = Vec::new();
    let mut errors = Vec::new();

    let values: Vec<serde_json::Value> = match serde_json::from_str(content) {
        Ok(values) => values,
        Err(err) => {
            eprintln!("error: invalid JSON alg list: {}", err);
            std::process::exit(1);
        }
    };

    for (i, value) in values.into_iter().enumerate() {
        let entry = json!([value]);
        match create_algset(entry.to_string()) {
            Ok(mut parsed) => algs.append(&mut parsed),
            Err(err) => errors.push(format!("entry {}: {}", i + 1, err)),
        }
    }

    (algs, errors)
}

fn report(cases: &[Case], solutions: &HashMap<u64, Vec<Solution>>, show_unsolved: bool) {
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

fn write_report(path: &Path, cases: &[Case], solutions: &HashMap<u64, Vec<Solution>>) {
    let entries: Vec<serde_json::Value> = cases
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
        .collect();

    let content = serde_json::to_string_pretty(&entries).expect("solutions are always serializable");
    fs::write(path, content).unwrap_or_else(|err| {
        eprintln!("error: couldn't write {}: {}", path.display(), err);
        std::process::exit(1);
    });
}
