//! Pure game logic. Nothing in this module touches the browser, so it is
//! unit-tested natively with `cargo test`.

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Hidden,
    Flagged,
    Revealed,
}

#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub mine: bool,
    /// Number of mines in the 8 surrounding cells.
    pub adjacent: u8,
    pub mark: Mark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Board created, mines not placed yet (they are placed on the first dig).
    Ready,
    Playing,
    Won,
    Lost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub rows: usize,
    pub cols: usize,
    pub mines: usize,
}

pub const MIN_SIDE: usize = 5;
pub const MAX_SIDE: usize = 30;

impl Config {
    /// Largest mine count that still leaves room for the 3x3 safe opening.
    pub fn max_mines(rows: usize, cols: usize) -> usize {
        (rows * cols).saturating_sub(9).max(1)
    }

    /// Validates a user supplied (custom) configuration.
    pub fn custom(rows: usize, cols: usize, mines: usize) -> Result<Config, String> {
        let side = MIN_SIDE..=MAX_SIDE;
        if !side.contains(&rows) || !side.contains(&cols) {
            return Err(format!(
                "Rows and columns must be between {MIN_SIDE} and {MAX_SIDE}."
            ));
        }
        let max = Config::max_mines(rows, cols);
        if mines < 1 || mines > max {
            return Err(format!(
                "Mines must be between 1 and {max} for a {rows} × {cols} board."
            ));
        }
        Ok(Config { rows, cols, mines })
    }
}

/// Small, fast xorshift64* generator. Seeded from the browser at runtime and
/// from fixed seeds in tests, which keeps the engine deterministic.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform-enough integer in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

pub struct Board {
    pub config: Config,
    cells: Vec<Cell>,
    status: Status,
    flags: usize,
    revealed: usize,
    exploded: Option<usize>,
    rng: Rng,
}

impl Board {
    pub fn new(config: Config, seed: u64) -> Board {
        let total = config.rows * config.cols;
        let config = Config {
            mines: config.mines.clamp(1, total - 1),
            ..config
        };
        Board {
            config,
            cells: vec![
                Cell {
                    mine: false,
                    adjacent: 0,
                    mark: Mark::Hidden
                };
                total
            ],
            status: Status::Ready,
            flags: 0,
            revealed: 0,
            exploded: None,
            rng: Rng::new(seed),
        }
    }

    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    pub fn cell(&self, i: usize) -> Cell {
        self.cells[i]
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn is_over(&self) -> bool {
        matches!(self.status, Status::Won | Status::Lost)
    }

    pub fn exploded(&self) -> Option<usize> {
        self.exploded
    }

    /// Mines minus flags. Can go negative when the player over-flags.
    pub fn mines_left(&self) -> i32 {
        self.config.mines as i32 - self.flags as i32
    }

    pub fn row_col(&self, i: usize) -> (usize, usize) {
        (i / self.config.cols, i % self.config.cols)
    }

    pub fn index(&self, row: usize, col: usize) -> usize {
        row * self.config.cols + col
    }

    pub fn neighbors(&self, i: usize) -> Vec<usize> {
        let (r, c) = self.row_col(i);
        let (rows, cols) = (self.config.rows as isize, self.config.cols as isize);
        let mut out = Vec::with_capacity(8);
        for dr in -1..=1isize {
            for dc in -1..=1isize {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let (nr, nc) = (r as isize + dr, c as isize + dc);
                if nr >= 0 && nr < rows && nc >= 0 && nc < cols {
                    out.push(self.index(nr as usize, nc as usize));
                }
            }
        }
        out
    }

    /// Places mines after the first dig so the first cell is always safe and,
    /// whenever the density allows, opens up (its neighbours are safe too).
    fn place_mines(&mut self, safe: usize) {
        let total = self.cell_count();
        let mut zone = self.neighbors(safe);
        zone.push(safe);
        if total - zone.len() < self.config.mines {
            zone = vec![safe];
        }

        let mut candidates: Vec<usize> = (0..total).filter(|i| !zone.contains(i)).collect();
        // Partial Fisher–Yates shuffle: the first `mines` entries become mines.
        for k in 0..self.config.mines {
            let j = k + self.rng.below(candidates.len() - k);
            candidates.swap(k, j);
            self.cells[candidates[k]].mine = true;
        }

        for i in 0..total {
            let count = self
                .neighbors(i)
                .iter()
                .filter(|&&n| self.cells[n].mine)
                .count();
            self.cells[i].adjacent = count as u8;
        }
    }

    /// Digs a hidden cell. Returns the cells that were revealed, in flood-fill
    /// order (handy for staggered animations).
    pub fn reveal(&mut self, i: usize) -> Vec<usize> {
        if self.is_over() || self.cells[i].mark != Mark::Hidden {
            return Vec::new();
        }
        if self.status == Status::Ready {
            self.place_mines(i);
            self.status = Status::Playing;
        }

        if self.cells[i].mine {
            self.cells[i].mark = Mark::Revealed;
            self.exploded = Some(i);
            self.status = Status::Lost;
            return vec![i];
        }

        let mut opened = Vec::new();
        let mut queue = VecDeque::from([i]);
        self.cells[i].mark = Mark::Revealed;
        while let Some(cur) = queue.pop_front() {
            opened.push(cur);
            self.revealed += 1;
            if self.cells[cur].adjacent != 0 {
                continue;
            }
            for n in self.neighbors(cur) {
                // Flags are respected: a flagged cell is never auto-opened.
                if self.cells[n].mark == Mark::Hidden && !self.cells[n].mine {
                    self.cells[n].mark = Mark::Revealed;
                    queue.push_back(n);
                }
            }
        }

        if self.revealed == self.cell_count() - self.config.mines {
            self.win();
        }
        opened
    }

    /// "Chording": digging a revealed number whose flag count matches opens
    /// every remaining hidden neighbour at once.
    pub fn chord(&mut self, i: usize) -> Vec<usize> {
        let cell = self.cells[i];
        if self.is_over() || cell.mark != Mark::Revealed || cell.adjacent == 0 {
            return Vec::new();
        }
        let neighbors = self.neighbors(i);
        let flagged = neighbors
            .iter()
            .filter(|&&n| self.cells[n].mark == Mark::Flagged)
            .count();
        if flagged != cell.adjacent as usize {
            return Vec::new();
        }
        let mut opened = Vec::new();
        for n in neighbors {
            opened.extend(self.reveal(n));
        }
        opened
    }

    /// Dig if hidden, chord if it is a revealed number.
    pub fn dig(&mut self, i: usize) -> Vec<usize> {
        match self.cells[i].mark {
            Mark::Hidden => self.reveal(i),
            Mark::Revealed => self.chord(i),
            Mark::Flagged => Vec::new(),
        }
    }

    /// Toggles a flag. Returns true when something changed.
    pub fn toggle_flag(&mut self, i: usize) -> bool {
        if self.is_over() {
            return false;
        }
        let cell = &mut self.cells[i];
        match cell.mark {
            Mark::Hidden => {
                cell.mark = Mark::Flagged;
                self.flags += 1;
                true
            }
            Mark::Flagged => {
                cell.mark = Mark::Hidden;
                self.flags -= 1;
                true
            }
            Mark::Revealed => false,
        }
    }

    fn win(&mut self) {
        self.status = Status::Won;
        for cell in self.cells.iter_mut().filter(|c| c.mine) {
            cell.mark = Mark::Flagged;
        }
        self.flags = self.config.mines;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(rows: usize, cols: usize, mines: usize) -> Config {
        Config { rows, cols, mines }
    }

    fn mine_count(b: &Board) -> usize {
        (0..b.cell_count()).filter(|&i| b.cell(i).mine).count()
    }

    #[test]
    fn first_dig_is_safe_and_opens_area() {
        for seed in 1..200 {
            let mut b = Board::new(cfg(9, 9, 10), seed);
            let start = b.index(4, 4);
            let opened = b.reveal(start);
            assert_ne!(b.status(), Status::Lost);
            assert_eq!(b.cell(start).adjacent, 0, "first cell should be an opening");
            assert!(opened.len() > 1);
            assert_eq!(mine_count(&b), 10);
        }
    }

    #[test]
    fn first_dig_safe_at_max_density() {
        for seed in 1..50 {
            let mut b = Board::new(cfg(5, 5, 24), seed);
            b.reveal(0);
            assert_eq!(b.status(), Status::Won);
            assert_eq!(mine_count(&b), 24);
        }
    }

    #[test]
    fn adjacency_matches_neighbours() {
        let mut b = Board::new(cfg(16, 30, 99), 7);
        b.reveal(0);
        for i in 0..b.cell_count() {
            let expected = b.neighbors(i).iter().filter(|&&n| b.cell(n).mine).count();
            assert_eq!(b.cell(i).adjacent as usize, expected);
        }
    }

    #[test]
    fn neighbours_at_corners_and_edges() {
        let b = Board::new(cfg(5, 6, 3), 1);
        assert_eq!(b.neighbors(0).len(), 3);
        assert_eq!(b.neighbors(b.index(0, 3)).len(), 5);
        assert_eq!(b.neighbors(b.index(2, 2)).len(), 8);
        assert_eq!(b.neighbors(b.cell_count() - 1).len(), 3);
    }

    #[test]
    fn digging_a_mine_loses() {
        let mut b = Board::new(cfg(9, 9, 10), 3);
        b.reveal(0);
        let mine = (0..b.cell_count()).find(|&i| b.cell(i).mine).unwrap();
        assert_eq!(b.reveal(mine), vec![mine]);
        assert_eq!(b.status(), Status::Lost);
        assert_eq!(b.exploded(), Some(mine));
        // No further moves once the game is over.
        assert!(b.reveal(b.cell_count() - 1).is_empty());
        assert!(!b.toggle_flag(b.cell_count() - 1));
    }

    #[test]
    fn revealing_every_safe_cell_wins_and_flags_mines() {
        let mut b = Board::new(cfg(9, 9, 10), 11);
        b.reveal(40);
        for i in 0..b.cell_count() {
            if !b.cell(i).mine {
                b.reveal(i);
            }
        }
        assert_eq!(b.status(), Status::Won);
        assert_eq!(b.mines_left(), 0);
        assert!((0..b.cell_count())
            .filter(|&i| b.cell(i).mine)
            .all(|i| b.cell(i).mark == Mark::Flagged));
    }

    #[test]
    fn flags_block_digging_and_flood_fill() {
        let mut b = Board::new(cfg(9, 9, 10), 5);
        assert!(b.toggle_flag(80));
        assert_eq!(b.mines_left(), 9);
        assert!(b.dig(80).is_empty());
        b.reveal(40);
        assert_eq!(b.cell(80).mark, Mark::Flagged);
        assert!(b.toggle_flag(80));
        assert_eq!(b.mines_left(), 10);
    }

    #[test]
    fn chord_opens_neighbours_when_flags_match() {
        // Find a seed/board with a revealed "1" next to hidden cells.
        for seed in 1..500 {
            let mut b = Board::new(cfg(9, 9, 10), seed);
            b.reveal(40);
            let Some(num) = (0..b.cell_count()).find(|&i| {
                let c = b.cell(i);
                c.mark == Mark::Revealed
                    && c.adjacent == 1
                    && b.neighbors(i)
                        .iter()
                        .any(|&n| b.cell(n).mark == Mark::Hidden && !b.cell(n).mine)
            }) else {
                continue;
            };
            // Wrong flag count: nothing happens.
            assert!(b.chord(num).is_empty());
            let mine = *b.neighbors(num).iter().find(|&&n| b.cell(n).mine).unwrap();
            b.toggle_flag(mine);
            let opened = b.dig(num);
            assert!(!opened.is_empty());
            assert_ne!(b.status(), Status::Lost);
            assert!(b
                .neighbors(num)
                .iter()
                .all(|&n| b.cell(n).mark != Mark::Hidden));
            return;
        }
        panic!("no suitable board found");
    }

    #[test]
    fn custom_config_validation() {
        assert!(Config::custom(9, 9, 10).is_ok());
        assert!(Config::custom(4, 9, 10).is_err());
        assert!(Config::custom(9, 31, 10).is_err());
        assert!(Config::custom(9, 9, 0).is_err());
        assert!(Config::custom(9, 9, 73).is_err());
        assert!(Config::custom(9, 9, 72).is_ok());
    }

    #[test]
    fn same_seed_same_board() {
        let mut a = Board::new(cfg(16, 16, 40), 42);
        let mut b = Board::new(cfg(16, 16, 40), 42);
        a.reveal(0);
        b.reveal(0);
        assert!((0..a.cell_count()).all(|i| a.cell(i).mine == b.cell(i).mine));
    }
}
