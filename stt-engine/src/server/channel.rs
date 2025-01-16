use std::{future::Future, sync::{atomic::{AtomicBool, Ordering}, Arc}};

use dashmap::DashMap;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf, sync::{mpsc, Mutex}};

use crate::{endpoint::{AsyncSend, Channel}, server::protol::{maker::{make_io_chunk_payload, make_pure_packet, make_transcribe_result_payload}, TranscribeResult}};

use super::protol::{maker::{make_connection_info_payload, make_packet}, ClientId, EndpointType, IOChunk, Packet, SerialNo};

pub enum WorkerChannelMessage {
    Attach(OwnedWriteHalf),
    RegisterOk(SerialNo),
    Status,
    Alive(SerialNo, bool),
    ConnOk(ClientId, OwnedWriteHalf),
    ClientData(IOChunk),
    ServerData(IOChunk),
    Eos(SerialNo, ClientId),
    Detach,
}

pub struct WorkerChannel {
    serial_no: Arc<SerialNo>,
    available: Arc<AtomicBool>,
    stream_closed: Arc<AtomicBool>,
    sender: Option<mpsc::Sender<WorkerChannelMessage>>,
}

unsafe impl Send for WorkerChannel {}
unsafe impl Sync for WorkerChannel {}

impl WorkerChannel {
    async fn send_worker_packet(writer: &mut OwnedWriteHalf, packet: Vec<u8>, stream_closed: Arc<AtomicBool>) -> () {
        match writer.write_all(&packet).await {
            Ok(_) => writer.flush().await.unwrap(),
            Err(_) => stream_closed.store(true, Ordering::Relaxed),
        }
    }

    async fn send_client_packet(tcp_stream_map: DashMap<ClientId, Arc<Mutex<OwnedWriteHalf>>>, packet: Vec<u8>, client_id: ClientId) -> () {
        if let Some(writer) = tcp_stream_map.get(&client_id) {
            let mut writer = (*writer).lock().await;
            let mut invalided_client_id: Option<ClientId> = None;
            match writer.write_all(&packet).await {
                Ok(_) => writer.flush().await.unwrap(),
                Err(e) => {
                    println!("send packet result with ClientId: {} failed, because of error: {}", client_id, e);
                    invalided_client_id = Some(client_id);
                    if let Err(e) = writer.shutdown().await {
                        println!("shutdown client stream with ClientId: {} failed, because of error: {}", client_id, e);
                    }
                },
            }
            if let Some(client_id) = invalided_client_id {
                tcp_stream_map.remove(&client_id);
            }
        }
    }
}

impl WorkerChannel {
    pub fn new(serial_no: SerialNo) -> Self {
        Self {
            serial_no: Arc::new(serial_no),
            available: Arc::new(AtomicBool::new(true)),
            stream_closed: Arc::new(AtomicBool::new(false)),
            sender: None,
        }
    }

    pub fn get_serial_no(&self) -> Arc<SerialNo> {
        self.serial_no.clone()
    }

    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }

    pub fn update_available(&self, available: bool) {
        self.available.store(available, Ordering::Relaxed);
    }

    pub fn is_stream_closed(&self) -> bool {
        self.stream_closed.load(Ordering::Relaxed)
    }
}

impl Channel for WorkerChannel {
    type Message = WorkerChannelMessage;

    type Writer = OwnedWriteHalf;

    fn open(&mut self, capacity: usize) -> impl Future<Output = ()> {
        async move {
            let (tx, mut rx) = mpsc::channel::<WorkerChannelMessage>(capacity);
            self.sender = Some(tx);

            {
                let serial_no = self.serial_no.clone();
                let available = self.available.clone();
                let stream_closed = self.stream_closed.clone();

                tokio::spawn(async move {
                    let mut owned_writer: Option<OwnedWriteHalf> = None;
                    let tcp_stream_map: DashMap<ClientId, Arc<Mutex<OwnedWriteHalf>>> = DashMap::new();

                    while let Some(msg) = rx.recv().await {
                        match msg {
                            WorkerChannelMessage::Attach(writer) => {
                                println!("attach received, writer: {:?}", writer);
                                owned_writer = Some(writer);
                            },
                            WorkerChannelMessage::RegisterOk(serial_no) => {
                                let reg_ok_packet = make_packet(EndpointType::Handler, Packet::RegOk, &serial_no);
                                if let Some(ref mut writer) = owned_writer {
                                    Self::send_worker_packet(writer, reg_ok_packet, stream_closed.clone()).await;
                                }
                            },
                            WorkerChannelMessage::ConnOk(client_id, writer) => {
                                if !tcp_stream_map.contains_key(&client_id) {
                                    tcp_stream_map.insert(client_id, Arc::new(Mutex::new(writer)));
                                    println!("conn_ok received, client_id: {:?}", client_id);

                                    let conn_ok_payload = make_connection_info_payload(&serial_no, &client_id);
                                    let conn_ok_packet = make_packet(EndpointType::Client, Packet::ConnOk, &conn_ok_payload);
                                    println!("send conn_ok packet: {:?}", conn_ok_packet);
                                    Self::send_client_packet(tcp_stream_map.clone(), conn_ok_packet, client_id).await;
                                }
                            },
                            WorkerChannelMessage::Status => {
                                let mut unused_client_ids: Vec<ClientId> = vec![];
                                // 找出无效连接
                                for tcp_stream in tcp_stream_map.iter() {
                                    let client_id = tcp_stream.key();
                                    let mut writer = tcp_stream.value().lock().await;
                                    let alive_packet = make_pure_packet(EndpointType::Client, Packet::Alive);
                                    if let Err(_) = writer.write_all(&alive_packet).await {
                                        println!("detach client with id: {:?} because of error", client_id);
                                        unused_client_ids.push(*client_id);
                                    } else {
                                        writer.flush().await.unwrap();
                                    }
                                }
                                // 清理无效连接
                                for client_id in unused_client_ids {
                                    tcp_stream_map.remove(&client_id);
                                }

                                // 确定Worker stream是否已关闭
                                let ack_packet = make_packet(EndpointType::Handler, Packet::Ack, serial_no.as_ref());
                                if let Some(ref mut writer) = owned_writer {
                                    Self::send_worker_packet(writer, ack_packet, stream_closed.clone()).await;
                                }
                            },
                            WorkerChannelMessage::Alive(serial_no, available_) => {
                                println!("alive received, serial_no: {:?}, available: {}", serial_no, available_);
                                available.store(available_, Ordering::Relaxed);
                                let ack_packet = make_packet(EndpointType::Handler, Packet::Ack, &serial_no);
                                if let Some(ref mut writer) = owned_writer {
                                    Self::send_worker_packet(writer, ack_packet, stream_closed.clone()).await;
                                }
                            },
                            WorkerChannelMessage::ClientData(io_chunk) => {
                                let io_chunk_payload = make_io_chunk_payload(&io_chunk);
                                let io_chunk_packet = make_packet(EndpointType::Client, Packet::Data, &io_chunk_payload);
                                if let Some(ref mut writer) = &mut owned_writer {
                                    Self::send_worker_packet(writer, io_chunk_packet, stream_closed.clone()).await;
                                }
                            },
                            WorkerChannelMessage::ServerData(io_chunk) => {
                                let client_id = io_chunk.get_client_id();
                                let length = io_chunk.get_length();
                                let data = io_chunk.get_data();
                                let transcribe_result = TranscribeResult::new(length, *data);
                                let transcribe_result_payload = make_transcribe_result_payload(&transcribe_result);
                                let transcribe_result_packet = make_packet(EndpointType::Client, Packet::Result, &transcribe_result_payload);
                                Self::send_client_packet(tcp_stream_map.clone(), transcribe_result_packet, client_id).await;
                            },
                            WorkerChannelMessage::Eos(serial_no, client_id) => {
                                println!("eos received, serial_no: {:?}, client_id: {:?}", serial_no, client_id);
                                let eos_payload = make_connection_info_payload(&serial_no, &client_id);
                                let eos_packet = make_packet(EndpointType::Handler, Packet::Eos, &eos_payload);
                                if let Some(ref mut writer) = owned_writer {
                                    Self::send_worker_packet(writer, eos_packet, stream_closed.clone()).await;
                                }
                            },
                            WorkerChannelMessage::Detach => {
                                if let Some(mut writer) = owned_writer {
                                    writer.shutdown().await.unwrap();
                                }
                                break;
                            },
                        }
                    }
                });
            }
        }
    }

    fn close(&mut self) -> impl Future<Output = ()> {
        async move {
            self.send(WorkerChannelMessage::Detach).await;
        }
    }
}

impl AsyncSend<WorkerChannelMessage> for WorkerChannel {
    fn send(&self, message: WorkerChannelMessage) -> impl Future<Output = ()> {
        async move {
            if let Some(sender) = &self.sender {
                sender.send(message).await.unwrap();
            }
        }
    }
}