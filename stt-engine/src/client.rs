use std::{io::Read, time::Duration};
use derive_new::new;
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, time::sleep};

#[derive(Debug, Clone, new)]
pub struct RunningRecord {
    _wav_file: String,
    _connect_result: usize,
    _error_occurred: bool,
    _readfile_time: usize,
    _connecting_time: usize,
    _sending_time: usize,
    _receiving_time: usize,
    _transcribe_result: String,
}

impl RunningRecord {
    pub fn is_connect_success(self) -> bool {
        self._connect_result == 0
    }
}

unsafe impl Send for RunningRecord {}
unsafe impl Sync for RunningRecord {}

pub async fn run_with(ip: String, port: u16, wav_file: String, debug: bool) -> Result<RunningRecord, Box<dyn std::error::Error>> {
    // 读取WAV文件
    let start_time = std::time::Instant::now();
    let mut file = std::fs::File::open(wav_file.clone())?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).unwrap();
    let readfile_time = start_time.elapsed().as_nanos() as usize;
    if debug {
        println!("Connecting...");
    }

    // 连接到服务器
    tokio::select! {
        stream = async move {
            let start_time = std::time::Instant::now();
            match TcpStream::connect(format!("{}:{}", ip, port)).await {
                Ok(stream) => {
                    let connecting_time = start_time.elapsed().as_nanos() as usize;
                    Ok((stream, connecting_time))
                },
                Err(_) => Err("Failed to connect"),
            }
        } => {
            match stream {
                Ok((mut stream, connecting_time)) => {
                    // 发送WAV文件数据
                    let mut _error_occurred = false;
                    let mut _connect_result = 0;
                    let mut _transcribe_result = "".to_string();

                    let mut start_time = std::time::Instant::now();
                    if let Err(_) = stream.write_all(&data).await {
                        return Ok(RunningRecord::new(wav_file, 1, true, 0, 0, 0, 0, "".to_string()));
                    }

                    let sending_time = start_time.elapsed().as_nanos() as usize;

                    // 读取服务器响应，直到连接关闭
                    let mut buf = [0; 4096];
                    let timeout_duration = Duration::from_secs(2); // 设置超时时间为2秒
                    start_time = std::time::Instant::now();
                    loop {
                        tokio::select! {
                            result = async {
                                let n = stream.read(&mut buf).await;
                                match n {
                                    Ok(n) => {
                                        if n == 0 {
                                            _connect_result = 2;
                                            return None; // 没有数据可读，连接可能已经关闭
                                        }
                                        Some(String::from_utf8_lossy(&buf[..n]).to_string())
                                    },
                                    Err(_) => {
                                        _connect_result = 3;
                                        _error_occurred = true;
                                        return None; // 读取错误，返回None
                                    },
                                }
                            } => {
                                if let Some(result) = result {
                                    // 以'\n'分割，打印每条消息
                                    for line in result.split('\n') {
                                        if !line.is_empty() {
                                            if debug {
                                                println!("Received for {}: {}", wav_file, line);
                                            }
                                            _transcribe_result = line.to_string();
                                        }
                                    }
                                } else {
                                    break; // 没有数据可读，连接可能已经关闭
                                }
                            },
                            _ = sleep(timeout_duration) => {
                                if debug {
                                    println!("Reading from server timeout occurred");
                                }
                                break;
                            }
                        }
                    }
                    let receiving_time = start_time.elapsed().as_nanos() as usize;
                    // 主动关闭连接
                    stream.shutdown().await?;

                    if debug {
                        println!("Connection closed.");
                    }
                    Ok(RunningRecord::new(wav_file, _connect_result, _error_occurred, readfile_time, connecting_time, sending_time, receiving_time, _transcribe_result))
                }, // 连接成功，直接返回
                Err(_) => Ok(RunningRecord::new(wav_file, 4, true, 0, 0, 0, 0, "".to_string())), // 连接失败，返回错误
            }
        },
        _ = sleep(Duration::from_secs(20)) => Ok(RunningRecord::new(wav_file, 5, true, 0, 0, 0, 0, "".to_string())), // 尝试连接时长最多不超20s，超过后服务端会断开连接
    }
}