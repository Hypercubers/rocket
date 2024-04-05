use eframe::egui;
use std::collections::HashSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
static STICKER_NOTATION: AtomicBool = AtomicBool::new(false);
static CHEAP_MOVES: AtomicU32 = AtomicU32::new(0);

mod cube;

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
                match cube::parse_moves(&self.alg) {
                    Ok(alg) => {
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
                            let (reorient_count, mut solutions) =
                                iddfs(&alg, max_depth, &output_ref);
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
                                    let min_cost = *solutions
                                        .iter()
                                        .map(|(cost, _string)| cost)
                                        .min()
                                        .unwrap();
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
                    Err(string) => *self.output.lock().unwrap() = string,
                }
            }
            egui::scroll_area::ScrollArea::vertical()
                .show(ui, |ui| ui.label(self.output.lock().unwrap().to_string()));
        });
        ctx.request_repaint();
    }
}

fn iddfs(
    moves: &[cube::Move],
    max_depth: usize,
    output: &Arc<Mutex<String>>,
) -> (usize, Vec<(usize, String)>) {
    if moves.len() <= 1 {
        return (
            0,
            vec![(
                0,
                moves
                    .first()
                    .copied()
                    .map(cube::display_move)
                    .unwrap_or_default(),
            )],
        );
    }

    for max_reorients in 0..std::cmp::min(moves.len(), max_depth + 1) {
        *output.lock().unwrap() +=
            &format!("Searching solutions with {} reorients\n", max_reorients);
        let mut ret = vec![];
        dfs(
            cube::CubeState::default(),
            cube::Orientation::default(),
            moves,
            Solution::default(),
            0,
            max_reorients,
            &mut ret,
        );
        if !ret.is_empty() {
            let solutions = ret
                .into_iter()
                .map(|solution| {
                    // Solutions are reversed, because reasons.
                    let solution_iter = solution.reorients(moves.len()).into_iter();

                    let mut return_string = String::new();
                    for (reorient, &mv) in solution_iter.zip(&moves[0..]) {
                        if let Some(reorient) = reorient {
                            return_string += &reorient.to_string();
                        }
                        return_string += &cube::display_move(mv);
                    }

                    let cost = solution
                        .reorients(moves.len())
                        .iter()
                        .map(|&r| if let Some(r) = r { r.cost() } else { 0 })
                        .sum();

                    (cost, return_string)
                })
                .collect();
            return (max_reorients, solutions);
        }
    }

    (0, vec![])
}

fn dfs(
    mut state: cube::CubeState,
    orientation: cube::Orientation,
    moves: &[cube::Move],
    solution: Solution,
    index: u8,
    max_reorients: usize,
    output: &mut Vec<Solution>,
) {
    if moves.len() <= 1 || max_reorients == 0 {
        // No more reorients allowed! Are we already solved?
        for m in moves {
            state = state.apply_move(orientation.transform_move(*m));
        }
        if state.is_solved() || state.is_one_from_solved() {
            // Success!
            output.push(solution)
        } else {
            // Fail!
        }
    } else if state.lower_bound() as usize > moves.len() + 1 {
        // Fail!
    } else {
        // Try not reorienting right now.
        let new_state = state.apply_move(orientation.transform_move(moves[0]));

        // Try every possible reorient, including the null reorient.
        for &reorient in Reorient::ALL {
            let remaining_reorients = max_reorients - 1 + reorient.is_none() as usize;
            let reorientation: cube::Orientation = reorient.into();
            let new_orientation = reorientation.transform_orientation(orientation);
            let new_solution = solution.push_if_not_ident(reorient, index + 1);
            dfs(
                new_state,
                new_orientation,
                &moves[1..],
                new_solution,
                index + 1,
                remaining_reorients,
                output,
            )
        }
    }
}

/// Reorientations between each move.
#[derive(Debug, Default, Copy, Clone)]
#[repr(align(16))]
pub struct Solution {
    reorients: [(u8, Reorient); 7],
    len: u8,
}
impl Solution {
    pub fn push_if_not_ident(mut self, reorient: Reorient, index: u8) -> Self {
        if !reorient.is_none() {
            self.reorients[self.len as usize] = (index, reorient);
            self.len += 1;
        }
        self
    }
    pub fn pop(mut self) -> Self {
        self.len = self.len.saturating_sub(1);
        self
    }
    pub fn reorients(self, movecount: usize) -> Vec<Option<Reorient>> {
        let mut vec = vec![None; movecount];
        for &(index, reorient) in self.reorients[0..self.len as usize].iter() {
            vec[index as usize] = Some(reorient);
        }
        vec
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Reorient {
    #[default]
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

    pub fn is_none(self) -> bool {
        self == Self::None
    }
}
