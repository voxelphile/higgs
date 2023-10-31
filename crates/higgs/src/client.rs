use crate::{Procedure, WorkLoad, WorkUnit};
use dashmap::DashMap;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use game_common::position::RegionId;
use higgs_common::net::{Request, Response};
use higgs_common::region::*;
use left_right::ReadHandle;
use quinn::{Connection, ReadError, WriteError};
use std::{collections::HashMap, iter, sync::Arc, time::Duration};
use tokio::sync::{
    broadcast::{self, Receiver},
    mpsc::unbounded_channel,
};

pub type ClientId = uuid::Uuid;

pub struct Client {
    connection: Connection,
    workload: Arc<WorkLoad>,
    subscriptions: Arc<DashMap<RegionId, Receiver<Procedure>>>,
}

impl Client {
    pub fn new(connection: Connection, workload: Arc<WorkLoad>) -> Self {
        Self {
            connection,
            workload,
            subscriptions: Default::default(),
        }
    }

    pub async fn start(self) {
        let client = ClientId::new_v4();
        let Self {
            connection,
            workload,
            subscriptions,
        } = self;
        let Ok((mut send_stream, mut recv_stream)) = connection.accept_bi().await else {
            return;
        };
        let (tx, mut rx) = unbounded_channel::<Response>();
        let (kill_switch_tx, kill_switch_rx) = broadcast::channel::<()>(1);

        //We want a task that receives information from the client
        {
            let tx = tx.clone();
            let subscriptions = subscriptions.clone();
            let workload = workload.clone();
            let mut kill_switch_rx = kill_switch_rx.resubscribe();
            let kill_switch_tx = kill_switch_tx.clone();
            tokio::spawn(async move {
                let mut buffer = vec![0u8; 65535];
                let mut readers = HashMap::<RegionId, ReadHandle<WorkUnit>>::new();
                loop {
                    if let Ok(()) = kill_switch_rx.try_recv() {
                        return;
                    }

                    let len = match recv_stream.read(&mut buffer).await {
                        Ok(Some(len)) => len,
                        Ok(_) => continue,
                        Err(ReadError::ConnectionLost(_)) => {
                            let _ = kill_switch_tx.send(());
                            return;
                        }
                        _ => return,
                    };

                    let Ok(request) = bincode::deserialize::<Request>(&buffer[..len]) else {
                        continue;
                    };

                    match request {
                        Request::Subscribe(region_ids) => {
                            let mut refresh = HashMap::new();

                            for region_id in region_ids {
                                let reader_factory = match workload.reader_factories.get(&region_id)
                                {
                                    Some(reader_factory) => reader_factory,
                                    None => {
                                        WorkUnit::init(region_id, workload.clone());
                                        workload.reader_factories.get(&region_id).unwrap()
                                    }
                                };

                                readers.insert(region_id, reader_factory.handle());
                                let read_guard = readers[&region_id].enter().unwrap();

                                subscriptions
                                    .insert(region_id, read_guard.subscriber.resubscribe());

                                refresh.insert(region_id, read_guard.region.clone());
                            }

                            tx.send(Response::Refresh(refresh)).unwrap();
                        }
                        Request::Perform(region_operations) => {
                            let mut futures = FuturesUnordered::new();

                            for (region_id, operations) in region_operations {
                                let workload = workload.clone();
                                futures.push(tokio::spawn(async move {
                                    let writer = workload.writers.get_mut(&region_id).unwrap();

                                    let mut write_guard = writer.lock().await;

                                    for operation in operations {
                                        write_guard.append(Procedure { client, operation });
                                    }

                                    write_guard.publish();
                                }));
                            }

                            while futures.next().await.is_some() {}
                        }
                    }
                }
            });
        }

        //..and a task that publishes information to the client
        {
            let tx = tx.clone();
            let mut kill_switch_rx = kill_switch_rx.resubscribe();
            tokio::spawn(async move {
                loop {
                    if let Ok(()) = kill_switch_rx.try_recv() {
                        return;
                    }

                    let mut operations = HashMap::<RegionId, Vec<Operation>>::new();

                    for mut subscriber in subscriptions.iter_mut() {
                        let region_id = *subscriber.key();
                        let mut iter = iter::repeat_with(|| subscriber.try_recv()).map(Result::ok);

                        operations.entry(region_id).or_default().extend(
                            iter.next()
                                .flatten()
                                .filter(|p| p.client != client)
                                .map(|p| p.operation),
                        );
                    }

                    tx.send(Response::Publish(operations)).unwrap();

                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            });
        }

        //..and finally a task that sends information to the client.
        {
            let mut kill_switch_rx = kill_switch_rx.resubscribe();
            let kill_switch_tx = kill_switch_tx.clone();

            tokio::spawn(async move {
                loop {
                    if let Ok(()) = kill_switch_rx.try_recv() {
                        return;
                    }

                    let mut iter = iter::repeat_with(|| rx.try_recv()).map(Result::ok);

                    while let Some(response) = iter.next().flatten() {
                        match send_stream
                            .write(&bincode::serialize(&response).unwrap())
                            .await
                        {
                            Ok(_) => continue,
                            Err(WriteError::ConnectionLost(_)) => {
                                let _ = kill_switch_tx.send(());
                                return;
                            }
                            _ => return,
                        }
                    }
                }
            });
        }
    }
}
