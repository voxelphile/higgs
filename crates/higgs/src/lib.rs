#![allow(dead_code)]

use client::ClientId;
use dashmap::DashMap;
use futures::Future;
use game_common::consts::*;
use game_common::position::{ChunkPosition, RegionId, RegionPosition};
use google_cloud_storage::http::objects::{
    download::Range,
    get::GetObjectRequest,
    upload::{Media, UploadObjectRequest, UploadType},
};
use higgs_common::entity::{Entity, EntityId};
use higgs_common::region::*;
use left_right::{Absorb, ReadHandleFactory, WriteHandle};
use serde::{Deserialize, Serialize};

use std::{
    collections::{HashMap, HashSet},
    iter, mem, ops,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{
    broadcast::{self, Receiver, Sender},
    oneshot, Mutex,
};

pub mod client;

#[derive(Clone)]
pub struct WorkUnit {
    region_id: RegionId,
    region: Region,
    publisher: Arc<Sender<Procedure>>,
    subscriber: Arc<Receiver<Procedure>>,
    work_load: Arc<WorkLoad>,
    kill_switch: Arc<oneshot::Sender<()>>,
}

impl WorkUnit {
    fn save(region_id: RegionId, work_load: Arc<WorkLoad>, region: Region) {
        tokio::spawn(async move {
            let data = bincode::serialize(&region).unwrap();

            let object_name = format!("regions/{}", region_id);

            work_load
                .cloud_storage_client
                .upload_object(
                    &UploadObjectRequest {
                        bucket: "xenotech".to_owned(),
                        ..Default::default()
                    },
                    data,
                    &UploadType::Simple(Media::new(object_name)),
                )
                .await
                .expect("failed to upload region");
        });
    }

    async fn save_on_timer(
        region_id: RegionId,
        work_load: Arc<WorkLoad>,
        mut kill_switch: oneshot::Receiver<()>,
    ) -> impl Future<Output = ()> {
        async move {
            loop {
                if kill_switch.try_recv().is_ok() {
                    return;
                }
                tokio::time::sleep(Duration::from_secs(60 * 5)).await;
                let read_handle = work_load.reader_factories.get(&region_id).unwrap().handle();
                let read_guard = read_handle.enter().unwrap();
                let region = read_guard.region.clone();
                drop(read_guard);
                Self::save(region_id, work_load.clone(), region);
            }
        }
    }
    async fn init(region_id: RegionId, work_load: Arc<WorkLoad>) {
        let (save_killswitch_tx, save_killswitch_rx) = oneshot::channel();

        let (tx, rx) = broadcast::channel(2usize.pow(16));
        let (w, r) = left_right::new_from_empty(WorkUnit {
            region_id,
            region: Default::default(),
            publisher: Arc::new(tx),
            subscriber: Arc::new(rx),
            work_load: work_load.clone(),
            kill_switch: Arc::new(save_killswitch_tx),
        });
        let (w, r) = (Writer(Mutex::new(w)), ReaderFactory(r.factory()));

        tokio::spawn(WorkUnit::save_on_timer(
            region_id,
            work_load.clone(),
            save_killswitch_rx,
        ));

        if let Ok(data) = work_load
            .cloud_storage_client
            .download_object(
                &GetObjectRequest {
                    bucket: "xenotech".to_string(),
                    object: format!("regions/{region_id}"),
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
        {
            unsafe {
                w.lock().await.raw_write_handle().as_mut().region =
                    bincode::deserialize(&data).unwrap()
            };
        }

        work_load.writers.insert(region_id, w);
        work_load.reader_factories.insert(region_id, r);
    }
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

    fn drop_second(self: Box<Self>) {
        WorkUnit::save(self.region_id, self.work_load.clone(), self.region.clone());
    }
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
pub struct WorkLoad {
    writers: DashMap<RegionId, Writer>,
    reader_factories: DashMap<RegionId, ReaderFactory>,
    cloud_storage_client: google_cloud_storage::client::Client,
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
