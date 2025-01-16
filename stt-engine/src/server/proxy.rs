use std::{future::Future, io::ErrorKind, sync::Arc, time::Duration};

use derive_new::new;
use tokio::{io::AsyncWriteExt, net::TcpListener, signal::ctrl_c, sync::RwLock};

use crate::{endpoint::{AsyncExecute, AsyncSend, Channel, Endpoint}, server::{channel::WorkerChannelMessage, protol::{maker::{make_pure_packet, make_serial_no}, parser::{is_magic_number, parse_ack_payload, parse_endpoint_type, parse_io_chunk_payload, parse_packet_type, parse_register_payload}, EndpointType, Packet, RwMode}}};

use super::channel::WorkerChannel;

#[derive(Debug, new)]
pub struct TcpListenerConfig {
    ip: String,
    port: u16,
}

pub struct TcpListenerEndpoint {
    listener: TcpListener,
    workers: Arc<RwLock<Vec<WorkerChannel>>>,
}

impl AsyncExecute for TcpListenerEndpoint {
    type Output = ();

    fn execute_async(&self) -> impl Future<Output = Self::Output> {
        async move {
            tokio::select! {
                _ = async move {
                    loop {
                        if let Ok(stream) = self.listener.accept().await {
                            let workers = self.workers.clone();
                            let (socket, addr) = stream;
                            tokio::spawn(async move {
                                println!("Received connection from {}", addr);
                                let (mut reader, mut writer) = socket.into_split();
                                match is_magic_number(&mut reader).await {
                                    Ok(_) => {
                                        match parse_endpoint_type(&mut reader).await {
                                            EndpointType::Handler => {
                                                let packet_type = parse_packet_type(&mut reader).await;
                                                if let Packet::Register = Packet::from(packet_type) {
                                                    println!("Received register packet from {}", addr);
                                                    if let Some(register_payload) = parse_register_payload(&mut reader).await {
                                                        let ip = register_payload.get_ip();
                                                        let port = register_payload.get_port();
                                                        let serial_no = make_serial_no(ip, port);
    
                                                        let mut channel = WorkerChannel::new(serial_no);
                                                        channel.open(20).await;
                                                        channel.send(WorkerChannelMessage::Attach(writer)).await;
                                                        channel.send(WorkerChannelMessage::RegisterOk(serial_no)).await;
                                                        workers.write().await.push(channel);
                                                    }
                                                }
                                            },
                                            EndpointType::Client => {
                                                if let Packet::Connect = Packet::from(parse_packet_type(&mut reader).await) {
                                                    println!("Received connect packet from {}", addr);
                                                    let workers = workers.read().await;
                                                    if let Some(worker) = workers.iter().find(|w| w.is_available()) {
                                                        let serial_no = worker.get_serial_no();
                                                        println!("Received connect packet from {}, attaching to worker {:?}", addr, serial_no);
                                                        // 生成一个随机的u32整形数作为client_id
                                                        let client_id = rand::random::<u32>();
                                                        worker.send(WorkerChannelMessage::ConnOk(client_id, writer)).await;
                                                    } else {
                                                        println!("Received connect packet from {}, but no available worker", addr);
                                                        // 如果workers没有空闲的worker，则拒绝连接
                                                        let conn_reject_packet = make_pure_packet(EndpointType::Client, Packet::ConnRejected);
                                                        writer.write_all(&conn_reject_packet).await.unwrap();
                                                        writer.flush().await.unwrap();
                                                    }
                                                }
                                            },
                                            _ => return,
                                        }
                                    },
                                    Err(e) => {
                                        match e.kind() {
                                            ErrorKind::UnexpectedEof => {
                                                println!("Connection {} closed before main loop", addr);
                                                return;
                                            },
                                            _ => {},
                                        }
                                    },
                                }

                                loop {
                                    match is_magic_number(&mut reader).await {
                                        Ok(_) => {
                                            match parse_endpoint_type(&mut reader).await {
                                                EndpointType::Handler => {
                                                    let packet_type = parse_packet_type(&mut reader).await;
                                                    match Packet::from(packet_type) {
                                                        Packet::Ack => {
                                                            if let Some(ack_payload) = parse_ack_payload(&mut reader).await {
                                                                let serial_no = ack_payload.get_serial_no();
                                                                let available = ack_payload.is_available();
                                                                println!("Received ack packet from {}, serial_no={:?}, available={}", addr, serial_no, available);
                                                                let workers = workers.read().await;
                                                                if let Some(worker) = workers.iter().find(|w| w.get_serial_no() == serial_no.into()) {
                                                                    worker.send(WorkerChannelMessage::Ack(serial_no, available)).await;
                                                                }
                                                            }
                                                        },
                                                        Packet::Data => {
                                                            if let Some(io_chunk_payload) = parse_io_chunk_payload(&mut reader).await {
                                                                let mode = io_chunk_payload.get_mode();
                                                                let serial_no = io_chunk_payload.get_serial_no();
                                                                if let RwMode::Server = mode {
                                                                    let workers = workers.read().await;
                                                                    if let Some(worker) = workers.iter().find(|w| serial_no == *w.get_serial_no()) {
                                                                        worker.send(WorkerChannelMessage::ServerData(io_chunk_payload)).await;
                                                                    }
                                                                }
                                                            }
                                                        },
                                                        _ => {}
                                                    }
                                                },
                                                EndpointType::Client => {
                                                    // 处理客户端语音数据
                                                    let packet_type = parse_packet_type(&mut reader).await;
                                                    match Packet::from(packet_type) {
                                                        Packet::Data => {
                                                            if let Some(io_chunk_payload) = parse_io_chunk_payload(&mut reader).await {
                                                                let mode = io_chunk_payload.get_mode();
                                                                let serial_no = io_chunk_payload.get_serial_no();
                                                                if let RwMode::Client = mode {
                                                                    let workers = workers.read().await;
                                                                    if let Some(worker) = workers.iter().find(|w| serial_no == *w.get_serial_no()) {
                                                                        worker.send(WorkerChannelMessage::ClientData(io_chunk_payload)).await;
                                                                    }
                                                                }
                                                            }
                                                        },
                                                        _ => {}
                                                    }
                                                },
                                                EndpointType::Unknown => {},
                                            }
                                        },
                                        Err(e) => {
                                            match e.kind() {
                                                ErrorKind::UnexpectedEof => {
                                                    println!("Connection {} closed.", addr);
                                                    break;
                                                },
                                                ErrorKind::InvalidData => {
                                                    println!("Invalid data received from {}, {}", addr, e);
                                                },
                                                _ => {}
                                            }
                                        },
                                    }
                                }
                            });
                        }
                    }
                } => {},
                _ = async move {
                    let workers = self.workers.clone();
                    // 向每个worker每隔10s发送一个Status包更新worker状态，同时清理已经失效的客户端
                    let mut interval = tokio::time::interval(Duration::from_secs(1));
                    loop {
                        interval.tick().await;
                        {
                            let workers = workers.read().await;

                            println!("sending status packet to workers...");
                            for worker in workers.iter() {
                                let serial_no = worker.get_serial_no();
                                worker.send(WorkerChannelMessage::Status(*serial_no)).await;
                            }
                        }
                    }
                } => {},
                _ = ctrl_c() => {},
            }
        }
        }
}

impl Endpoint for TcpListenerEndpoint {
    type Config = TcpListenerConfig;
    type Output = Self;

    fn init(config: Self::Config) -> impl Future<Output = Option<Self::Output>> {
        async move {
            match TcpListener::bind(format!("{}:{}", config.ip, config.port)).await {
                Ok(listener) => Some(TcpListenerEndpoint {
                    listener,
                    workers: Arc::new(RwLock::new(Vec::new())),
                }),
                Err(_) => None,
            }
        }
    }
}