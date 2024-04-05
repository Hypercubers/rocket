use std::{fmt::Display, ops::Add, str::FromStr};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Axis {
    X = 0,
    Y = 1,
    Z = 2,
}
impl Axis {
    fn u8(self) -> u8 {
        self.into()
    }
}
impl From<Axis> for u8 {
    fn from(value: Axis) -> Self {
        value as u8
    }
}
impl From<u8> for Axis {
    fn from(value: u8) -> Self {
        match value {
            0 => Axis::X,
            1 => Axis::Y,
            2 => Axis::Z,
            _ => panic!(""),
        }
    }
}
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub enum TurnMultiple {
    #[default]
    None = 0,
    Cw = 1,
    Half = 2,
    Ccw = 3,
}
impl From<TurnMultiple> for u8 {
    fn from(value: TurnMultiple) -> Self {
        value as u8
    }
}
impl From<u8> for TurnMultiple {
    fn from(value: u8) -> Self {
        match value {
            0 => TurnMultiple::None,
            1 => TurnMultiple::Cw,
            2 => TurnMultiple::Half,
            3 => TurnMultiple::Ccw,
            _ => panic!(""),
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
pub struct Move {
    bits: u8, // as AA0PP0NN for Axis, Positive, and Negative
}
impl Move {
    fn new(axis: Axis, positive: TurnMultiple, negative: TurnMultiple) -> Self {
        Self {
            bits: (u8::from(axis) << 6) | (u8::from(positive) << 3) | u8::from(negative),
        }
    }
    fn axis(self) -> Axis {
        (self.bits >> 6).into()
    }
    fn positive(self) -> TurnMultiple {
        ((self.bits >> 3) & 3).into()
    }
    fn negative(self) -> TurnMultiple {
        (self.bits & 3).into()
    }
    fn is_ident(self) -> bool {
        self.bits & 0b00_011_011 == 0
    }
    fn is_double_move(self) -> bool {
        (self.bits & 3 != 0) && ((self.bits >> 3) & 3 != 0)
    }
}
impl Add for Move {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        assert_eq!(self.axis(), rhs.axis());
        Self {
            bits: (self.bits + (rhs.bits & 0b00_011_011)) & 0b11_011_011,
        }
    }
}
impl FromStr for Move {
    type Err = String;

    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        let mut multiple = TurnMultiple::Cw;
        if let Some(prefix) = s.strip_suffix('\'') {
            s = prefix;
            multiple = TurnMultiple::Ccw;
        }
        if let Some(prefix) = s.strip_suffix('2') {
            s = prefix;
            multiple = TurnMultiple::Half;
        }
        match s {
            "R" => Ok(Self::new(Axis::X, multiple, TurnMultiple::None)),
            "U" => Ok(Self::new(Axis::Y, multiple, TurnMultiple::None)),
            "F" => Ok(Self::new(Axis::Z, multiple, TurnMultiple::None)),
            "L" => Ok(Self::new(Axis::X, TurnMultiple::None, multiple)),
            "D" => Ok(Self::new(Axis::Y, TurnMultiple::None, multiple)),
            "B" => Ok(Self::new(Axis::Z, TurnMultiple::None, multiple)),

            _ => Err(format!("unknown move {s:?}")),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Orientation {
    bits: u8, // 0xyzXXYY for the signs of XYZ and the position of X and Y
}
impl Orientation {
    fn z_bits(self) -> u8 {
        !(self.bits ^ (self.bits >> 2)) & 3
    }
    #[must_use]
    pub fn transform_move(self, m: Move) -> Move {
        let (axis, sign_flip) = self.transform_axis(m.axis());
        let (mut pos, mut neg) = (m.positive(), m.negative());
        if sign_flip {
            std::mem::swap(&mut pos, &mut neg);
        }
        Move::new(axis, pos, neg)
    }
    #[must_use]
    fn transform_axis(self, a: Axis) -> (Axis, bool) {
        (
            match a {
                Axis::X => (self.bits >> 2) & 3,
                Axis::Y => self.bits & 3,
                Axis::Z => self.z_bits(),
            }
            .into(),
            (self.bits & (1 << (6 - a as u8))) != 0,
        )
    }
    #[must_use]
    fn transform_signed_axis(self, a: (Axis, bool)) -> (Axis, bool) {
        let (a, s) = a;
        let (a, s2) = self.transform_axis(a);
        (a, s ^ s2)
    }
    #[must_use]
    pub fn transform_orientation(self, o: Orientation) -> Orientation {
        let (x, xflip) = self.transform_signed_axis(o.transform_axis(Axis::X));
        let (y, yflip) = self.transform_signed_axis(o.transform_axis(Axis::Y));
        let (_, zflip) = self.transform_signed_axis(o.transform_axis(Axis::Z));
        let xflip = xflip as u8;
        let yflip = yflip as u8;
        let zflip = zflip as u8;
        Orientation {
            bits: (xflip << 6) + (yflip << 5) + (zflip << 4) + (x.u8() << 2) + y.u8(),
        }
    }
}
impl Default for Orientation {
    fn default() -> Self {
        Self {
            bits: 0b0_000_00_01,
        }
    }
}
impl From<crate::Reorient> for Orientation {
    fn from(value: crate::Reorient) -> Self {
        Orientation {
            bits: match value {
                crate::Reorient::None => 0b0_000_00_01,
                crate::Reorient::R => 0b0_010_00_10,
                crate::Reorient::L => 0b0_001_00_10,
                crate::Reorient::U => 0b0_001_10_01,
                crate::Reorient::D => 0b0_100_10_01,
                crate::Reorient::F => 0b0_100_10_00,
                crate::Reorient::B => 0b0_010_10_00,
                crate::Reorient::R2 => 0b0_011_00_01,
                crate::Reorient::U2 => 0b0_101_00_01,
                crate::Reorient::F2 => 0b0_110_00_01,
                crate::Reorient::UF => 0b0_100_00_10,
                crate::Reorient::UR => 0b0_001_01_00,
                crate::Reorient::FR => 0b0_010_10_01,
                crate::Reorient::DF => 0b0_111_00_10,
                crate::Reorient::UL => 0b0_111_01_00,
                crate::Reorient::BR => 0b0_111_10_01,
                crate::Reorient::UFR => 0b0_000_10_00,
                crate::Reorient::DBL => 0b0_000_01_10,
                crate::Reorient::UFL => 0b0_101_01_10,
                crate::Reorient::DBR => 0b0_101_10_00,
                crate::Reorient::DFR => 0b0_110_01_10,
                crate::Reorient::UBL => 0b0_110_10_00,
                crate::Reorient::UBR => 0b0_011_01_10,
                crate::Reorient::DFL => 0b0_011_10_00,
            },
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
#[repr(align(32))]
pub struct CubeState {
    moves: [Move; 31],
    len: u8,
}
impl CubeState {
    #[must_use]
    pub fn apply_move(mut self, m: Move) -> Self {
        match self.last_mut() {
            Some(old_m) if old_m.axis() == m.axis() => {
                *old_m = *old_m + m;
                if old_m.is_ident() {
                    self.pop()
                } else {
                    self
                }
            }
            _ => self.push(m),
        }
    }
    pub fn last_mut(&mut self) -> Option<&mut Move> {
        Some(&mut self.moves[self.len.checked_sub(1)? as usize])
    }
    #[must_use]
    pub fn push(mut self, m: Move) -> Self {
        self.moves[self.len as usize] = m;
        self.len += 1;
        self
    }
    #[must_use]
    pub fn pop(mut self) -> Self {
        self.len = self.len.saturating_sub(1);
        self
    }
    pub fn is_solved(self) -> bool {
        self.len == 0
    }
    pub fn is_one_from_solved(self) -> bool {
        self.len == 1 && !self.moves[0].is_double_move()
    }
    pub fn lower_bound(self) -> u8 {
        self.moves[0..self.len as usize]
            .into_iter()
            .map(|m| match m.is_double_move() {
                true => 2,
                false => 1,
            })
            .sum()
    }
}
impl Display for CubeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.len == 0 {
            return Ok(());
        }
        write!(f, "{}", display_move(self.moves[0]))?;
        for &m in &self.moves[1..self.len as usize] {
            write!(f, " {}", display_move(m))?;
        }
        Ok(())
    }
}

pub fn display_move(m: Move) -> String {
    let (p, n) = match m.axis() {
        Axis::X => ("R", "L"),
        Axis::Y => ("U", "D"),
        Axis::Z => ("F", "B"),
    };
    let (p, n) = (p.to_owned(), n.to_owned());
    (match m.positive() {
        TurnMultiple::None => String::new(),
        TurnMultiple::Cw => p,
        TurnMultiple::Half => p + "2",
        TurnMultiple::Ccw => p + "'",
    } + &match m.negative() {
        TurnMultiple::None => String::new(),
        TurnMultiple::Cw => n,
        TurnMultiple::Half => n + "2",
        TurnMultiple::Ccw => n + "'",
    })
}

pub fn parse_moves(moves: &String) -> Result<Vec<Move>, String> {
    moves.split_ascii_whitespace().map(str::parse).collect()
}
