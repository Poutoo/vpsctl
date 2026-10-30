//! Trames de l'IPC locale de Discord : opcode (u32, little-endian), longueur
//! (u32, little-endian), puis la charge en JSON.

use std::io;

use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const OP_HANDSHAKE: u32 = 0;
pub const OP_FRAME: u32 = 1;
pub const OP_CLOSE: u32 = 2;
pub const OP_PING: u32 = 3;
pub const OP_PONG: u32 = 4;

/// Taille maximale d'une trame reçue : au-delà, la connexion est abandonnée
/// (READY, la plus grande, fait quelques Kio).
const MAX_PAYLOAD: usize = 256 * 1024;

pub fn encode(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string();
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(body.as_bytes());
    frame
}

pub async fn write<W: AsyncWrite + Unpin>(
    writer: &mut W,
    op: u32,
    payload: &Value,
) -> io::Result<()> {
    writer.write_all(&encode(op, payload)).await?;
    writer.flush().await
}

pub async fn read<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<(u32, Value)> {
    let mut header = [0u8; 8];
    reader.read_exact(&mut header).await?;
    let op = u32::from_le_bytes(header[0..4].try_into().expect("4 octets"));
    let len = u32::from_le_bytes(header[4..8].try_into().expect("4 octets")) as usize;
    if len > MAX_PAYLOAD {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "trame Discord trop grande",
        ));
    }
    let mut body = vec![0u8; len];
    reader.read_exact(&mut body).await?;
    let payload =
        serde_json::from_slice(&body).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok((op, payload))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encodes_opcode_length_and_json_payload() {
        let frame = encode(
            OP_HANDSHAKE,
            &json!({"v": 1, "client_id": "123456789012345678"}),
        );
        assert_eq!(&frame[0..4], &[0, 0, 0, 0]);
        // Longueur en little-endian : celle de la charge JSON qui suit.
        assert_eq!(&frame[4..8], &((frame.len() - 8) as u32).to_le_bytes());
        let body: Value = serde_json::from_slice(&frame[8..]).unwrap();
        assert_eq!(body, json!({"v": 1, "client_id": "123456789012345678"}));
    }

    #[tokio::test]
    async fn reads_back_what_it_writes() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write(&mut a, OP_FRAME, &json!({"evt": "READY"}))
            .await
            .unwrap();
        assert_eq!(
            read(&mut b).await.unwrap(),
            (OP_FRAME, json!({"evt": "READY"}))
        );
    }

    #[tokio::test]
    async fn refuses_oversized_or_invalid_frames() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        a.write_all(&[1, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0x7F])
            .await
            .unwrap();
        assert!(read(&mut b).await.is_err());

        let (mut a, mut b) = tokio::io::duplex(1024);
        a.write_all(&[1, 0, 0, 0, 3, 0, 0, 0]).await.unwrap();
        a.write_all(b"{{{").await.unwrap();
        assert!(read(&mut b).await.is_err());
    }
}
