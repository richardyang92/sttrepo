use std::{fs::File, future::Future, io::Read, sync::Arc, time::Duration};

use derive_new::new;
use tokio::{io::AsyncWriteExt, signal::ctrl_c, sync::Mutex};

use crate::{endpoint::{AsyncExecute, Endpoint}, server::protol::{maker::{make_io_chunk_payload, make_packet, make_pure_packet}, parser::{is_magic_number_limited, parse_connect_ok_payload, parse_endpoint_type, parse_packet_type, parse_transcribe_result_payload}, EndpointType, IOChunk, Packet, RwMode, IO_CHUNK_SIZE}};

#[derive(Debug, new)]
pub struct TcpClientConfig {
    ip: String,
    port: u16,
}

#[derive(Debug, PartialEq)]
pub enum TcpClientState {
    Init,
    Connected,
    Rejected,
    Eos,
}

pub struct TcpClientEndpoint {
    ip: String,
    port: u16,
}

impl Endpoint for TcpClientEndpoint {
    type Config = TcpClientConfig;

    type Output = Self;

    fn init(config: Self::Config) -> impl Future<Output = Option<Self::Output>> {
        async move {
            Some(TcpClientEndpoint {
                ip: config.ip,
                port: config.port,
            })
        }
    }
}

impl AsyncExecute for TcpClientEndpoint {
    type Output = ();

    fn execute_async(&self) -> impl Future<Output = Self::Output> {
        async move {
            tokio::select! {
                _ = async move {
                    let addr = format!("{}:{}", self.ip, self.port);
                    
                    if let Ok(socket) = tokio::net::TcpStream::connect(addr.clone()).await {
                        println!("Connected to {}", addr);
                        let (mut reader, mut writer) = socket.into_split();
                        let status = Arc::new(Mutex::new(TcpClientState::Init));
                        let serial_no = Arc::new(Mutex::new([0u8; 6]));
                        let client_id = Arc::new(Mutex::new(0u32));
                        
                        let status1 = status.clone();
                        let serial_no1 = serial_no.clone();
                        let client_id1 = client_id.clone();

                        println!("Sending connect packet...");
                        let packet = make_pure_packet(EndpointType::Client, Packet::Connect);
                        writer.write_all(&packet).await.unwrap();
                        writer.flush().await.unwrap();

                        // 发送任务
                        let send_joint = tokio::spawn(async move {
                            let mut interval = tokio::time::interval(Duration::from_secs(2));
                            loop {
                                interval.tick().await;
                                let status = status1.lock().await;
                                if *status != TcpClientState::Init {
                                    break;
                                }
                            }
                            println!("Sending data...");
                            let status = status1.lock().await;
                            if let TcpClientState::Connected = *status {
                                let wav_file = "./data/segment/split_part_1.wav";
                                if let Ok(mut file) = File::open(wav_file) {
                                    let mut data = Vec::new();
                                    file.read_to_end(&mut data).unwrap();
                                    for chunk in data.chunks(IO_CHUNK_SIZE) {
                                        let mut buffer = [0; IO_CHUNK_SIZE];
                                        buffer[..chunk.len()].copy_from_slice(chunk);
                                        let serial_no = *serial_no1.lock().await;
                                        let client_id = *client_id1.lock().await;
                                        let io_chunk = IOChunk::new(RwMode::Client, serial_no, client_id, chunk.len() as u16, buffer);
                                        let io_chunk_payload = make_io_chunk_payload(&io_chunk);
                                        let io_chunk_packet = make_packet(EndpointType::Client, Packet::Data, io_chunk_payload.as_slice());
                                        writer.write_all(&io_chunk_packet).await.unwrap();
                                        writer.flush().await.unwrap();
                                    }
                                }
                            }
                        });
                        
                        // 接收任务
                        let recv_joint = tokio::spawn(async move {
                            loop {
                                if is_magic_number_limited(&mut reader, 100).await.is_ok() {
                                    if let EndpointType::Client = parse_endpoint_type(&mut reader).await {
                                        let packet_type = parse_packet_type(&mut reader).await;
                                        match Packet::from(packet_type) {
                                            Packet::ConnOk => {
                                                if let Some((serial_no_, client_id_)) = parse_connect_ok_payload(&mut reader).await {
                                                    println!("Connected ok: serial_no: {:?}, client_id: {}", serial_no_, client_id_);
                                                    serial_no.lock().await.copy_from_slice(&serial_no_);
                                                    *client_id.lock().await = client_id_;
                                                    let mut status = status.lock().await;
                                                    *status = TcpClientState::Connected;
                                                } else {
                                                    println!("Connected rejected");
                                                    let mut status = status.lock().await;
                                                    *status = TcpClientState::Rejected;
                                                }
                                            },
                                            Packet::ConnRejected => {
                                                println!("Connected rejected");
                                                let mut status = status.lock().await;
                                                *status = TcpClientState::Rejected;
                                            },
                                            Packet::Result => {
                                                if let Some(result) = parse_transcribe_result_payload(&mut reader).await {
                                                    let length = result.get_length();
                                                    let data = result.get_data();
                                                    let result = String::from_utf8_lossy(&data[..length as usize]).to_string();
                                                    println!("Result: {}", result);
                                                }
                                            },
                                            _ => {},
                                        }
                                    }
                                } else {
                                    break;
                                }
                            }
                        });

                        send_joint.await.unwrap();
                        recv_joint.await.unwrap();
                    }
                } => {},
                _ = ctrl_c() => {},
            }
        }
    }
}