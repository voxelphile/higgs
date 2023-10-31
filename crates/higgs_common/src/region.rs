use crate::entity::*;
use game_common::{consts::*, position::*};
use serde::*;
use std::collections::*;
use std::iter;
use std::mem;
use voxels::Channel;
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
                .take(CHUNK_SIZE as usize),
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
    pub fn set_blocks<I: IntoIterator<Item = (RegionPosition, Block)>>(&mut self, iter: I) {
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
    pub fn insert_entities(&mut self, mapping: HashMap<EntityId, Entity>) {
        self.entities.extend(mapping);
    }
    pub fn remove_entities(&mut self, entities: HashSet<EntityId>) {
        self.entities.retain(|id, _| !entities.contains(id));
    }
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
