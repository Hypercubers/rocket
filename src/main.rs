use cubesim::{parse_scramble, Cube, FaceletCube, Move, MoveVariant, PruningTable, Solver};
use lazy_static::lazy_static;
use std::{fmt, io::Write};

// minimum of 2
const PRUNING_TABLE_DEPTH: i32 = 4;

lazy_static! {
    static ref NAIVE_SOLVER: Solver = make_naive_solver();
}

fn make_naive_solver() -> Solver {
    use Move::{B, D, F, L, R, U};
    use MoveVariant::*;

    let faces = [R, L, U, D, B, F];
    let variants = [Standard, Double, Inverse];

    let move_set: Vec<Move> = faces
        .into_iter()
        .flat_map(|f| variants.into_iter().map(f))
        .collect();

    let initial_states: Vec<FaceletCube> = Reorient::ALL
        .iter()
        .map(|r| FaceletCube::new(3).apply_moves(r.equivalent_rkt_moves()))
        .collect();

    let pruning_table = PruningTable::new(&initial_states, PRUNING_TABLE_DEPTH, &move_set);

    Solver::new(move_set, pruning_table)
}

fn main() {
    println!(
        "Initializing pruning table to depth {} ...",
        PRUNING_TABLE_DEPTH
    );

    let _ = &*NAIVE_SOLVER;

    println!("Ready!");
    println!();

    loop {
        let mut alg_string = String::new();

        print!("Enter rotationless algorithm: ");
        std::io::stdout().flush().unwrap();
        match std::io::stdin().read_line(&mut alg_string) {
            Ok(0) => std::process::exit(0),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1)
            }
            _ => (),
        }
        println!();

        let alg = parse_scramble(alg_string);

        let (reorient_count, mut solutions) = iddfs(&alg);
        let solution_count = solutions.len();
        let stm = alg.len() + reorient_count;
        let min_cost = *solutions.iter().map(|(cost, _string)| cost).min().unwrap();
        solutions.retain(|(cost, _string)| *cost == min_cost);
        let good_solution_count = solutions.len();
        println!("Found {solution_count} solutions with {reorient_count} reorients ({stm} STM).");
        println!("{good_solution_count} of them add only {min_cost} ETM:");
        for (cost, string) in solutions {
            println!("{}", string);
        }
        println!();
    }
}

fn iddfs(moves: &[Move]) -> (usize, Vec<(usize, String)>) {
    if moves.len() <= 1 {
        return (
            0,
            vec![(
                0,
                moves.first().copied().map(display_move).unwrap_or_default(),
            )],
        );
    }

    for max_reorients in 0..moves.len() {
        println!("Searching solutions with {} reorients", max_reorients);
        let ret = dfs(&FaceletCube::new(3), moves, max_reorients);
        if !ret.is_empty() {
            let solutions = ret
                .into_iter()
                .map(|solution| {
                    // Solutions are reversed, because reasons.
                    let solution_iter = solution.iter().rev();

                    let mut return_string = display_move(moves[0]);
                    for (reorient, &mv) in solution_iter.zip(&moves[1..]) {
                        return_string += &reorient.to_string();
                        return_string += &display_move(mv);
                    }

                    let cost = solution.iter().map(|r| r.cost()).sum();

                    (cost, return_string)
                })
                .collect();
            return (max_reorients, solutions);
        }
    }

    panic!("no solution!")
}

fn dfs(state: &FaceletCube, moves: &[Move], max_reorients: usize) -> Vec<Solution> {
    if moves.len() <= 1 || max_reorients == 0 {
        // No more reorients allowed! Are we already solved?
        let end_result = state.apply_moves(moves);
        if NAIVE_SOLVER.lower_bound(&end_result) <= 1 {
            // Success!
            vec![vec![Reorient::None; moves.len().saturating_sub(1)]]
        } else {
            // Fail!
            vec![]
        }
    } else if NAIVE_SOLVER.lower_bound(state) as usize > moves.len() + 1 {
        // Fail!
        vec![]
    } else {
        let mut ret = vec![];

        // Try not reorienting right now.
        let new_state = state.apply_move(moves[0]);

        // Try every possible reorient, including the null reorient.
        for &reorient in Reorient::ALL {
            let remaining_reorients = max_reorients - 1 + reorient.is_none() as usize;
            ret.extend(
                dfs(
                    &new_state.apply_moves(reorient.equivalent_rkt_moves()),
                    &moves[1..],
                    remaining_reorients,
                )
                .into_iter()
                .map(|mut solution| {
                    solution.push(reorient);
                    solution
                }),
            );
        }

        ret
    }
}

/// Reorientations between each move.
pub type Solution = Vec<Reorient>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Reorient {
    None,

    R,
    L,
    U,
    D,
    F,
    B,

    R2,
    U2,
    F2,

    UF,
    UR,
    FR,
    DF,
    UL,
    BR,

    UFR,
    DBL,
    UFL,
    DBR,
    DFR,
    UBL,
    UBR,
    DFL,
}
impl fmt::Display for Reorient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Reorient::*;
        match self {
            None => write!(f, " "),

            R => write!(f, " Ox "),
            L => write!(f, " Ox' "),
            U => write!(f, " Oy "),
            D => write!(f, " Oy' "),
            F => write!(f, " Oz "),
            B => write!(f, " Oz' "),

            R2 => write!(f, " Ox2 "),
            U2 => write!(f, " Oy2 "),
            F2 => write!(f, " Oz2 "),

            UF => write!(f, " Oxy2 "),
            UR => write!(f, " Ozx2 "),
            FR => write!(f, " Oyz2 "),
            DF => write!(f, " Oxz2 "),
            UL => write!(f, " Ozy2 "),
            BR => write!(f, " Oyx2 "),

            UFR => write!(f, " Oxy "),
            DBL => write!(f, " Oy'x' "),
            UFL => write!(f, " Ozy "),
            DBR => write!(f, " Oxy' "), // (equivalent: z'x)
            DFR => write!(f, " Oxz "),
            UBL => write!(f, " Oyz' "), // (equivalent: z'y)
            UBR => write!(f, " Oyx "),
            DFL => write!(f, " Ozx' "), // (equivalent: y'z)
        }
    }
}
impl Reorient {
    pub const ALL: &'static [Self] = &[
        Self::None,
        Self::R,
        Self::L,
        Self::U,
        Self::D,
        Self::F,
        Self::B,
        Self::R2,
        Self::U2,
        Self::F2,
        Self::UF,
        Self::UR,
        Self::FR,
        Self::DF,
        Self::UL,
        Self::BR,
        Self::UFR,
        Self::DBL,
        Self::UFL,
        Self::DBR,
        Self::DFR,
        Self::UBL,
        Self::UBR,
        Self::DFL,
    ];

    pub fn cost(self) -> usize {
        use Reorient::*;

        match self {
            None => 0,
            R | L | U | D | F | B => 1,
            R2 | U2 | F2 => 2,
            UF | UR | FR | DF | UL | BR => 3,
            UFR | DBL | UFL | DBR | DFR | UBL | UBR | DFL => 2,
        }
    }

    pub fn equivalent_rkt_moves(self) -> &'static [Move] {
        use Move::{X, Y, Z};
        use MoveVariant::*;
        use Reorient::*;

        match self {
            None => &[],

            R => &[X(Standard)],
            L => &[X(Inverse)],
            U => &[Y(Standard)],
            D => &[Y(Inverse)],
            F => &[Z(Standard)],
            B => &[Z(Inverse)],

            R2 => &[X(Double)],
            U2 => &[Y(Double)],
            F2 => &[Z(Double)],

            UF => &[X(Standard), Y(Double)],
            UR => &[Z(Standard), X(Double)],
            FR => &[Y(Standard), Z(Double)],
            DF => &[X(Standard), Z(Double)],
            UL => &[Z(Standard), Y(Double)],
            BR => &[Y(Standard), X(Double)],

            UFR => &[X(Standard), Y(Standard)],
            DBL => &[Y(Inverse), X(Inverse)],
            UFL => &[Z(Standard), Y(Standard)],
            DBR => &[X(Standard), Y(Inverse)],
            DFR => &[X(Standard), Z(Standard)],
            UBL => &[Y(Standard), Z(Inverse)],
            UBR => &[Y(Standard), X(Standard)],
            DFL => &[Z(Standard), X(Inverse)],
        }
    }

    pub fn is_none(self) -> bool {
        self == Self::None
    }
}

pub fn display_move(mv: Move) -> String {
    match mv {
        Move::U(v) => "U".to_string() + display_move_variant(v),
        Move::L(v) => "L".to_string() + display_move_variant(v),
        Move::F(v) => "F".to_string() + display_move_variant(v),
        Move::R(v) => "R".to_string() + display_move_variant(v),
        Move::B(v) => "B".to_string() + display_move_variant(v),
        Move::D(v) => "D".to_string() + display_move_variant(v),
        Move::Uw(2, v) => "Uw".to_string() + display_move_variant(v),
        Move::Lw(2, v) => "Lw".to_string() + display_move_variant(v),
        Move::Fw(2, v) => "Fw".to_string() + display_move_variant(v),
        Move::Rw(2, v) => "Rw".to_string() + display_move_variant(v),
        Move::Bw(2, v) => "Bw".to_string() + display_move_variant(v),
        Move::Dw(2, v) => "Dw".to_string() + display_move_variant(v),
        Move::X(v) => "x".to_string() + display_move_variant(v),
        Move::Y(v) => "y".to_string() + display_move_variant(v),
        Move::Z(v) => "z".to_string() + display_move_variant(v),
        _ => panic!("unsupported move {:?}", mv),
    }
}
pub fn display_move_variant(v: MoveVariant) -> &'static str {
    match v {
        MoveVariant::Standard => "",
        MoveVariant::Double => "2",
        MoveVariant::Inverse => "'",
    }
}
