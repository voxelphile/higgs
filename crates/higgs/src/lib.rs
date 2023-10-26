#![allow(dead_code)]

use std::{sync::{Arc, atomic::{AtomicUsize, AtomicPtr}, Mutex}, collections::HashMap, iter, mem};
use consts::{REGION_SIZE, CHUNK_AXIS};
use dashmap::DashMap;
use left_right::{WriteHandle, ReadHandle, Absorb};
use nalgebra::SVector;
use position::{ChunkPosition, RegionPosition};
use voxels::Channel;

pub mod position;

pub mod consts {
    pub const REGION_AXIS: usize = 64;

    pub const REGION_SIZE: usize = REGION_AXIS.pow(3);

    pub const CHUNK_AXIS: usize = 8;

    pub const CHUNK_SIZE: usize = CHUNK_AXIS.pow(3);

    pub const WORLD_AXIS: usize = 1_000_000;

    pub const WORLD_SIZE: usize = WORLD_AXIS.pow(3);

    pub const CHUNKS_PER_REGION: usize = REGION_SIZE / CHUNK_SIZE;
}

pub type RegionId = u64;

#[repr(u64)]
#[derive(Clone, Copy)]
pub enum Block {
    Void,
    Air,
    Grass,
    Dirt,
    Stone,
}

#[derive(Clone)]
pub struct Chunk {
    ids: Channel,
}

impl Default for Chunk {
    fn default() -> Self {
        let mut ids = Channel::default();
        ids.extend(iter::repeat(Block::Void).map(|block| block as u64).take(consts::CHUNK_SIZE));
        Self { ids }
    }
}

impl Chunk {
    fn set(&mut self, set: &[(ChunkPosition, Block)]) {
        self.ids.set(set.into_iter().copied().map(|(pos,b)| (ChunkPosition::linearize(pos) as usize, b as u64)));
    }
    fn get(&self, set: &[ChunkPosition]) -> Vec<Block> {
        //trust me ma, i know what I am doing *puts on motorcycle helmet*
        unsafe { mem::transmute(self.ids.get(set.into_iter().copied().map(|pos| ChunkPosition::linearize(pos) as usize))) }
    }
}

#[derive(Clone)]
pub struct Region {
    chunks: Vec<Chunk>
}

impl Region {
    fn set(&mut self, set: &[(RegionPosition, Block)]) {
        let mut map = HashMap::<usize, Vec<(ChunkPosition, Block)>>::new();
        for (pos, block) in set.into_iter().copied() {
            let index = (pos / CHUNK_AXIS as u64).linearize() as usize;
            map.entry(index).or_default().push((pos.to_chunk_pos(), block));
        }
        for (index, blocks_for_chunk) in map {
            self.chunks.get_mut(index).unwrap().set(&blocks_for_chunk)
        }
    }
    fn get(&self, set: &[RegionPosition]) -> Vec<Block> {
        let mut blocks = vec![];
        let mut map = HashMap::<usize, Vec<ChunkPosition>>::new();
        for pos in set.into_iter().copied() {
            let index = (pos / CHUNK_AXIS as u64).linearize() as usize;
            map.entry(index).or_default().push(pos.to_chunk_pos());
        }
        for (index, chunk_positions) in map {
            blocks.extend(self.chunks[index].get(&chunk_positions))
        }
        blocks
    }
}


impl Absorb<Operation> for Region {
    fn absorb_first(&mut self, operation: &mut Operation, _: &Self) {
        use Operation::*;
        match operation {
            SetBlocks(blocks) => self.set(blocks)
        }
    }

    fn sync_with(&mut self, first: &Self) {
        *self = first.clone();
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
            chunks: iter::repeat_with(|| Chunk::default()).take(REGION_SIZE).collect()
        }
    }
}

pub enum Operation {
    SetBlocks(Vec<(RegionPosition, Block)>)
}

pub struct Writer(WriteHandle<Region, Operation>);

impl Writer {
    fn set(&mut self, set: Vec<(RegionPosition, Block)>) {
        let handle = &mut self.0;

        handle.append(Operation::SetBlocks(set));
    }
}

pub struct Reader(ReadHandle<Region>);

impl Reader {
    fn get(&self, set: &[RegionPosition]) {
        let handle = &self.0;

        handle.enter().expect("could not enter").get(set);
    }
}

#[derive(Default)]
pub struct Higgs {
    writers: DashMap<RegionId, Writer>,
    readers: DashMap<RegionId, Reader>,
}