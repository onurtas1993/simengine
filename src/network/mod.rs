use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::Duration,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMessage {
    pub input: String,
    pub payload: Vec<u8>,
}

pub fn send(endpoint: &str, message: &NetworkMessage) -> io::Result<()> {
    let mut stream = TcpStream::connect(endpoint)?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    write_message(&mut stream, message)
}

pub fn start_listener<F>(endpoint: String, on_message: F) -> io::Result<thread::JoinHandle<()>>
where
    F: Fn(NetworkMessage) + Send + Sync + 'static,
{
    let listener = TcpListener::bind(&endpoint)?;
    let on_message = Arc::new(on_message);

    let handle = thread::spawn(move || {
        println!("[network] listening on {endpoint}");

        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };

            let on_message = Arc::clone(&on_message);
            thread::spawn(move || match read_message(&mut stream) {
                Ok(message) => on_message(message),
                Err(err) => eprintln!("[network] failed to read message: {err}"),
            });
        }
    });

    Ok(handle)
}

fn write_message(stream: &mut TcpStream, message: &NetworkMessage) -> io::Result<()> {
    let body = serde_json::to_vec(message).map_err(io::Error::other)?;
    let len = u32::try_from(body.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "network message too large"))?;

    stream.write_all(&len.to_be_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}

fn read_message(stream: &mut TcpStream) -> io::Result<NetworkMessage> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes)?;
    let len = u32::from_be_bytes(len_bytes) as usize;

    let mut body = vec![0u8; len];
    stream.read_exact(&mut body)?;

    serde_json::from_slice(&body).map_err(io::Error::other)
}
