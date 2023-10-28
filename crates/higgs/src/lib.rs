#![allow(dead_code)]

use client::ClientId;
use consts::{CHUNK_AXIS, REGION_SIZE};
use dashmap::DashMap;
use entity::{Entity, EntityId};
use left_right::{Absorb, ReadHandleFactory, WriteHandle};
use position::{ChunkPosition, RegionId, RegionPosition};
use serde::{Deserialize, Serialize};

use std::{
    collections::{HashMap, HashSet},
    iter, mem, ops,
    sync::Arc,
};
use tokio::sync::{
    broadcast::{Receiver, Sender},
    Mutex,
};

use voxels::Channel;

pub mod client;
pub mod entity;
pub mod net;
pub mod position;

pub mod consts {
    pub const REGION_AXIS: u64 = 8;

    pub const REGION_SIZE: u64 = REGION_AXIS.pow(3);

    pub const CHUNK_AXIS: u64 = 8;

    pub const CHUNK_SIZE: u64 = CHUNK_AXIS.pow(3);

    pub const WORLD_AXIS: u64 = 1_000_000;

    pub const WORLD_SIZE: u64 = WORLD_AXIS.pow(3);

    pub const CHUNKS_PER_REGION: u64 = REGION_SIZE / CHUNK_SIZE;
}

#[repr(u64)]
#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum Block {
    Void,
    Air,
    Grass,
    Dirt,
    Stone,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Chunk {
    ids: Channel,
}

impl Default for Chunk {
    fn default() -> Self {
        let mut ids = Channel::default();
        ids.extend(
            iter::repeat(Block::Void)
                .map(|block| block as u64)
                .take(consts::CHUNK_SIZE as usize),
        );
        Self { ids }
    }
}

impl Chunk {
    fn set_blocks<I: IntoIterator<Item = (ChunkPosition, Block)>>(&mut self, iter: I) {
        self.ids.set(
            iter.into_iter()
                .map(|(pos, b)| (ChunkPosition::linearize(pos), b as u64)),
        );
    }
    fn get_blocks<I: IntoIterator<Item = ChunkPosition>>(
        &self,
        iter: I,
    ) -> HashMap<ChunkPosition, Block> {
        let block_positions = iter.into_iter().collect::<Vec<_>>();

        //trust me ma, i know what I am doing *puts on motorcycle helmet*
        let blocks: Vec<Block> = unsafe {
            mem::transmute(
                self.ids.get(
                    block_positions
                        .iter()
                        .copied()
                        .map(ChunkPosition::linearize),
                ),
            )
        };

        block_positions
            .into_iter()
            .zip(blocks)
            .collect::<HashMap<_, _>>()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Region {
    chunks: Vec<Chunk>,
    entities: HashMap<EntityId, Entity>,
}

impl Region {
    fn set_blocks<I: IntoIterator<Item = (RegionPosition, Block)>>(&mut self, iter: I) {
        let mut map = HashMap::<usize, HashMap<ChunkPosition, Block>>::new();
        for (pos, block) in iter.into_iter() {
            map.entry(pos.to_chunk_id() as usize)
                .or_default()
                .insert(pos.to_chunk_pos(), block);
        }
        for (index, blocks_for_chunk) in map {
            self.chunks
                .get_mut(index)
                .unwrap()
                .set_blocks(blocks_for_chunk)
        }
    }
    fn get_blocks<I: IntoIterator<Item = RegionPosition>>(
        &self,
        iter: I,
    ) -> HashMap<RegionPosition, Block> {
        let mut blocks = HashMap::new();
        let mut map = HashMap::<u64, HashSet<ChunkPosition>>::new();
        for pos in iter.into_iter() {
            let index = (pos / CHUNK_AXIS).linearize();
            map.entry(index).or_default().insert(pos.to_chunk_pos());
        }
        for (index, chunk_positions) in map {
            blocks.extend(
                self.chunks[index as usize]
                    .get_blocks(chunk_positions)
                    .into_iter()
                    .map(|(p, b)| (p.to_region_pos(index), b)),
            );
        }
        blocks
    }
    fn insert_entities(&mut self, mapping: HashMap<EntityId, Entity>) {
        self.entities.extend(mapping);
    }
    fn remove_entities(&mut self, entities: HashSet<EntityId>) {
        self.entities.retain(|id, _| !entities.contains(id));
    }
}

#[derive(Clone)]
pub struct WorkUnit {
    region: Region,
    publisher: Arc<Sender<Procedure>>,
    subscriber: Arc<Receiver<Procedure>>,
}

impl Absorb<Procedure> for WorkUnit {
    fn absorb_first(&mut self, Procedure { operation, .. }: &mut Procedure, _: &Self) {
        use Operation::*;
        match operation {
            SetBlocks(blocks) => self.region.set_blocks(blocks.clone()),
            InsertEntity(mapping) => self.region.insert_entities(mapping.clone()),
            RemoveEntity(entities) => self.region.remove_entities(entities.clone()),
        }
    }

    fn sync_with(&mut self, first: &Self) {
        *self = first.clone();
    }

    fn absorb_second(&mut self, mut procedure: Procedure, other: &Self) {
        Self::absorb_first(self, &mut procedure, other);
        let _ = self.publisher.send(procedure);
    }

    fn drop_first(self: Box<Self>) {}

    fn drop_second(self: Box<Self>) {}
}

impl Default for Region {
    fn default() -> Self {
        Self {
            chunks: iter::repeat_with(Chunk::default)
                .take(REGION_SIZE as usize)
                .collect(),
            entities: Default::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Operation {
    SetBlocks(HashMap<RegionPosition, Block>),
    InsertEntity(HashMap<EntityId, Entity>),
    RemoveEntity(HashSet<EntityId>),
}

#[derive(Clone)]
pub struct Procedure {
    client: ClientId,
    operation: Operation,
}

pub struct Writer(Mutex<WriteHandle<WorkUnit, Procedure>>);

impl ops::Deref for Writer {
    type Target = Mutex<WriteHandle<WorkUnit, Procedure>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for Writer {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// impl Writer {
//     fn set_blocks(&mut self, set: Vec<(RegionPosition, Block)>) {
//         let handle = &mut self.0;

//         handle.append(Operation::SetBlocks(set));
//     }
// }

pub struct ReaderFactory(ReadHandleFactory<WorkUnit>);

impl ops::Deref for ReaderFactory {
    type Target = ReadHandleFactory<WorkUnit>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// impl Reader {
//     fn get_blocks(&self, set: &[RegionPosition]) -> Vec<Block> {
//         let handle = &self.0;

//         handle.enter().expect("could not enter").get_blocks(set)
//     }
// }

#[derive(Default)]
pub struct Workload {
    writers: DashMap<RegionId, Writer>,
    reader_factories: DashMap<RegionId, ReaderFactory>,
}

// impl Higgs {
// fn set_blocks(&mut self, set: Vec<(GlobalPosition, Block)>) {
//     let mut map = HashMap::<u64, Vec<(RegionPosition, Block)>>::new();
//     for (pos, block) in set {
//         let id = pos.to_region_id();
//         map.entry(id)
//             .or_default()
//             .push((pos.to_region_pos(), block));
//     }
//     for (id, blocks_for_region) in map {
//         self.workload.writers
//             .get_mut(&id)
//             .unwrap()
//             .set_blocks(blocks_for_region)
//     }
// }
// fn get_blocks(&self, set: Vec<GlobalPosition>) -> Vec<Block> {
//     let mut blocks = Vec::with_capacity(set.len());
//     let mut map = HashMap::<u64, Vec<RegionPosition>>::new();
//     for pos in set {
//         let id = pos.to_region_id();
//         map.entry(id).or_default().push(pos.to_region_pos());
//     }
//     for (id, blocks_for_region) in map {
//         blocks.extend(self.workload.readers
//             .get(&id)
//             .unwrap()
//             .get_blocks(&blocks_for_region));
//     }
//     blocks
// }
// }
