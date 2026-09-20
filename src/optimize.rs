// Greedy weighted set cover: given a pool of candidate algs (each a bundle
// of its mirror/invert variants, see `candidates::Candidate`), find a cheap
// subset that -- together with duplex pairing between chosen algs -- solves
// every target case.
//
// Set cover is NP-hard in general; the standard, well-understood answer is a
// greedy approximation: repeatedly pick whichever unpicked candidate covers
// the most *new* ground per unit cost, until everything's covered or no
// candidate helps anymore. This is within a factor of (1 + ln n) of optimal
// for weighted set cover, which is what makes it the default choice rather
// than something bespoke.
//
// The wrinkle here versus textbook set cover is that coverage isn't just
// per-candidate: a case can also only be reachable by *pairing* two chosen
// candidates (a "duplex") with an AUF between them. So each round we just
// re-run the full `search::search` over (already chosen algs + this
// candidate) and see how many previously-uncovered cases that trial set
// newly reaches -- simplest correct thing, and cheap enough since the chosen
// set stays small in practice (see README for real numbers).

use std::collections::{HashMap, HashSet};

use crate::alg::Alg;
use crate::candidates::Candidate;
use crate::enumerate::Case;
use crate::search::{self, Solution};

/// Cost assigned to a candidate: `per_alg + per_move * candidate.cost_moves`.
/// Set `per_move: 1.0, per_alg: 0.0` to minimize total moves learned (the
/// default -- "tersest subset"); `per_alg: 1.0, per_move: 0.0` to minimize
/// the number of algs instead; blend both for something in between.
pub struct Objective {
    pub per_alg: f64,
    pub per_move: f64,
}

impl Objective {
    fn cost(&self, candidate: &Candidate) -> f64 {
        self.per_alg + self.per_move * candidate.cost_moves as f64
    }
}

pub struct Picked {
    pub label: String,
    pub cost_moves: usize,
    pub newly_covered: usize,
}

pub struct Report {
    pub picked: Vec<Picked>,
    pub covered: usize,
    pub total: usize,
    pub solutions: HashMap<u64, Vec<Solution>>,
}

pub fn greedy_cover(
    candidates: &[Candidate],
    cases: &[Case],
    depth: usize,
    objective: &Objective,
    max_picks: Option<usize>,
    seed_variants: &[Alg],
) -> Report {
    let target: HashSet<u64> = cases.iter().map(|c| c.ll_index).collect();

    // algs already known going in (free -- don't count toward cost/picked,
    // just narrow down what's left to cover)
    let mut chosen_variants: Vec<Alg> = seed_variants.to_vec();
    let mut solutions = search::search(&chosen_variants, cases, depth);
    let mut uncovered: HashSet<u64> = target
        .iter()
        .filter(|ll| !solutions.get(ll).map(|s| !s.is_empty()).unwrap_or(false))
        .cloned()
        .collect();

    let mut remaining: Vec<usize> = (0..candidates.len()).collect();
    let mut picked = Vec::new();

    while !uncovered.is_empty() {
        if let Some(max) = max_picks {
            if picked.len() >= max {
                break;
            }
        }

        let mut best: Option<(usize, usize, f64, HashMap<u64, Vec<Solution>>)> = None;

        for (pos, &idx) in remaining.iter().enumerate() {
            let candidate = &candidates[idx];

            let mut trial: Vec<Alg> = chosen_variants.clone();
            trial.extend(candidate.variants.iter().cloned());
            let trial_solutions = search::search(&trial, cases, depth);

            let gain = uncovered
                .iter()
                .filter(|ll| {
                    trial_solutions
                        .get(ll)
                        .map(|s| !s.is_empty())
                        .unwrap_or(false)
                })
                .count();
            if gain == 0 {
                continue;
            }

            let ratio = gain as f64 / objective.cost(candidate).max(f64::EPSILON);
            let better = match &best {
                None => true,
                Some((_, _, best_ratio, _)) => ratio > *best_ratio,
            };
            if better {
                best = Some((pos, gain, ratio, trial_solutions));
            }
        }

        let Some((pos, gain, _, trial_solutions)) = best else {
            break; // nothing left in the pool covers any remaining case
        };

        let idx = remaining.remove(pos);
        let candidate = &candidates[idx];

        chosen_variants.extend(candidate.variants.iter().cloned());
        uncovered.retain(|ll| {
            !trial_solutions
                .get(ll)
                .map(|s| !s.is_empty())
                .unwrap_or(false)
        });
        picked.push(Picked {
            label: candidate.label.clone(),
            cost_moves: candidate.cost_moves,
            newly_covered: gain,
        });
        solutions = trial_solutions;
    }

    Report {
        covered: target.len() - uncovered.len(),
        total: target.len(),
        picked,
        solutions,
    }
}
