use std::io;

use serde::{de::DeserializeOwned, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::{ClientMessage, ServerMessage};

pub const MAX_CLIENT_FRAME_BYTES: usize = 64 * 1024;
pub const MAX_SERVER_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Reads the next client frame. Returns None only for a clean frame-boundary EOF.
/// Keep this future running to completion or drop the stream on cancellation.
pub async fn read_client(
    reader: &mut (impl AsyncRead + Unpin),
) -> io::Result<Option<ClientMessage>> {
    read_frame(reader, MAX_CLIENT_FRAME_BYTES).await
}

/// Reads the next server frame. Returns None only for a clean frame-boundary EOF.
/// Keep this future running to completion or drop the stream on cancellation.
pub async fn read_server(
    reader: &mut (impl AsyncRead + Unpin),
) -> io::Result<Option<ServerMessage>> {
    read_frame(reader, MAX_SERVER_FRAME_BYTES).await
}

pub async fn write_client(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &ClientMessage,
) -> io::Result<()> {
    write_frame(writer, message, MAX_CLIENT_FRAME_BYTES).await
}

pub async fn write_server(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &ServerMessage,
) -> io::Result<()> {
    write_frame(writer, message, MAX_SERVER_FRAME_BYTES).await
}

async fn read_frame<T: DeserializeOwned>(
    reader: &mut (impl AsyncRead + Unpin),
    limit: usize,
) -> io::Result<Option<T>> {
    let mut header = [0; 4];
    if reader.read(&mut header[..1]).await? == 0 {
        return Ok(None);
    }
    reader.read_exact(&mut header[1..]).await?;
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LAN frame length is outside the allowed range",
        ));
    }

    let mut payload = vec![0; size];
    reader.read_exact(&mut payload).await?;
    let (message, tail) = postcard::take_from_bytes(&payload)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if !tail.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "LAN frame contains trailing bytes",
        ));
    }
    Ok(Some(message))
}

async fn write_frame<T: Serialize>(
    writer: &mut (impl AsyncWrite + Unpin),
    message: &T,
    limit: usize,
) -> io::Result<()> {
    let payload = postcard::to_allocvec(message)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if payload.is_empty() || payload.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "LAN frame length is outside the allowed range",
        ));
    }
    let length = u32::try_from(payload.len()).expect("frame limit fits wire length");
    writer.write_all(&length.to_be_bytes()).await?;
    writer.write_all(&payload).await
}
