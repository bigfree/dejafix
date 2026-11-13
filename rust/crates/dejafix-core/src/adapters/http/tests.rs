use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::latest_version;

const AGENT: &str = "dejafix-test/9.9";
const PATIENT: Duration = Duration::from_secs(5);

fn reply(status: &str, body: &[u8]) -> Vec<u8> {
    let mut raw = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    raw.extend_from_slice(body);
    raw
}

fn serve(raw: Vec<u8>) -> (SocketAddr, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        while !request.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
        }
        let _ = sender.send(String::from_utf8_lossy(&request).into_owned());
        let _ = stream.write_all(&raw);
        let _ = stream.flush();
    });
    (address, receiver)
}

fn fetch(raw: Vec<u8>) -> Option<String> {
    let (address, _request) = serve(raw);
    latest_version(
        &format!("http://{address}/pypi/dejafix/json"),
        AGENT,
        PATIENT,
    )
}

#[test]
fn a_valid_reply_yields_the_version() {
    let body = br#"{"info": {"version": "0.4.2", "name": "dejafix"}}"#;
    assert_eq!(fetch(reply("200 OK", body)), Some("0.4.2".to_owned()));
}

#[test]
fn a_non_success_status_yields_nothing() {
    assert_eq!(fetch(reply("404 Not Found", b"{}")), None);
}

#[test]
fn invalid_json_yields_nothing() {
    assert_eq!(fetch(reply("200 OK", b"<html>nope")), None);
}

#[test]
fn a_missing_version_yields_nothing() {
    assert_eq!(fetch(reply("200 OK", br#"{"info": {}}"#)), None);
    assert_eq!(fetch(reply("200 OK", br#"{"releases": {}}"#)), None);
}

#[test]
fn a_version_that_is_not_a_string_yields_nothing() {
    assert_eq!(fetch(reply("200 OK", br#"{"info": {"version": 3}}"#)), None);
    assert_eq!(
        fetch(reply("200 OK", br#"{"info": {"version": null}}"#)),
        None
    );
}

#[test]
fn a_body_over_one_mebibyte_yields_nothing() {
    let mut body = br#"{"info": {"version": "1.0.0"}, "pad": ""#.to_vec();
    body.extend(std::iter::repeat_n(b'x', 1024 * 1024 + 16));
    body.extend_from_slice(b"\"}");
    assert_eq!(fetch(reply("200 OK", &body)), None);
}

#[test]
fn a_closed_port_yields_nothing() {
    let port = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let url = format!("http://127.0.0.1:{port}/pypi/dejafix/json");
    assert_eq!(latest_version(&url, AGENT, PATIENT), None);
}

#[test]
fn a_server_that_never_answers_times_out() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let url = format!("http://{address}/pypi/dejafix/json");
    let started = Instant::now();
    let version = latest_version(&url, AGENT, Duration::from_millis(200));
    assert_eq!(version, None);
    assert!(started.elapsed() < Duration::from_secs(3));
    drop(listener);
}

#[test]
fn the_user_agent_is_sent() {
    let (address, request) = serve(reply("200 OK", br#"{"info": {"version": "1.0.0"}}"#));
    let url = format!("http://{address}/pypi/dejafix/json");
    assert_eq!(
        latest_version(&url, AGENT, PATIENT),
        Some("1.0.0".to_owned())
    );
    let received = request.recv_timeout(PATIENT).unwrap().to_lowercase();
    assert!(received.starts_with("get /pypi/dejafix/json "));
    assert!(received.contains("user-agent: dejafix-test/9.9\r\n"));
}
