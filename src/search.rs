// Native (non-wasm) search over candidate algorithms, looking for which ones
// solve which ZBLL (last-layer) cases, optionally chaining two algs together
// (a "duplex") with an AUF between them so a smaller learned set of algs can
// still cover the full ZBLL case list.
//
// The approach mirrors `web::run_algs`: rather than generating the scrambled
// state for every ZBLL case and applying each candidate alg forwards, we run
// each candidate *backwards* from a solved cube. The resulting state is, by
// definition, a state the forward alg solves. Comparing that state (up to a
// final AUF, handled by `Cube::get_ll_indices`) against the canonical case
// list tells us which case(s) the alg solves and with which leading AUF(s).

use std::collections::{HashMap, HashSet};

use crate::alg::Alg;
use crate::cube::{Cube, Move, UDBLTRANS, UPRITRANS, UTRANS};
use crate::enumerate::Case;

const AUF_LABELS: [&str; 4] = ["", "U", "U2", "U'"];

#[derive(Debug, Clone)]
pub struct Solution {
    /// alg names in the order they're executed to solve the case
    pub alg_names: Vec<String>,
    /// the full move sequence (AUFs included) that solves the case
    pub sequence: String,
    pub move_count: usize,
    /// how many candidate algs were chained together (1 = single alg, 2 = duplex)
    pub depth: usize,
}

fn do_auf(cube: &mut Cube, index: usize) {
    match index {
        1 => cube.do_transform(&UTRANS),
        2 => cube.do_transform(&UDBLTRANS),
        3 => cube.do_transform(&UPRITRANS),
        _ => {}
    }
}

// AUFs get reversed in meaning once we invert a sequence built backwards.
fn invert_auf(index: usize) -> usize {
    match index {
        1 => 3,
        3 => 1,
        other => other,
    }
}

fn auf_prefix(index: usize) -> String {
    if index == 0 {
        String::new()
    } else {
        format!("{} ", AUF_LABELS[index])
    }
}

fn moves_to_string(moves: &[Move]) -> String {
    moves
        .iter()
        .map(|m| format!("{:?}", m))
        .collect::<Vec<String>>()
        .join(" ")
}

/// Search `algs` (and, if `max_depth >= 2`, every AUF-separated pair of
/// `algs`) for solutions to every case in `cases`. Returns every solution
/// found per case's ll_index, sorted with the shortest / lowest-depth
/// solutions first.
pub fn search(algs: &[Alg], cases: &[Case], max_depth: usize) -> HashMap<u64, Vec<Solution>> {
    let target_indices: HashSet<u64> = cases.iter().map(|c| c.ll_index).collect();
    let inverted: Vec<Alg> = algs.iter().map(|a| a.invert()).collect();

    let mut solutions: HashMap<u64, Vec<Solution>> = HashMap::new();
    let mut add = |ll_index: u64, solution: Solution| {
        solutions.entry(ll_index).or_insert_with(Vec::new).push(solution);
    };

    let mut cube = Cube::new();

    // depth 1: a single candidate alg, with a leading AUF
    for alg in inverted.iter() {
        for auf in 0..4 {
            cube.replace(Cube::new());
            cube.do_transform(&alg.transform);
            do_auf(&mut cube, auf);

            for ll_index in cube.get_ll_indices() {
                if target_indices.contains(&ll_index) {
                    let solved_by = alg.invert();
                    let leading_auf = invert_auf(auf);
                    let sequence = format!(
                        "{}{}",
                        auf_prefix(leading_auf),
                        moves_to_string(&solved_by.moves),
                    );
                    add(
                        ll_index,
                        Solution {
                            alg_names: vec![solved_by.get_full_name()],
                            move_count: solved_by.moves.len() + (leading_auf != 0) as usize,
                            sequence,
                            depth: 1,
                        },
                    );
                }
            }
        }
    }

    // depth 2 ("duplex"): two candidate algs chained with an AUF between them
    if max_depth >= 2 {
        for first in inverted.iter() {
            for first_auf in 0..4 {
                for second in inverted.iter() {
                    for second_auf in 0..4 {
                        cube.replace(Cube::new());
                        cube.do_transform(&first.transform);
                        do_auf(&mut cube, first_auf);
                        cube.do_transform(&second.transform);
                        do_auf(&mut cube, second_auf);

                        for ll_index in cube.get_ll_indices() {
                            if target_indices.contains(&ll_index) {
                                // executed in this order to solve the case
                                let solved_by_1st = second.invert();
                                let solved_by_2nd = first.invert();
                                let auf_1st = invert_auf(second_auf);
                                let auf_2nd = invert_auf(first_auf);
                                let sequence = format!(
                                    "{}{} {}{}",
                                    auf_prefix(auf_1st),
                                    moves_to_string(&solved_by_1st.moves),
                                    auf_prefix(auf_2nd),
                                    moves_to_string(&solved_by_2nd.moves),
                                );
                                add(
                                    ll_index,
                                    Solution {
                                        alg_names: vec![
                                            solved_by_1st.get_full_name(),
                                            solved_by_2nd.get_full_name(),
                                        ],
                                        move_count: solved_by_1st.moves.len()
                                            + solved_by_2nd.moves.len()
                                            + (auf_1st != 0) as usize
                                            + (auf_2nd != 0) as usize,
                                        sequence,
                                        depth: 2,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    for group in solutions.values_mut() {
        group.sort_by_key(|s| (s.depth, s.move_count));
    }

    solutions
}
