use std::{future::Future, io::ErrorKind, sync::{Arc, Mutex}, time::Duration};

use derive_new::new;
use tokio::{io::AsyncWriteExt, net::TcpListener, signal::ctrl_c, sync::RwLock};

use crate::{endpoint::{AsyncExecute, AsyncSend, Channel, Endpoint}, server::{channel::WorkerChannelMessage, protol::{maker::{make_pure_packet, make_serial_no}, parser::{is_magic_number, parse_alive_payload, parse_connection_info_payload, parse_endpoint_type, parse_io_chunk_payload, parse_packet_type, parse_register_payload}, EndpointType, Packet, RwMode}}};

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
                                let lock = Mutex::new(0);

                                match is_magic_number(&mut reader).await {
                                    Ok(_) => {
                                        match parse_endpoint_type(&mut reader).await {
                                            EndpointType::Handler => {
                                                let packet_type = parse_packet_type(&mut reader).await;
                                                if let Packet::Register = Packet::from(packet_type) {
                                                    if let Some(register_payload) = parse_register_payload(&mut reader).await {
                                                        let ip = register_payload.get_ip();
                                                        let port = register_payload.get_port();
                                                        println!("Received register packet from {}, ip={:?}, port={}", addr, ip, port);
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
                                                    println!("Received connect packet from {}>>", addr);
                                                    let workers = workers.read().await;
                                                    if lock.lock().is_ok() {
                                                        if let Some(worker) = workers.iter().find(|w| w.is_available()) {
                                                            let serial_no = worker.get_serial_no();
                                                            println!("Received connect packet from {}, attaching to worker {:?}", addr, serial_no);
                                                            worker.update_available(false);
                                                            println!("Updated worker {:?} available status to false", serial_no);
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
                                                        println!("Received connect packet from {}<<", addr);
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
                                                        Packet::Alive => {
                                                            if let Some(ack_payload) = parse_alive_payload(&mut reader).await {
                                                                let serial_no = ack_payload.get_serial_no();
                                                                let available = ack_payload.is_available();
                                                                println!("Received ack packet from {}, serial_no={:?}, available={}", addr, serial_no, available);
                                                                let workers = workers.read().await;
                                                                if let Some(worker) = workers.iter().find(|w| w.get_serial_no() == serial_no.into()) {
                                                                    worker.send(WorkerChannelMessage::Alive(serial_no, available)).await;
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
                                                        Packet::Eos => {
                                                            if let Some(eos_payload) = parse_connection_info_payload(&mut reader).await {
                                                                let serial_no = eos_payload.0;
                                                                let client_id = eos_payload.1;
                                                                println!("Received EOS packet from {}, serial_no={:?}", addr, serial_no);
                                                                let workers = workers.read().await;
                                                                if let Some(worker) = workers.iter().find(|w| w.get_serial_no() == serial_no.into()) {
                                                                    worker.send(WorkerChannelMessage::Eos(serial_no, client_id)).await;
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
                    // 向每个worker每隔10s发送一个Status包清理已经失效的客户端
                    let mut interval = tokio::time::interval(Duration::from_secs(10));
                    loop {
                        interval.tick().await;
                        {
                            let workers = workers.read().await;
                            for worker in workers.iter() {
                                worker.send(WorkerChannelMessage::Status).await;
                            }
                        }

                        {
                            let mut workers = workers.write().await;
                            workers.retain(|w| !w.is_stream_closed());
                            println!("alive worker size: {}", workers.len());
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