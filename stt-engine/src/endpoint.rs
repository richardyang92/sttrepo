use std::future::Future;

pub trait Endpoint {
    type Config;
    type Output;

    fn init(config: Self::Config) -> impl Future<Output = Option<Self::Output>>;
    fn run(&self) -> impl Future<Output = ()>;
}

pub trait Executor {
    type Context;
    type Channel;

    fn execute(&self) -> impl Future<Output = ()>;
}

trait Channel {
    type Message;

    fn open(&mut self, capacity: usize) -> impl Future<Output = ()>;
    fn close(&mut self) -> impl Future<Output = ()>;
}

trait Sender<M> {
    fn send(&self, message: M) -> impl Future<Output = ()>;
}

trait Receiver<M> {
    fn recv(&self, _message: M) -> ();
}

pub mod server {
    use std::{future::Future, sync::{atomic::AtomicBool, Arc}, time::Duration};

    use derive_new::new;
    use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{tcp::OwnedWriteHalf, TcpListener, TcpStream}, signal::ctrl_c, sync::mpsc, time::sleep};

    use crate::sherpa::Sherpa;

    use super::{Channel, Endpoint, Executor, Sender};

    pub(crate) const SHERPA_TOKENS: &str = "../sherpa/sherpa-models/tokens.txt";
    pub(crate) const SHERPA_ENCODER: &str = "../sherpa/sherpa-models/encoder-epoch-20-avg-1-chunk-16-left-128.onnx";
    pub(crate) const SHERPA_DECODER: &str = "../sherpa/sherpa-models/decoder-epoch-20-avg-1-chunk-16-left-128.onnx";
    pub(crate) const SHERPA_JOINER: &str = "../sherpa/sherpa-models/joiner-epoch-20-avg-1-chunk-16-left-128.onnx";

    #[derive(Debug, Clone, new)]
    pub struct ServerConfig {
        ip: &'static str,
        port: u16,
        channel_num: usize,
        channel_capacity: usize,
    }

    pub enum ServerMessage {
        Connected(OwnedWriteHalf),
        Disconnected,
        DataReceived(Vec<u8>),
        CloseChannel,
    }

    #[derive(Debug)]
    struct TcpStreamChannel {
        sherpa_proxy: Option<Arc<Sherpa>>,
        sender: Option<mpsc::Sender<ServerMessage>>,
        onwed_writer: Option<OwnedWriteHalf>,
        is_selected: Arc<AtomicBool>,
    }

    impl TcpStreamChannel {
        fn is_selected(&self) -> bool {
            self.is_selected.load(std::sync::atomic::Ordering::Relaxed)
        }

        fn select(&self) -> () {
            self.is_selected.store(true, std::sync::atomic::Ordering::Relaxed)
        }
    }

    impl Default for TcpStreamChannel {
        fn default() -> Self {
            Self { sherpa_proxy: None, sender: None, onwed_writer: None, is_selected: Arc::new(false.into()), }
        }
    }

    impl Channel for TcpStreamChannel {
        type Message = ServerMessage;
        
        fn open(&mut self, capacity: usize) -> impl Future<Output = ()> {
            async move {
                let mut sherpa_proxy = Sherpa::new();
                sherpa_proxy.init(SHERPA_TOKENS, SHERPA_ENCODER, SHERPA_DECODER, SHERPA_JOINER);
                self.sherpa_proxy.replace(Arc::new(sherpa_proxy));

                let (tx, mut rx) = mpsc::channel::<ServerMessage>(capacity);
                self.sender = Some(tx);

                {
                    let mut onwed_writer = self.onwed_writer.take();
                    let is_selected = self.is_selected.clone();
                    let sherpa_proxy = self.sherpa_proxy.as_ref().unwrap().clone();
                    tokio::spawn(async move {
                        while let Some(message) = rx.recv().await {
                            match message {
                                ServerMessage::Connected(writer) => {
                                    onwed_writer.replace(writer);
                                },
                                ServerMessage::Disconnected => {
                                    match sherpa_proxy.reset() {
                                        Ok(_) => {
                                            println!("Sharpa proxy reset successfully");
                                            is_selected.store(false, std::sync::atomic::Ordering::Relaxed);
                                        },
                                        Err(e) => {
                                            eprintln!("Error resetting sherpa proxy: {}", e);
                                        }
                                    }
                                },
                                ServerMessage::DataReceived(data) => {
                                    if let Some(writer) = &mut onwed_writer {
                                        // data两两一组，每组数据转成f32，然后把这些f32数据收集起来组成一个Vec<f32>
                                        let sample = data.chunks(2).map(|chunk| {
                                            ((chunk[1] as i16) << 8 | (chunk[0] as i16) & 0xff) as f32 / 32767f32
                                        }).collect::<Vec<f32>>();
                                        match sherpa_proxy.transcribe(&sample) {
                                            Ok(result) => {
                                                if let Err(e) = writer.write_all(format!("{}\n", &result).as_bytes()).await {
                                                    eprintln!("Error writing to stream: {}", e);
                                                }
                                            },
                                            Err(e) => {
                                                eprintln!("Error transcribing: {}", e);
                                            }
                                        }
                                    }
                                },
                                ServerMessage::CloseChannel => {
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
                if let Some(sender) = &self.sender {
                    sender.send(ServerMessage::Disconnected).await.unwrap();
                }
                if let Some(sherpa_proxy) = &mut self.sherpa_proxy {
                    if let Err(e) = sherpa_proxy.close() {
                        eprintln!("Error closing sherpa proxy: {}", e);
                    }
                }
            }
        }
    }

    impl Sender<ServerMessage> for TcpStreamChannel {
        fn send(&self, message: ServerMessage) -> impl Future<Output = ()> {
            async move {
                if let Some(sender) = &self.sender {
                    sender.send(message).await.unwrap();
                }
            }
        }
    }

    struct TcpListenerExecutor {
        listener: Option<TcpListener>,
        channels: Arc<Vec<Arc<TcpStreamChannel>>>,
    }

    impl TcpListenerExecutor {
        fn build_from(listener: TcpListener, num: usize, capacity: usize) -> impl Future<Output = Self> {
            let mut channels = Vec::new();
            async move {
                for _ in 0..num {
                    let mut channel = TcpStreamChannel::default();
                    channel.open(capacity).await;
                    channels.push(Arc::new(channel));
                }
    
                Self {
                    listener: Some(listener),
                    channels: Arc::new(channels),
                }
            }
        }
    }
    
    impl Executor for TcpListenerExecutor {
        type Context = TcpStream;
        type Channel = TcpStreamChannel;
    
        fn execute(&self) -> impl Future<Output = ()> {
            async move {
                tokio::select! {
                    _ = async {
                        if let Some(listener) = &self.listener {
                            loop {
                                if let Ok((stream, _)) = listener.accept().await {
                                    {
                                        let channels = self.channels.clone();
                                        tokio::spawn(async move {
                                            let mut channel = None;
                                            let max_attempts = 10; // 设置最大尝试次数
                                            let mut retrying_count = 0;
                                            loop {
                                                if retrying_count >= max_attempts {
                                                    eprintln!("Failed to select a channel after {} attempts", max_attempts);
                                                    break; // 达到最大尝试次数后退出循环
                                                }
                                                if let Some(ch) = channels.iter().find(|c| !c.is_selected()) {
                                                    ch.select();
                                                    channel.replace(Arc::clone(ch));
                                                    break;
                                                } else {
                                                    retrying_count += 1;
                                                    eprintln!("No channel available for selection, retrying count: {}", retrying_count);
                                                    sleep(Duration::from_secs(2)).await; // 等待一段时间再尝试选择其他通道（例如，10毫秒）
                                                }
                                            }
                                            match channel {
                                                Some(channel) => {
                                                    // handle(stream, channel.clone()).await;
                                                    let channel = channel.clone();
                                                    let addr = stream.peer_addr().unwrap();
                                                    println!("Connected: {}", addr);
                                                    let (mut reader, writer) = stream.into_split();
                                                    channel.send(ServerMessage::Connected(writer)).await;

                                                    tokio::spawn(async move {
                                                        // println!("Reader start...");
                                                        let mut buf = [0; 1024];
                                                        let timeout_duration = Duration::from_secs(2); // 设置超时时间为2秒

                                                        loop {
                                                            tokio::select! {
                                                                result = async {
                                                                    match reader.read(&mut buf).await {
                                                                        Ok(n) => Ok(buf[..n].to_vec()),
                                                                        Err(_) => Err("Error reading from stream"),
                                                                    }
                                                                } => match result {
                                                                    Ok(data) => channel.send(ServerMessage::DataReceived(data)).await,
                                                                    Err(e) => eprintln!("Error: {}", e),
                                                                },
                                                                _ = sleep(timeout_duration) => {
                                                                    println!("Timeout occurred");
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        channel.send(ServerMessage::Disconnected).await;
                                                    });
                                                },
                                                None => {
                                                    eprintln!("No channel available, closed stream...");
                                                    drop(stream);
                                                },
                                            }
                                        });
                                    }
                                }
                            }
                        }
                    } => {},
                    _ = ctrl_c() => {}
                }
                println!("\nServer is shutting down...");
                for channel in self.channels.iter() {
                    channel.send(ServerMessage::CloseChannel).await;
                }
            }
        }
    }

    pub struct Server {
        executor: TcpListenerExecutor,
    }

    impl Endpoint for Server {
        type Config = ServerConfig;
        type Output = Self;

        fn init(config: Self::Config) -> impl Future<Output = Option<Self::Output>> {
            let addr = format!("{}:{}", config.ip, config.port);
            async move {
                if let Ok(listener) = TcpListener::bind(addr).await {
                    let executor = TcpListenerExecutor::build_from(listener,
                        config.channel_num, config.channel_capacity).await;
                    Some(Self {
                        executor,
                    })
                } else {
                    None
                }
            }
        }

        fn run(&self) -> impl std::future::Future<Output = ()> {
            async move {
                self.executor.execute().await;
            }
        }
    }
}