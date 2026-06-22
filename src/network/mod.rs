use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
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

#[derive(Default)]
pub struct NetworkSender {
    streams: HashMap<String, TcpStream>,
}

impl NetworkSender {
    pub fn send(&mut self, endpoint: &str, message: &NetworkMessage) -> io::Result<()> {
        match self.write_to_endpoint(endpoint, message) {
            Ok(()) => Ok(()),
            Err(err) => {
                self.streams.remove(endpoint);
                self.write_to_endpoint(endpoint, message).map_err(|_| err)
            }
        }
    }

    fn write_to_endpoint(&mut self, endpoint: &str, message: &NetworkMessage) -> io::Result<()> {
        if !self.streams.contains_key(endpoint) {
            self.streams
                .insert(endpoint.to_string(), connect_stream(endpoint)?);
        }

        let stream = self
            .streams
            .get_mut(endpoint)
            .expect("stream was just inserted");
        write_message(stream, message)
    }
}

pub fn send(endpoint: &str, message: &NetworkMessage) -> io::Result<()> {
    let mut stream = connect_stream(endpoint)?;
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
            thread::spawn(move || {
                loop {
                    match read_message(&mut stream) {
                        Ok(message) => on_message(message),
                        Err(err) if is_connection_closed(&err) => break,
                        Err(err) => {
                            eprintln!("[network] failed to read message: {err}");
                            break;
                        }
                    }
                }
            });
        }
    });

    Ok(handle)
}

fn connect_stream(endpoint: &str) -> io::Result<TcpStream> {
    let stream = TcpStream::connect(endpoint)?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.set_nodelay(true)?;
    Ok(stream)
}

fn is_connection_closed(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::UnexpectedEof
            | io::ErrorKind::ConnectionReset
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::BrokenPipe
    )
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
