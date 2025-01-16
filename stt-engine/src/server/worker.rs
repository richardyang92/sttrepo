use std::{future::Future, io::ErrorKind, sync::{atomic::AtomicBool, Arc}};

use derive_new::new;
use tokio::{io::AsyncWriteExt, net::TcpStream, signal::ctrl_c};

use crate::{endpoint::{AsyncExecute, Endpoint}, server::protol::{maker::{make_ack_payload, make_io_chunk_payload, make_packet, make_register_payload}, parser::{is_magic_number_limited, parse_endpoint_type, parse_io_chunk_payload, parse_packet_type, parse_reg_ok_payload}, Ack, EndpointType, IOChunk, Packet, Register, RwMode, IO_CHUNK_SIZE}, sherpa::Sherpa};

use super::protol::SerialNo;

const SHERPA_TOKENS: &str = "../sherpa/sherpa-models/tokens.txt";
const SHERPA_ENCODER: &str = "../sherpa/sherpa-models/encoder-epoch-20-avg-1-chunk-16-left-128.onnx";
const SHERPA_DECODER: &str = "../sherpa/sherpa-models/decoder-epoch-20-avg-1-chunk-16-left-128.onnx";
const SHERPA_JOINER: &str = "../sherpa/sherpa-models/joiner-epoch-20-avg-1-chunk-16-left-128.onnx";

#[derive(Debug, new)]
pub struct TcpWorkerConfig {
    ip: String,
    port: u16,
}

pub struct TcpWorkerEndpoint {
    ip: String,
    port: u16,
    sherpa: Arc<Sherpa>,
}

impl TcpWorkerEndpoint {
    fn exact_ip(&self) -> [u8; 4] {
        let ip_str = self.ip.to_string();
        ip_str.split('.')
            .map(|x| x.parse::<u8>().unwrap())
            .collect::<Vec<u8>>()
            .try_into()
            .unwrap()
    }
}

impl AsyncExecute for TcpWorkerEndpoint {
    type Output = ();

    fn execute_async(&self) -> impl Future<Output = ()> {
        async move {
            tokio::select! {
                _ = async move {
                    let addr = format!("{}:{}", self.ip, self.port);
                    if let Ok(socket) = TcpStream::connect(addr.clone()).await {
                        println!("Connected to {} success", addr);
                        let (mut reader, mut writer) = socket.into_split();
                        let ip = self.exact_ip();
                        let port = self.port;
                        let register_payload = make_register_payload(&Register::new(ip, port));
                        let register_packet = make_packet(EndpointType::Handler, Packet::Register, &register_payload);
                        writer.write_all(&register_packet).await.unwrap();
                        writer.flush().await.unwrap();

                        let mut serial_no = SerialNo::default();
                        let avaibale = AtomicBool::new(true);
                        let sherpa = self.sherpa.clone();

                        loop {
                            match is_magic_number_limited(&mut reader, 5000).await {
                                Ok(_) => {
                                    match parse_endpoint_type(&mut reader).await {
                                        EndpointType::Handler => {
                                            let packet_type = parse_packet_type(&mut reader).await;
                                            match Packet::from(packet_type) {
                                                Packet::RegOk => {
                                                    if let Some(serial_no_) = parse_reg_ok_payload(&mut reader).await {
                                                        println!("Get serial no: {:?}", &serial_no_);
                                                        serial_no.copy_from_slice(&serial_no_);
                                                    }
                                                },
                                                Packet::Status => {
                                                    let ack = Ack::new(serial_no, avaibale.load(std::sync::atomic::Ordering::Relaxed));
                                                    let ack_payload = make_ack_payload(&ack);
                                                    let ack_packet = make_packet(EndpointType::Handler, Packet::Ack, ack_payload.as_slice());
                                                    writer.write_all(&ack_packet).await.unwrap();
                                                }
                                                _ => {}
                                            }
                                        },
                                        EndpointType::Client => {
                                            let packet_type = parse_packet_type(&mut reader).await;
                                            match Packet::from(packet_type) {
                                                Packet::Data => {
                                                    avaibale.store(false, std::sync::atomic::Ordering::Relaxed);
                                                    if let Some(io_chunk_payload) = parse_io_chunk_payload(&mut reader).await {
                                                        let len = io_chunk_payload.get_length() as usize;
                                                        let client_id = io_chunk_payload.get_client_id();
                                                        let serial_no = io_chunk_payload.get_serial_no();
                                                        if len % 2 != 0 {
                                                            println!("Transfer buffer length is odd");
                                                        } else {
                                                            let data = io_chunk_payload.get_data()[..len].chunks(2).map(|chunk| {
                                                                ((chunk[1] as i16) << 8 | (chunk[0] as i16) & 0xff) as f32 / 32767f32
                                                            }).collect::<Vec<f32>>();
                                                            match sherpa.transcribe(data.as_slice()) {
                                                                Ok(result) => {
                                                                    if result != "" {
                                                                        // 去除result中的空格
                                                                        let result = result.replace(" ", "");
                                                                        println!("Transcribed: {}", result);
                                                                        // length等于result转化为bytes的长度
                                                                        let buffer: Vec<u8> = result.bytes().collect();
                                                                        let length = buffer.len();
                                                                        let mut data = [0u8; IO_CHUNK_SIZE];
                                                                        data[..length].copy_from_slice(&buffer);
                                                                        let io_chunk = IOChunk::new(RwMode::Server, serial_no, client_id, length as u16, data);
                                                                        let transfer = make_io_chunk_payload(&io_chunk);
                                                                        let transfer_packet = make_packet(EndpointType::Handler, Packet::Data, &transfer);
                                                                        writer.write_all(&transfer_packet).await.unwrap();
                                                                        writer.flush().await.unwrap();
                                                                    }
                                                                },
                                                                Err(_) => {},
                                                            }
                                                        }
                                                    }
                                                    
                                                },
                                                _ => {}
                                            }
                                        },
                                        _ => {},
                                    }
                                },
                                Err(e) => {
                                    sherpa.reset().unwrap();
                                    match e.kind() {
                                        ErrorKind::UnexpectedEof => {
                                            println!("Unexpected EOF");
                                            break;
                                        },
                                        ErrorKind::TimedOut => {
                                            avaibale.store(true, std::sync::atomic::Ordering::Relaxed);
                                        }
                                        _ => {},
                                    }
                                }
                            }
                        }
                    }
                } => {
                    self.sherpa.close().unwrap();
                },
                _ = ctrl_c() => {},
            }
        }
    }
}

impl Endpoint for TcpWorkerEndpoint {
    type Config = TcpWorkerConfig;
    type Output = Self;

    fn init(config: Self::Config) -> impl Future<Output = Option<Self::Output>> {
        let mut sherpa_proxy = Sherpa::new();
        sherpa_proxy.init(SHERPA_TOKENS, SHERPA_ENCODER, SHERPA_DECODER, SHERPA_JOINER);

        async move {
            Some(TcpWorkerEndpoint {
                ip: config.ip,
                port: config.port,
                sherpa: Arc::new(sherpa_proxy),
            })
        }
    }
}