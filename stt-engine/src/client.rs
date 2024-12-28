use std::{io::Read, time::Duration};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream, time::sleep};

pub async fn run_with(wav_file: String) -> Result<(), Box<dyn std::error::Error>> {
    // 读取WAV文件
    let mut file = std::fs::File::open(wav_file)?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).unwrap();

    println!("Connecting...");

    // 连接到服务器
    let mut stream = TcpStream::connect("127.0.0.1:8888").await?;

    // 发送WAV文件数据
    stream.write_all(&data).await?;

    // 读取服务器响应，直到连接关闭
    let mut buf = [0; 1024];
    let timeout_duration = Duration::from_secs(5); // 设置超时时间为5秒
    
    loop {
        tokio::select! {
            result = async {
                let n = stream.read(&mut buf).await;
                match n {
                    Ok(n) => {
                        if n == 0 {
                            return None; // 没有数据可读，连接可能已经关闭
                        }
                        Some(String::from_utf8_lossy(&buf[..n]).to_string())
                    },
                    Err(_) => Some("Error reading from the socket".to_string()),
                }
            } => {
                if let Some(result) = result {
                    // 以'\n'分割，打印每条消息
                    for line in result.split('\n') {
                        if !line.is_empty() {
                            println!("Received: {}", line);
                        }
                    }
                } else {
                    break; // 没有数据可读，连接可能已经关闭
                }
            },
            _ = sleep(timeout_duration) => {
                println!("Timeout occurred");
                break;
            }
        }
    }

    // 主动关闭连接
    stream.shutdown().await?;

    println!("Connection closed.");
    Ok(())
}