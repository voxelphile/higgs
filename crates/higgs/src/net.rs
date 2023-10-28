use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{position::RegionId, Operation, Region};

#[derive(Serialize, Deserialize)]
pub enum Request {
    Subscribe(Vec<RegionId>),
    Perform(HashMap<RegionId, Vec<Operation>>),
}

#[derive(Serialize, Deserialize)]
pub enum Response {
    Publish(HashMap<RegionId, Vec<Operation>>),
    Refresh(HashMap<RegionId, Region>),
}
