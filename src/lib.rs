#![allow(dead_code)]

use std::{sync::{Arc, atomic::{AtomicUsize, AtomicPtr}, Mutex}, collections::HashMap, iter};
use dashmap::DashMap;
use left_right::{WriteHandle, ReadHandle, Absorb};
use voxels::Channel;

pub const REGION_AXIS: usize = 512;
pub const REGION_SIZE: usize = REGION_AXIS.pow(3);

pub const CHUNK_AXIS: usize = 8;
pub const CHUNK_SIZE: usize = CHUNK_AXIS.pow(3);

pub const WORLD_REGIONS_AXIS: usize = 1_000_000;
pub const WORLD_REGIONS_SIZE: usize = WORLD_REGIONS_AXIS.pow(3);

pub const CHUNKS_PER_REGION: usize = REGION_SIZE / CHUNK_SIZE;

pub type Epoch = Arc<AtomicUsize>;
pub type Epochs = Arc<Mutex<Vec<Epoch>>>;

#[repr(u64)]
#[derive(Clone, Copy)]
pub enum Block {
    Void,
    Air,
    Grass,
    Dirt,
    Stone,
}

pub struct Chunk {
    ids: Channel,
}

impl Default for Chunk {
    fn default() -> Self {
        let mut ids = Channel::default();
        ids.extend(iter::repeat(Block::Void).map(|block| block as u64).take(CHUNK_SIZE));
        Self { ids }
    }
}

pub struct Region {
    chunks: Vec<Chunk>
}

impl Absorb<Operation> for Region {

    fn absorb_first(&mut self, operation: &mut Operation, other: &Self) {
        todo!()
    }

    fn sync_with(&mut self, first: &Self) {
        todo!()
    }

    fn absorb_second(&mut self, mut operation: Operation, other: &Self) {
        Self::absorb_first(self, &mut operation, other)
    }

    fn drop_first(self: Box<Self>) {}

    fn drop_second(self: Box<Self>) {}
}

impl Default for Region {
    fn default() -> Self {
        Self {
            chunks: iter::repeat_with(|| Chunk::default()).collect()
        }
    }
}

pub enum Operation {
    //this signals that the read region has 
    //been updated but not the write region
    Watermark, 
    DoAVoxelOperation,
}

pub struct Access {
    writer: WriteHandle<Region, Operation>,
    reader: ReadHandle<Region>,
}

#[derive(Default)]
pub struct Higgs {
    regions: DashMap<usize, Access>,
}