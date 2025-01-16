use std::{future::Future, sync::{atomic::{AtomicBool, Ordering}, Arc}};

use dashmap::DashMap;
use tokio::{io::AsyncWriteExt, net::tcp::OwnedWriteHalf, sync::{mpsc, Mutex}};

use crate::{endpoint::{AsyncSend, Channel}, server::protol::{maker::{make_io_chunk_payload, make_pure_packet, make_transcribe_result_payload}, TranscribeResult}};

use super::protol::{maker::{make_connect_ok_payload, make_packet}, ClientId, EndpointType, IOChunk, Packet, SerialNo};

pub enum WorkerChannelMessage {
    Attach(OwnedWriteHalf),
    RegisterOk(SerialNo),
    Status(SerialNo),
    Ack(SerialNo, bool),
    ConnOk(ClientId, OwnedWriteHalf),
    ClientData(IOChunk),
    ServerData(IOChunk),
    Detach,
}

pub struct WorkerChannel {
    serial_no: Arc<SerialNo>,
    available: Arc<AtomicBool>,
    sender: Option<mpsc::Sender<WorkerChannelMessage>>,
}

unsafe impl Send for WorkerChannel {}
unsafe impl Sync for WorkerChannel {}

impl WorkerChannel {
    pub fn new(serial_no: SerialNo) -> Self {
        Self {
            serial_no: Arc::new(serial_no),
            available: Arc::new(AtomicBool::new(true)),
            sender: None,
        }
    }

    pub fn get_serial_no(&self) -> Arc<SerialNo> {
        self.serial_no.clone()
    }

    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
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
                                    writer.write_all(&reg_ok_packet).await.unwrap();
                                    writer.flush().await.unwrap();
                                }
                            },
                            WorkerChannelMessage::ConnOk(client_id, writer) => {
                                if !tcp_stream_map.contains_key(&client_id) {
                                    tcp_stream_map.insert(client_id, Arc::new(Mutex::new(writer)));
                                    println!("conn_ok received, client_id: {:?}", client_id);

                                    let conn_ok_payload = make_connect_ok_payload(&serial_no, &client_id);
                                    let conn_ok_packet = make_packet(EndpointType::Client, Packet::ConnOk, &conn_ok_payload);
                                    println!("send conn_ok packet: {:?}", conn_ok_packet);
                                    
                                    {
                                        if let Some(writer) = tcp_stream_map.get(&client_id) {
                                            let mut writer = (*writer).lock().await;
                                            if let Err(e) = writer.write_all(&conn_ok_packet).await {
                                                println!("detach worker with serial_no: {:?} because of error: {}", serial_no, e);
                                            }
                                        }
                                    }
                                }
                            },
                            WorkerChannelMessage::Status(serial_no) => {
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
                                // 发送心跳包，更新worker状态
                                println!("status packet received, serial_no: {:?}", serial_no);
                                let alive_packet = make_packet(EndpointType::Handler, Packet::Status, &serial_no);
                                if let Some(ref mut writer) = owned_writer {
                                    println!("send alive packet: {:?}", alive_packet);
                                    if let Err(_) = writer.write_all(&alive_packet).await {
                                        println!("worker with serial_no: {:?} is detached", serial_no);
                                    }
                                }
                            },
                            WorkerChannelMessage::Ack(serial_no, available_) => {
                                println!("ack received, serial_no: {:?}, available: {}", serial_no, available_);
                                available.store(available_, Ordering::Relaxed);
                            },
                            WorkerChannelMessage::ClientData(io_chunk) => {
                                if let Some(ref mut writer) = &mut owned_writer {
                                    let io_chunk_payload = make_io_chunk_payload(&io_chunk);
                                    let io_chunk_packet = make_packet(EndpointType::Client, Packet::Data, &io_chunk_payload);
                                    writer.write_all(&io_chunk_packet).await.unwrap();
                                    writer.flush().await.unwrap();
                                }
                            },
                            WorkerChannelMessage::ServerData(io_chunk) => {
                                let client_id = io_chunk.get_client_id();
                                if let Some(writer) = tcp_stream_map.get(&client_id) {
                                    let mut writer = (*writer).lock().await;
                                    let length = io_chunk.get_length();
                                    let data = io_chunk.get_data();
                                    let transcribe_result = TranscribeResult::new(length, *data);
                                    let transcribe_result_payload = make_transcribe_result_payload(&transcribe_result);
                                    let transcribe_result_packet = make_packet(EndpointType::Client, Packet::Result, &transcribe_result_payload);
                                    writer.write_all(&transcribe_result_packet).await.unwrap();
                                    writer.flush().await.unwrap();
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