// |-----------------------------------------------------------------------|
// |                    header                  |     payload(optional)    |
// |-----------------------------------------------------------------------|
// | magic number | endpoint type | packet type |          payload         |
// |-----------------------------------------------------------------------|
// |  2 bytes     |   1 byte      |    1 byte   |                          |
// |-----------------------------------------------------------------------|
// The payload is a serialized protobuf message.

use derive_new::new;

pub type SerialNo = [u8; 6];
pub type ClientId = u32;
pub const MAGCI_NUMBER: u16 = 0x89ab;
pub const IO_CHUNK_SIZE: usize = 1024;


#[repr(u8)]
pub enum EndpointType {
    Handler = 0,
    Client = 1,
    Unknown = 255,
}

impl From<u8> for EndpointType {
    fn from(value: u8) -> Self {
        match value {
            0 => EndpointType::Handler,
            1 => EndpointType::Client,
            _ => EndpointType::Unknown,
        }
    }
}

#[repr(u8)]
pub enum Packet {
    Register = 0,
    RegOk = 1,
    Status = 2,
    Ack = 3,
    Connect = 4,
    ConnOk = 5,
    ConnRejected = 6,
    Data = 7,
    Result = 8,
    Alive = 9,
    Unknown = 255,
}

impl From<u8> for Packet {
    fn from(value: u8) -> Self {
        match value {
            0 => Packet::Register,
            1 => Packet::RegOk,
            2 => Packet::Status,
            3 => Packet::Ack,
            4 => Packet::Connect,
            5 => Packet::ConnOk,
            6 => Packet::ConnRejected,
            7 => Packet::Data,
            8 => Packet::Result,
            9 => Packet::Alive,
            _ => Packet::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, new)]
pub struct Register {
    ip: [u8; 4],
    port: u16,
}

impl Register {
    pub fn get_ip(&self) -> [u8; 4] {
        self.ip
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }
}

#[derive(Debug, Clone, Copy, new)]
pub struct Ack {
    serial_no: SerialNo,
    available: bool,
}

impl Ack {
    pub fn get_serial_no(&self) -> SerialNo {
        self.serial_no
    }

    pub fn is_available(&self) -> bool {
        self.available
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum RwMode {
    Client = 0,
    Server = 1,
    Unknown = 255,
}

impl From<u8> for RwMode {
    fn from(value: u8) -> Self {
        match value {
            0 => RwMode::Client,
            1 => RwMode::Server,
            _ => RwMode::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, new)]
pub struct IOChunk {
    mode: RwMode,
    serial_no: SerialNo,
    client_id: ClientId,
    length: u16,
    data: [u8; IO_CHUNK_SIZE],
}

impl IOChunk {
    pub fn get_serial_no(&self) -> SerialNo {
        self.serial_no
    }

    pub fn get_mode(&self) -> RwMode {
        self.mode
    }

    pub fn get_client_id(&self) -> ClientId {
        self.client_id
    }

    pub fn get_length(&self) -> u16 {
        self.length
    }

    pub fn get_data(&self) -> &[u8; IO_CHUNK_SIZE] {
        &self.data
    }
}

#[derive(Debug, Clone, Copy, new)]
pub struct TranscribeResult {
    length: u16,
    data: [u8; IO_CHUNK_SIZE],
}

impl TranscribeResult {
    pub fn get_length(&self) -> u16 {
        self.length
    }

    pub fn get_data(&self) -> &[u8; IO_CHUNK_SIZE] {
        &self.data
    }
}

pub mod maker {
    use super::{Ack, ClientId, EndpointType, IOChunk, Packet, Register, SerialNo, TranscribeResult, MAGCI_NUMBER};

    pub fn make_serial_no(ip: [u8; 4], port: u16) -> SerialNo {
        let mut serial_no: SerialNo = [0; 6];
        serial_no[0..4].copy_from_slice(&ip);
        serial_no[4..6].copy_from_slice(&port.to_be_bytes());
        serial_no
    }

    pub fn make_register_payload(register: &Register) -> Vec<u8> {
        let mut payload = vec![];
        payload.extend_from_slice(&register.ip);
        payload.extend_from_slice(&register.port.to_be_bytes());
        payload
    }

    pub fn make_ack_payload(ack: &Ack) -> Vec<u8> {
        let mut payload = ack.serial_no.to_vec();
        payload.push(if ack.available { 1u8 } else { 0u8 });
        payload
    }

    pub fn make_connect_ok_payload(serial_no: &SerialNo, client_id: &ClientId) -> Vec<u8> {
        let mut payload = serial_no.to_vec();
        payload.extend_from_slice(&client_id.to_be_bytes());
        payload
    }

    pub fn make_io_chunk_payload(io_chunk: &IOChunk) -> Vec<u8> {
        let mut payload = vec![];
        payload.push(io_chunk.mode as u8);
        payload.extend_from_slice(&io_chunk.serial_no);
        payload.extend_from_slice(&io_chunk.client_id.to_be_bytes());
        payload.extend_from_slice(&io_chunk.length.to_be_bytes());
        payload.extend_from_slice(&io_chunk.data);
        payload
    }

    pub fn make_transcribe_result_payload(transcribe_result: &TranscribeResult) -> Vec<u8> {
        let mut payload = vec![];
        payload.extend_from_slice(&transcribe_result.length.to_be_bytes());
        payload.extend_from_slice(&transcribe_result.data);
        payload
    }

    pub fn make_pure_packet(e_type: EndpointType, p_type: Packet) -> Vec<u8> {
        let mut packet: Vec<u8> = vec![];
        packet.extend_from_slice(MAGCI_NUMBER.to_be_bytes().as_slice());
        packet.push(e_type as u8);
        packet.push(p_type as u8);
        packet
    }

    pub fn make_packet(e_type: EndpointType, p_type: Packet, payload: &[u8]) -> Vec<u8> {
        let mut packet: Vec<u8> = vec![];
        packet.extend_from_slice(MAGCI_NUMBER.to_be_bytes().as_slice());
        packet.push(e_type as u8);
        packet.push(p_type as u8);
        packet.extend_from_slice(payload);
        packet
    }
}

pub mod parser {
    use std::io::{Error, ErrorKind};

    use tokio::{io::AsyncReadExt, net::tcp::OwnedReadHalf};

    use crate::endpoint::SttResult;

    use super::{Ack, ClientId, EndpointType, IOChunk, Packet, Register, SerialNo, TranscribeResult, IO_CHUNK_SIZE, MAGCI_NUMBER};

    pub async fn is_magic_number_limited(reader: &mut OwnedReadHalf, limit: u64) -> SttResult<Error> {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_millis(limit)) => Err(Error::new(ErrorKind::TimedOut, "Timed out")),
            result = is_magic_number(reader) => result,
        }
    }

    pub async fn is_magic_number(reader: &mut OwnedReadHalf) -> SttResult<Error> {
        match reader.read_u16().await {
            Ok(magic_number) => {
                if magic_number == MAGCI_NUMBER {
                    Ok(())
                } else {
                    Err(Error::new(ErrorKind::InvalidData, format!("Invalided magic number is {:#x}", magic_number)))
                }
            },
            Err(e) => Err(e),
        }
    }

    pub async fn parse_endpoint_type(reader: &mut OwnedReadHalf) -> EndpointType {
        if let Ok(endpoint_type) = reader.read_u8().await {
            endpoint_type.into()
        } else {
            EndpointType::Unknown
        }
    }

    pub async fn parse_packet_type(reader: &mut OwnedReadHalf) -> Packet {
        if let Ok(packet_type) = reader.read_u8().await {
            packet_type.into()
        } else {
            Packet::Unknown
        }
    }

    // WORKER REGISTER PACKET PAYLOAD
    // |--------------------|
    // |     ip   |   port  |
    // |--------------------|
    // |  4 bytes | 2 bytes |
    // |--------------------|
    pub async fn parse_register_payload(reader: &mut OwnedReadHalf) -> Option<Register> {
        let mut payload = [0u8; 6];
        if let Ok(_) = reader.read_exact(&mut payload).await {
            Some(Register {
                ip: payload[0..4].try_into().unwrap(),
                port: u16::from_be_bytes(payload[4..6].try_into().unwrap()),
            })
        } else {
            None
        }
    }

    // WORKER REG_OK PACKET PAYLOAD
    // |----------------------------------|
    // |   serial_no                      |
    // |----------------------------------|
    // |   6 bytes                        |
    // |----------------------------------|
    pub async fn parse_reg_ok_payload(reader: &mut OwnedReadHalf) -> Option<SerialNo> {
        let mut payload = [0u8; 6];
        if let Ok(_) = reader.read_exact(&mut payload).await {
            Some(payload)
        } else {
            None
        }
    }

    // CLIENT CONN_OK PACKET PAYLOAD
    // |----------------------------------|
    // |   serial_no | client_id          |
    // |----------------------------------|
    // |   6 bytes   |  4 bytes           |
    // |----------------------------------|
    pub async fn parse_connect_ok_payload(reader: &mut OwnedReadHalf) -> Option<(SerialNo, ClientId)> {
        let mut payload = [0u8; 10];
        if let Ok(_) = reader.read_exact(&mut payload).await {
            Some((payload[0..6].try_into().unwrap(), u32::from_be_bytes(payload[6..10].try_into().unwrap())))
        } else {
            None
        }
    }

    // WORKER STATUS PACKET PAYLOAD
    // |----------------------------------|
    // |   serial_no                      |
    // |----------------------------------|
    // |   6 bytes                        |
    // |----------------------------------|
    pub async fn parse_status_payload(reader: &mut OwnedReadHalf) -> Option<SerialNo> {
        let mut payload = [0u8; 6];
        if let Ok(_) = reader.read_exact(&mut payload).await {
            Some(payload)
        } else {
            None
        }
    }

    // WORKER ACK PACKET PAYLOAD
    // |----------------------------------|
    // |   serial_no |   available        |
    // |----------------------------------|
    // |   6 bytes   |    1 byte          |
    // |----------------------------------|
    pub async fn parse_ack_payload(reader: &mut OwnedReadHalf) -> Option<Ack> {
        let mut payload = [0u8; 7];
        if let Ok(_) = reader.read_exact(&mut payload).await {
            Some(Ack {
                serial_no: payload[0..6].try_into().unwrap(),
                available: payload[6] != 0,
            })
        } else {
            None
        }
    }

    // IO CHUNK PACKET PAYLOAD
    // |---------------------------------------------------------|
    // |  mode  |  serial_no | client_id  |  length |    data    |
    // |----------------------------------|---------|------------|
    // | 1 byte |   6 bytes  |  4 bytes   | 2 bytes |            |
    // |----------------------------------|---------|------------|
    pub async fn parse_io_chunk_payload(reader: &mut OwnedReadHalf) -> Option<IOChunk> {
        let mut payload_header = [0u8; 13];
        if reader.read_exact(&mut payload_header).await.is_err() {
            return None;
        }

        let mode = payload_header[0];
        let serial_no = payload_header[1..7].try_into().unwrap();
        let client_id = u32::from_be_bytes(payload_header[7..11].try_into().unwrap());
        let length = u16::from_be_bytes(payload_header[11..13].try_into().unwrap());
        let mut payload_buf = [0u8; IO_CHUNK_SIZE];
        if reader.read_exact(&mut payload_buf).await.is_err() {
            return None;
        } else {
            Some(IOChunk {
                mode: mode.into(),
                serial_no,
                client_id,
                length,
                data: payload_buf,
            })
        }
    }
    // TRANSCRIBE RESULT PACKET PAYLOAD
    // |----------------------------------|
    // |   length  |   data               |
    // |----------------------------------|
    // |  2 bytes  | 1024 byte            |
    // |----------------------------------|
    pub async fn parse_transcribe_result_payload(reader: &mut OwnedReadHalf) -> Option<TranscribeResult> {
        let mut payload_header = [0u8; 2];
        if reader.read_exact(&mut payload_header).await.is_err() {
            return None;
        }

        let length = u16::from_be_bytes(payload_header.try_into().unwrap());
        let mut payload_buf = [0u8; IO_CHUNK_SIZE];
        if reader.read_exact(&mut payload_buf).await.is_err() {
            return None;
        } else {
            Some(TranscribeResult {
                length,
                data: payload_buf,
            })
        }
    }
}