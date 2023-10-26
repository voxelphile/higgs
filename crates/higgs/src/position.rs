use std::ops;

use higgs::consts::{CHUNK_AXIS, REGION_AXIS};
use nalgebra::{SVD, SVector};

#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct ChunkPosition(SVector<u64, 3>);
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct RegionPosition(SVector<u64, 3>);
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct GlobalPosition(SVector<i64, 3>);

impl RegionPosition {
    pub fn to_chunk_pos(self) -> ChunkPosition {
        ChunkPosition((self % CHUNK_AXIS as u64).0)
    }
}

impl GlobalPosition {
    pub fn to_region_pos(self) -> RegionPosition {
        RegionPosition(nalgebra::try_convert((self % REGION_AXIS as i64).0).unwrap())
    }
}

macro_rules! linearization {
    ($type:ty, $axis:expr) => {
        impl $type {
            pub fn linearize(self) -> u64 {
                (self.0[2] * $axis + self.0[1]) * $axis + self.0[0]
            }
            pub fn delineraize(index: u64) -> Self {
                let mut idx = index;
                let z = idx / $axis.pow(2);
                idx -= z * $axis.pow(2);
                let y = idx / $axis;
                let x = idx % $axis;
                Self::new(x,y,z)
            }
        }
    };
}

macro_rules! constructor {
    ($type:ty, $num:ty) => {
        impl $type {
            pub fn new(x: $num, y: $num, z: $num) -> Self {
                Self(SVector::<$num, 3>::new(x,y,z))
            }
        }
    };
}

macro_rules! math_operators {
    ($type:ty, $num:ty) => {
        impl ops::Add for $type {
            type Output = Self;

            fn add(self, rhs: Self) -> Self::Output {
                Self(self.0 + rhs.0)
            }
        }

        impl ops::Sub for $type {
            type Output = Self;

            fn sub(self, rhs: Self) -> Self::Output {
                Self(self.0 - rhs.0)
            }
        }

        impl ops::Mul for $type {
            type Output = Self;

            fn mul(self, rhs: Self) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].mul(rhs.0[0]),
                    self.0[1].mul(rhs.0[1]),
                    self.0[2].mul(rhs.0[2]),
                ))
            }
        }

        impl ops::Div for $type {
            type Output = Self;

            fn div(self, rhs: Self) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].div_euclid(rhs.0[0]),
                    self.0[1].div_euclid(rhs.0[1]),
                    self.0[2].div_euclid(rhs.0[2]),
                ))
            }
        }

        impl ops::Rem for $type {
            type Output = Self;

            fn rem(self, rhs: Self) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].rem_euclid(rhs.0[0]),
                    self.0[1].rem_euclid(rhs.0[1]),
                    self.0[2].rem_euclid(rhs.0[2]),
                ))
            }
        }

        impl ops::Add<$num> for $type {
            type Output = Self;

            fn add(self, rhs: $num) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].add(rhs),
                    self.0[1].add(rhs),
                    self.0[2].add(rhs),
                ))
            }
        }

        impl ops::Sub<$num> for $type {
            type Output = Self;

            fn sub(self, rhs: $num) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].sub(rhs),
                    self.0[1].sub(rhs),
                    self.0[2].sub(rhs),
                ))
            }
        }

        impl ops::Mul<$num> for $type {
            type Output = Self;

            fn mul(self, rhs: $num) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].mul(rhs),
                    self.0[1].mul(rhs),
                    self.0[2].mul(rhs),
                ))
            }
        }

        impl ops::Div<$num> for $type {
            type Output = Self;

            fn div(self, rhs: $num) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].div_euclid(rhs),
                    self.0[1].div_euclid(rhs),
                    self.0[2].div_euclid(rhs),
                ))
            }
        }

        impl ops::Rem<$num> for $type {
            type Output = Self;

            fn rem(self, rhs: $num) -> Self::Output {
                Self(SVector::<$num, 3>::new(
                    self.0[0].rem_euclid(rhs),
                    self.0[1].rem_euclid(rhs),
                    self.0[2].rem_euclid(rhs),
                ))
            }
        }
    };
}

constructor!(ChunkPosition, u64);
constructor!(RegionPosition, u64);
constructor!(GlobalPosition, i64);
linearization!(ChunkPosition, CHUNK_AXIS as u64);
linearization!(RegionPosition, REGION_AXIS as u64);
math_operators!(ChunkPosition, u64);
math_operators!(RegionPosition, u64);
math_operators!(GlobalPosition, i64);