use cubesim::{parse_scramble, Cube, FaceletCube, Move, MoveVariant, PruningTable, Solver};
use eframe::egui;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
static PRUNING_TABLE_DEPTH: AtomicI32 = AtomicI32::new(0);
static STICKER_NOTATION: AtomicBool = AtomicBool::new(false);
static CHEAP_MOVES: AtomicU32 = AtomicU32::new(0);

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

    let pruning_table =
        PruningTable::new(&initial_states, PRUNING_TABLE_DEPTH.load(SeqCst), &move_set);

    Solver::new(move_set, pruning_table)
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        follow_system_theme: false,
        ..Default::default()
    };
    eframe::run_native(
        "RocKeT",
        native_options,
        Box::new(|cc| Box::new(App::new(cc))),
    )
}

struct App {
    alg: String,
    cheap_moves: String,
    max_depth: usize,
    all: bool,
    output: Arc<Mutex<String>>,
}
impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        PRUNING_TABLE_DEPTH.store(2 as i32, SeqCst);
        let _ = &*NAIVE_SOLVER;
        Self {
            alg: String::new(),
            cheap_moves: String::new(),
            max_depth: 5,
            all: false,
            output: Arc::new(Mutex::new("".to_string())),
        }
    }
}
impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Alg: ");
                egui::TextEdit::singleline(&mut self.alg)
                    .hint_text("eg. R U2 R2 U' R2 U' R2 U2 R ...")
                    .show(ui);
            });
            ui.horizontal(|ui| {
                ui.label("Cheap moves: ");
                egui::TextEdit::singleline(&mut self.cheap_moves)
                    .hint_text("eg. xy2 xz2 y2 ...")
                    .show(ui);
            });
            ui.horizontal(|ui| {
                let label = ui.label("Max depth: ");
                ui.add(egui::Slider::new(&mut self.max_depth, 0..=10))
                    .labelled_by(label.id);
            });
            ui.checkbox(&mut self.all, "Show all algs");
            if ui.button("Run").clicked() {
                let alg = parse_scramble(self.alg.clone());
                let cheap_move_set: HashSet<_> = self
                    .cheap_moves
                    .split_ascii_whitespace()
                    .map(|s| format!(" O{} ", s))
                    .collect();
                let mut cheap_move_set_mask = 0;
                for (i, r) in Reorient::ALL.iter().enumerate() {
                    if cheap_move_set.contains(&r.to_string()) {
                        cheap_move_set_mask |= 1 << i;
                    }
                }
                CHEAP_MOVES.store(cheap_move_set_mask, SeqCst);
                *self.output.lock().unwrap() = String::new();

                // let output = Arc::new(Mutex::new(String::new()));
                let output_ref = Arc::clone(&self.output);
                let max_depth = self.max_depth;
                let all = self.all;
                std::thread::spawn(move || {
                    let (reorient_count, mut solutions) = iddfs(&alg, max_depth, &output_ref);
                    let mut output = output_ref.lock().unwrap();
                    let solution_count = solutions.len();
                    if solution_count == 0 {
                        *output += "No solutions?\n";
                    } else {
                        let stm = alg.len() + reorient_count;
                        *output += &format!(
                            "Found {solution_count} solutions with \
                        {reorient_count} reorients ({stm} STM).\n"
                        );
                        if !all {
                            let min_cost =
                                *solutions.iter().map(|(cost, _string)| cost).min().unwrap();
                            solutions.retain(|(cost, _string)| *cost == min_cost);
                            let good_solution_count = solutions.len();
                            *output += &format!(
                                "{good_solution_count} of them add only {min_cost} ETM.\n"
                            );
                        }
                        for (_cost, string) in solutions {
                            *output += &format!("{}\n", string);
                        }
                    }
                });
            }
            egui::scroll_area::ScrollArea::vertical()
                .show(ui, |ui| ui.label(self.output.lock().unwrap().to_string()));
        });
        ctx.request_repaint();
    }
}

fn iddfs(
    moves: &[Move],
    max_depth: usize,
    output: &Arc<Mutex<String>>,
) -> (usize, Vec<(usize, String)>) {
    if moves.len() <= 1 {
        return (
            0,
            vec![(
                0,
                moves.first().copied().map(display_move).unwrap_or_default(),
            )],
        );
    }

    for max_reorients in 0..std::cmp::min(moves.len(), max_depth + 1) {
        *output.lock().unwrap() +=
            &format!("Searching solutions with {} reorients\n", max_reorients);
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

    (0, vec![])
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
    None = 0,

    R = 1,
    L = 2,
    U = 3,
    D = 4,
    F = 5,
    B = 6,

    R2 = 7,
    U2 = 8,
    F2 = 9,

    UF = 10,
    UR = 11,
    FR = 12,
    DF = 13,
    UL = 14,
    BR = 15,

    UFR = 16,
    DBL = 17,
    UFL = 18,
    DBR = 19,
    DFR = 20,
    UBL = 21,
    UBR = 22,
    DFL = 23,
}
impl fmt::Display for Reorient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use Reorient::*;

        let s = STICKER_NOTATION.load(SeqCst);

        match self {
            None => write!(f, " "),

            R => write!(f, " {} ", if s { "23I:L" } else { "Ox" }),
            L => write!(f, " {} ", if s { "23I:R" } else { "Ox'" }),
            U => write!(f, " {} ", if s { "23I:D" } else { "Oy" }),
            D => write!(f, " {} ", if s { "23I:U" } else { "Oy'" }),
            F => write!(f, " {} ", if s { "23I:B" } else { "Oz" }),
            B => write!(f, " {} ", if s { "23I:F" } else { "Oz'" }),

            R2 => write!(f, " {} ", if s { "23I:R2" } else { "Ox2" }),
            U2 => write!(f, " {} ", if s { "23I:U2" } else { "Oy2" }),
            F2 => write!(f, " {} ", if s { "23I:F2" } else { "Oz2" }),

            UF => write!(f, " {} ", if s { "23I:UF" } else { "Oxy2" }),
            UR => write!(f, " {} ", if s { "23I:UR" } else { "Ozx2" }),
            FR => write!(f, " {} ", if s { "23I:FR" } else { "Oyz2" }),
            DF => write!(f, " {} ", if s { "23I:DF" } else { "Oxz2" }),
            UL => write!(f, " {} ", if s { "23I:UL" } else { "Ozy2" }),
            BR => write!(f, " {} ", if s { "23I:BR" } else { "Oyx2" }),

            UFR => write!(f, " {} ", if s { "23I:DBL" } else { "Oxy" }),
            DBL => write!(f, " {} ", if s { "23I:UFR" } else { "Oy'x'" }),
            UFL => write!(f, " {} ", if s { "23I:DBR" } else { "Ozy" }),
            DBR => write!(f, " {} ", if s { "23I:UFL" } else { "Oxy'" }),
            DFR => write!(f, " {} ", if s { "23I:UBL" } else { "Oxz" }),
            UBL => write!(f, " {} ", if s { "23I:DFR" } else { "Oyz'" }),
            UBR => write!(f, " {} ", if s { "23I:DFL" } else { "Oyx" }),
            DFL => write!(f, " {} ", if s { "23I:UBR" } else { "Ozx'" }),
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

        if (CHEAP_MOVES.load(SeqCst) >> self as u32) & 1 != 0 && self != Self::None {
            return 1;
        }

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
