//! On Linux the window's player gets a transcript's cached audio over HTTP,
//! from this server on 127.0.0.1. WebKitGTK's player can't read media by the
//! asset protocol ("Loading failed, error: FormatError", while GStreamer
//! played the file), and from a blob of the WAV's bytes it started a minute
//! in and played a clicked row's audio from elsewhere: GStreamer asked the
//! blob for byte ranges, and the lengths that came back didn't match them
//! (the Omarchy test, 2026-10-25). Byte ranges from a web server are what
//! every page's audio plays from.
//!
//! It listens on the loopback only, at a port the system picks, and serves
//! only the files the window was handed, each at a path with 128 random
//! bits in it: another program on the computer, or a page in a browser, can
//! reach the port but can't name a file. It answers only to its own name,
//! 127.0.0.1 and the port, which a page that renamed its own site to
//! 127.0.0.1 does not send.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// The files served, by their URLs' paths.
type Files = Arc<Mutex<HashMap<String, PathBuf>>>;

pub struct Server {
    port: u16,
    /// 128 random bits, in hex, in every file's path.
    token: String,
    files: Files,
}

impl Server {
    /// Listens, on a thread of its own, until the app quits.
    pub fn start() -> io::Result<Server> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let port = listener.local_addr()?.port();
        let mut bits = [0u8; 16];
        File::open("/dev/urandom")?.read_exact(&mut bits)?;
        let token = bits.iter().map(|b| format!("{b:02x}")).collect();
        let files = Files::default();
        let served = files.clone();
        thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let files = served.clone();
                // a thread each, so that a connection the player has stopped
                // reading from holds up no other; an error is mostly the
                // player closing one mid-answer, which is how a seek ends
                thread::spawn(move || {
                    let _ = answer(conn, port, &files);
                });
            }
        });
        Ok(Server { port, token, files })
    }

    /// Where the window's player gets the file at path, from now on.
    pub fn url(&self, path: &Path) -> String {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let at = format!("/{}/{}", self.token, name);
        self.files.lock().unwrap().insert(at.clone(), path.into());
        format!("http://127.0.0.1:{}{}", self.port, at)
    }
}

/// What a request asks, as far as this needs.
struct Request {
    method: String,
    path: String,
    host: String,
    range: Option<String>,
}

impl Request {
    /// The request line and headers, from no more than the first 16 KiB.
    fn read(conn: &TcpStream) -> io::Result<Request> {
        let mut r = BufReader::new(conn.take(16 << 10));
        let mut line = String::new();
        r.read_line(&mut line)?;
        let mut words = line.split_whitespace();
        let mut req = Request {
            method: words.next().unwrap_or("").into(),
            path: words.next().unwrap_or("").into(),
            host: String::new(),
            range: None,
        };
        loop {
            line.clear();
            if r.read_line(&mut line)? == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            let h = line.trim_end();
            if h.is_empty() {
                return Ok(req);
            }
            if let Some((name, value)) = h.split_once(':') {
                if name.eq_ignore_ascii_case("host") {
                    req.host = value.trim().into();
                } else if name.eq_ignore_ascii_case("range") {
                    req.range = Some(value.trim().into());
                }
            }
        }
    }
}

/// Answers one request, then closes: a seek opens a new connection, which
/// costs nothing on the loopback.
fn answer(conn: TcpStream, port: u16, files: &Mutex<HashMap<String, PathBuf>>) -> io::Result<()> {
    conn.set_read_timeout(Some(Duration::from_secs(10)))?;
    // each answer's head in one write, sent at once
    conn.set_nodelay(true)?;
    let req = Request::read(&conn)?;
    let mut out = conn;
    let found = match req.method.as_str() {
        "GET" | "HEAD" if req.host == format!("127.0.0.1:{port}") => {
            let path = files.lock().unwrap().get(&req.path).cloned();
            path.and_then(|p| File::open(p).ok())
        }
        _ => None,
    };
    let Some(mut file) = found else {
        return out.write_all(
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        );
    };
    let len = file.metadata()?.len();
    let (first, end, mut head) = match want(req.range.as_deref(), len) {
        Want::All => (0, len, "HTTP/1.1 200 OK\r\n".to_string()),
        Want::Part(first, last) => (
            first,
            last + 1,
            format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {first}-{last}/{len}\r\n"
            ),
        ),
        Want::Past => {
            let head = format!(
                "HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{len}\r\n\
                 Content-Length: 0\r\nConnection: close\r\n\r\n"
            );
            return out.write_all(head.as_bytes());
        }
    };
    // no-store: the cache's WAV stays the audio's one copy, with none in
    // WebKit's cache
    head += &format!(
        "Content-Type: audio/wav\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\
         Cache-Control: no-store\r\nConnection: close\r\n\r\n",
        end - first
    );
    out.write_all(head.as_bytes())?;
    if req.method == "GET" {
        file.seek(SeekFrom::Start(first))?;
        io::copy(&mut file.take(end - first), &mut out)?;
    }
    Ok(())
}

/// What a request's Range header asks of a file of len bytes. HTTP lets a
/// server answer one it doesn't take with the whole file: here, several
/// ranges at once, which the player doesn't ask for.
#[derive(Debug, PartialEq)]
enum Want {
    All,
    /// The bytes from first to last, both in.
    Part(u64, u64),
    /// None the file has: past its end.
    Past,
}

fn want(range: Option<&str>, len: u64) -> Want {
    let Some((a, b)) = range
        .and_then(|r| r.trim().strip_prefix("bytes="))
        .filter(|spec| !spec.contains(','))
        .and_then(|spec| spec.split_once('-'))
    else {
        return Want::All;
    };
    let (a, b) = (a.trim(), b.trim());
    let (first, last) = match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(first), Ok(last)) if first <= last => (first, last),
        (Ok(first), Err(_)) if b.is_empty() => (first, u64::MAX),
        // the last n bytes
        (Err(_), Ok(n)) if a.is_empty() => (len.saturating_sub(n), u64::MAX),
        _ => return Want::All,
    };
    if first >= len {
        Want::Past
    } else {
        Want::Part(first, last.min(len - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(want(None, 100), Want::All);
        assert_eq!(want(Some("bytes=0-"), 100), Want::Part(0, 99));
        assert_eq!(want(Some("bytes=10-19"), 100), Want::Part(10, 19));
        assert_eq!(want(Some("bytes=90-200"), 100), Want::Part(90, 99));
        assert_eq!(want(Some("bytes=-10"), 100), Want::Part(90, 99));
        assert_eq!(want(Some("bytes=-500"), 100), Want::Part(0, 99));
        assert_eq!(want(Some("bytes=100-"), 100), Want::Past);
        assert_eq!(want(Some("bytes=-0"), 100), Want::Past);
        assert_eq!(want(Some("bytes=0-"), 0), Want::Past);
        // ignored: the whole file
        assert_eq!(want(Some("bytes=20-10"), 100), Want::All);
        assert_eq!(want(Some("bytes=0-1,5-6"), 100), Want::All);
        assert_eq!(want(Some("items=0-1"), 100), Want::All);
        assert_eq!(want(Some("bytes=-"), 100), Want::All);
    }

    /// A raw request to the server, its answer back: the head and the body.
    fn ask(s: &Server, req: &str) -> (String, Vec<u8>) {
        let mut c = TcpStream::connect((Ipv4Addr::LOCALHOST, s.port)).unwrap();
        c.write_all(req.as_bytes()).unwrap();
        let mut got = Vec::new();
        c.read_to_end(&mut got).unwrap();
        let at = got.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let body = got.split_off(at);
        (String::from_utf8(got).unwrap(), body)
    }

    #[test]
    fn serves_what_it_was_handed() {
        let dir = std::env::temp_dir().join(format!("pktito-serve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wav = dir.join("1761600000000.wav");
        let bytes: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(&wav, &bytes).unwrap();
        let s = Server::start().unwrap();
        let host = format!("127.0.0.1:{}", s.port);
        let url = s.url(&wav);
        let at = format!("/{}/1761600000000.wav", s.token);
        assert_eq!(url, format!("http://{host}{at}"));
        let req = |method: &str, more: &str| {
            format!("{method} {at} HTTP/1.1\r\nHost: {host}\r\n{more}\r\n")
        };

        let (head, body) = ask(&s, &req("GET", ""));
        assert!(head.starts_with("HTTP/1.1 200 OK\r\n"), "{head}");
        assert!(head.contains("Content-Length: 1000\r\n"), "{head}");
        assert_eq!(body, bytes);

        let (head, body) = ask(&s, &req("GET", "Range: bytes=100-\r\n"));
        assert!(
            head.starts_with("HTTP/1.1 206 Partial Content\r\n"),
            "{head}"
        );
        assert!(
            head.contains("Content-Range: bytes 100-999/1000\r\n"),
            "{head}"
        );
        assert!(head.contains("Content-Length: 900\r\n"), "{head}");
        assert_eq!(body, &bytes[100..]);

        let (head, body) = ask(&s, &req("HEAD", "range: bytes=0-9\r\n"));
        assert!(head.contains("Content-Length: 10\r\n"), "{head}");
        assert!(body.is_empty());

        let (head, _) = ask(&s, &req("GET", "Range: bytes=1000-\r\n"));
        assert!(head.starts_with("HTTP/1.1 416 "), "{head}");

        // not its name, not a path it was handed, not a method it takes
        let wrong = [
            format!("GET {at} HTTP/1.1\r\nHost: localhost:{}\r\n\r\n", s.port),
            format!("GET /{}/1.wav HTTP/1.1\r\nHost: {host}\r\n\r\n", s.token),
            format!("GET /1761600000000.wav HTTP/1.1\r\nHost: {host}\r\n\r\n"),
            req("POST", ""),
        ];
        for r in wrong {
            let (head, _) = ask(&s, &r);
            assert!(head.starts_with("HTTP/1.1 404 "), "{r}");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
