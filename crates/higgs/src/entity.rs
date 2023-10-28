use crate::position::PrecisePosition;
use nalgebra::SVector;
use serde::{Deserialize, Serialize};

pub type EntityId = uuid::Uuid;

pub type PlayerId = uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Look(SVector<f32, 2>);

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum Entity {
    Player {
        player_id: PlayerId,
        position: PrecisePosition,
        look: Look,
    },
}
