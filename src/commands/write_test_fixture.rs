use crate::client::Client;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

pub fn fixture(
    responses: Vec<(&'static str, &'static str)>,
) -> (Client, thread::JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        responses.into_iter().map(|(status, body)| {
            let mut stream = accept_request(&listener);
            let request = read_request(&mut stream);
            write!(stream, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            request
        }).collect()
    });
    (
        Client::for_test("test", &format!("http://{address}")).unwrap(),
        server,
    )
}

fn accept_request(listener: &TcpListener) -> TcpStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match listener.accept() {
            Ok((stream, _)) => return stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "HTTP fixture timed out awaiting request"
                );
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => panic!("HTTP fixture accept failed: {error}"),
        }
    }
}

fn read_request(stream: &mut TcpStream) -> String {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut data = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0, "HTTP fixture received incomplete request");
        data.extend_from_slice(&buffer[..count]);
        if let Some(request) = parse_request(&data) {
            return request;
        }
    }
}

fn parse_request(data: &[u8]) -> Option<String> {
    let end = data.windows(4).position(|window| window == b"\r\n\r\n")?;
    let headers = String::from_utf8_lossy(&data[..end]);
    let length = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap_or(0);
    let body_end = end + 4 + length;
    if data.len() < body_end {
        return None;
    }
    Some(String::from_utf8(data[..body_end].to_vec()).unwrap())
}
