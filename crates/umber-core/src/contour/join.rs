//! Joining oriented pieces of contour geometry, end to start, into lines and rings: the step
//! that a grid and a triangle mesh share once each has cut its own pieces.
//!
//! A piece runs from one vertex to another, and a vertex names one point exactly (a grid point,
//! a mesh point, or the crossing of a level on a side or an edge), so two pieces join where the
//! end of one is the start of the other. Wherever several unused pieces leave a vertex, the
//! first of them in the given order is taken, so the result is a function of the pieces and
//! their order alone.

use std::collections::HashMap;
use std::hash::Hash;

/// The pieces, by index into the given ones, that make one line or ring, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Chain {
    /// The indices of the pieces, each piece starting where the one before it ends.
    pub(super) pieces: Vec<usize>,
    /// Whether the last piece ends where the first starts.
    pub(super) closed: bool,
}

/// The pieces `pieces` (each `(from, to)`) joined into lines. Open lines come first: each
/// starts at a piece in order whose start more pieces leave than reach, while such a start is
/// left, and runs on until no unused piece leaves where it ends. Then each remaining piece in
/// order starts a line that runs until it returns to its start, which closes it, or until no
/// unused piece leaves where it ends.
pub(super) fn lines<V: Copy + Eq + Hash>(pieces: &[(V, V)]) -> Vec<Chain> {
    let mut leaving = Leaving::of(pieces);
    // How many more pieces leave each vertex than reach it.
    let mut excess: HashMap<V, isize> = HashMap::with_capacity(pieces.len());
    for &(from, to) in pieces {
        *excess.entry(from).or_insert(0) += 1;
        *excess.entry(to).or_insert(0) -= 1;
    }
    let mut used = vec![false; pieces.len()];
    let mut chains = Vec::new();
    for (first, &(from, _)) in pieces.iter().enumerate() {
        let opens = excess.get_mut(&from).filter(|left| **left > 0);
        if let Some(left) = opens.filter(|_| used.get(first) == Some(&false)) {
            *left -= 1;
            chains.push(leaving.follow(pieces, first, Closing::Never, &mut used));
        }
    }
    for first in 0..pieces.len() {
        if used.get(first) == Some(&false) {
            chains.push(leaving.follow(pieces, first, Closing::AtStart, &mut used));
        }
    }
    chains
}

/// The pieces `pieces` (each `(from, to)`), which reach every vertex as often as they leave it,
/// joined into closed rings: each starts at the first unused piece in order and runs until it
/// returns to its start.
pub(super) fn rings<V: Copy + Eq + Hash>(pieces: &[(V, V)]) -> Vec<Vec<usize>> {
    let mut leaving = Leaving::of(pieces);
    let mut used = vec![false; pieces.len()];
    let mut rings = Vec::new();
    for first in 0..pieces.len() {
        if used.get(first) == Some(&false) {
            rings.push(
                leaving
                    .follow(pieces, first, Closing::AtStart, &mut used)
                    .pieces,
            );
        }
    }
    rings
}

/// Whether a chain stops when it returns to the vertex it starts at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Closing {
    /// It never does: an open line runs on through its start.
    Never,
    /// It does, and is then closed.
    AtStart,
}

/// The pieces leaving each vertex, in order, as a list: a cursor at the first one that may be
/// unused, and after each piece the next one leaving the same vertex. Pieces are only ever
/// marked used, never unused again, so a cursor moved past used pieces still finds the first
/// unused one in order, and each piece is passed over once in all: joining takes time in
/// proportion to the number of pieces, however many leave one vertex.
struct Leaving<V> {
    /// For each vertex, the first piece leaving it that is not known to be used.
    cursor: HashMap<V, usize>,
    /// After each piece, the next one leaving the same vertex.
    after: Vec<Option<usize>>,
}

impl<V: Copy + Eq + Hash> Leaving<V> {
    /// The pieces leaving each vertex of `pieces`.
    fn of(pieces: &[(V, V)]) -> Leaving<V> {
        let mut first: HashMap<V, usize> = HashMap::with_capacity(pieces.len());
        let mut after: Vec<Option<usize>> = vec![None; pieces.len()];
        for (index, &(from, _)) in pieces.iter().enumerate().rev() {
            if let (Some(slot), Some(next)) = (after.get_mut(index), first.insert(from, index)) {
                *slot = Some(next);
            }
        }
        Leaving {
            cursor: first,
            after,
        }
    }

    /// The first unused piece leaving `vertex`, with the vertex's cursor moved to it, or
    /// removed when none is left.
    fn next_unused(&mut self, vertex: V, used: &[bool]) -> Option<usize> {
        let slot = self.cursor.get_mut(&vertex)?;
        let mut next = Some(*slot);
        while let Some(index) = next.filter(|&index| used.get(index) != Some(&false)) {
            next = self.after.get(index).copied().flatten();
        }
        match next {
            Some(index) => *slot = index,
            None => {
                self.cursor.remove(&vertex);
            }
        }
        next
    }

    /// The chain that starts with the unused piece `first` of `pieces` and runs on through the
    /// first unused piece leaving each end, marking each piece used, until none is left or, when
    /// `closing` says so, it returns to its start.
    fn follow(
        &mut self,
        pieces: &[(V, V)],
        first: usize,
        closing: Closing,
        used: &mut [bool],
    ) -> Chain {
        let mut chain = Vec::new();
        let start = pieces.get(first).map(|&(from, _)| from);
        let mut current = first;
        while let Some(&(_, to)) = pieces.get(current) {
            if let Some(flag) = used.get_mut(current) {
                *flag = true;
            }
            chain.push(current);
            if closing == Closing::AtStart && Some(to) == start {
                return Chain {
                    pieces: chain,
                    closed: true,
                };
            }
            match self.next_unused(to, used) {
                Some(next) => current = next,
                None => break,
            }
        }
        Chain {
            pieces: chain,
            closed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// The chains of `pieces` as joining found them before vertices kept a cursor: each lookup
    /// scans the pieces leaving the vertex from the first, so `lines` and `rings` must agree
    /// with it on every input.
    fn rescanned(pieces: &[(u8, u8)], open_first: bool) -> Vec<Chain> {
        let next_unused = |vertex: u8, used: &[bool]| {
            (0..pieces.len()).find(|&i| !used[i] && pieces[i].0 == vertex)
        };
        let follow = |first: usize, closing: bool, used: &mut Vec<bool>| {
            let (start, mut current, mut chain) = (pieces[first].0, first, Vec::new());
            loop {
                used[current] = true;
                chain.push(current);
                let to = pieces[current].1;
                if closing && to == start {
                    return Chain {
                        pieces: chain,
                        closed: true,
                    };
                }
                match next_unused(to, used) {
                    Some(next) => current = next,
                    None => {
                        return Chain {
                            pieces: chain,
                            closed: false,
                        };
                    }
                }
            }
        };
        let mut used = vec![false; pieces.len()];
        let mut chains = Vec::new();
        if open_first {
            let mut excess: HashMap<u8, isize> = HashMap::new();
            for &(from, to) in pieces {
                *excess.entry(from).or_insert(0) += 1;
                *excess.entry(to).or_insert(0) -= 1;
            }
            for first in 0..pieces.len() {
                let from = pieces[first].0;
                if !used[first] && excess[&from] > 0 {
                    *excess.entry(from).or_insert(0) -= 1;
                    chains.push(follow(first, false, &mut used));
                }
            }
        }
        for first in 0..pieces.len() {
            if !used[first] {
                chains.push(follow(first, true, &mut used));
            }
        }
        chains
    }

    proptest! {
        #[test]
        fn a_cursor_per_vertex_joins_as_a_scan_from_the_first_piece_does(
            pieces in prop::collection::vec((0_u8..6, 0_u8..6), 0..80),
        ) {
            // Few vertices and many pieces: vertices left and reached many times over.
            prop_assert_eq!(lines(&pieces), rescanned(&pieces, true));
            let rings: Vec<Chain> = rescanned(&pieces, false);
            let expected: Vec<Vec<usize>> = rings.into_iter().map(|chain| chain.pieces).collect();
            prop_assert_eq!(super::rings(&pieces), expected);
        }
    }

    #[test]
    fn open_lines_come_first_and_loops_close() {
        // A loop 0 → 1 → 2 → 0, then a line 5 → 6 → 7.
        let pieces = [(0, 1), (1, 2), (6, 7), (2, 0), (5, 6)];
        let lines = lines(&pieces);
        assert_eq!(
            lines,
            [
                Chain {
                    pieces: vec![4, 2],
                    closed: false
                },
                Chain {
                    pieces: vec![0, 1, 3],
                    closed: true
                },
            ]
        );
        assert_eq!(rings(&[(0, 1), (1, 2), (2, 0)]), [vec![0, 1, 2]]);
    }

    #[test]
    fn a_vertex_left_twice_takes_the_first_unused_piece() {
        // Two loops through vertex 0: 0 → 1 → 0 and 0 → 2 → 0.
        let pieces = [(0, 1), (0, 2), (1, 0), (2, 0)];
        assert_eq!(rings(&pieces), [vec![0, 2], vec![1, 3]]);
        // An open line passing through its start runs on: 3 → 0 → 1 → 0 → 4 has its start
        // vertex 3 left once more than reached.
        let pieces = [(3, 0), (0, 1), (1, 0), (0, 4)];
        assert_eq!(
            lines(&pieces),
            [Chain {
                pieces: vec![0, 1, 2, 3],
                closed: false
            }]
        );
    }
}
