// Loads a candidate alg list and groups each row's expansions (mirror /
// invert variants) together as one purchasable unit -- learning one alg
// gets you its mirror/inverse "for free", so `optimize` should buy or skip
// a whole row at once rather than picking individual expanded variants.

use std::fs;
use std::path::Path;

use serde_json::json;

use crate::alg::{create_algset, Alg};

pub struct Candidate {
    pub label: String,
    /// move count of the row's own (unmirrored, uninverted) alg -- what you'd
    /// actually have to learn and what "shortest subset" is measured against.
    pub cost_moves: usize,
    /// the base alg plus whichever of its invert/mirror/mirror-invert
    /// variants the row asked for.
    pub variants: Vec<Alg>,
}

impl Candidate {
    fn from_variants(label: String, variants: Vec<Alg>) -> Self {
        let cost_moves = variants
            .iter()
            .find(|a| !a.mirror && !a.invert)
            .map(|a| a.moves.len())
            .unwrap_or_else(|| variants.iter().map(|a| a.moves.len()).min().unwrap_or(0));
        Candidate {
            label,
            cost_moves,
            variants,
        }
    }
}

pub fn load(path: &Path) -> (Vec<Candidate>, Vec<String>) {
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
        load_json(&content)
    } else {
        load_csv(&content)
    }
}

/// `moves,mirror,invert` per line -- mirror in {fb, lr, no}, invert in {yes, no}.
fn load_csv(content: &str) -> (Vec<Candidate>, Vec<String>) {
    let mut candidates = Vec::new();
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
            Ok(variants) => candidates.push(Candidate::from_variants(moves.to_string(), variants)),
            Err(err) => errors.push(format!("line {}: {}", i + 1, err)),
        }
    }

    (candidates, errors)
}

/// Array of {name, moves, mirror: "FB"|"LR"|null, invert} objects.
fn load_json(content: &str) -> (Vec<Candidate>, Vec<String>) {
    let mut candidates = Vec::new();
    let mut errors = Vec::new();

    let values: Vec<serde_json::Value> = match serde_json::from_str(content) {
        Ok(values) => values,
        Err(err) => {
            eprintln!("error: invalid JSON alg list: {}", err);
            std::process::exit(1);
        }
    };

    for (i, value) in values.into_iter().enumerate() {
        let label = value
            .get("name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("entry {}", i + 1));
        let entry = json!([value]);
        match create_algset(entry.to_string()) {
            Ok(variants) => candidates.push(Candidate::from_variants(label, variants)),
            Err(err) => errors.push(format!("entry {}: {}", i + 1, err)),
        }
    }

    (candidates, errors)
}
