use super::*;

#[test]
#[cfg(unix)]
fn test_socket_listen_connect_and_peer_uid() {
    use std::io::{Read, Write};

    let temp = tempfile::tempdir().unwrap();
    let sock_path = temp.path().join("test.sock");

    let listener = listen_restricted(&sock_path, 0o600).unwrap();
    let handle = std::thread::spawn({
        let sock_path = sock_path.clone();
        move || {
            let mut stream = connect_unix(&sock_path).unwrap();
            let peer = peer_uid(&stream).unwrap();
            assert_eq!(peer, current_uid());
            stream.write_all(b"ping").unwrap();
        }
    });

    let (mut conn, _) = listener.accept().unwrap();
    let conn_peer = peer_uid(&conn).unwrap();
    assert_eq!(conn_peer, current_uid());

    let mut buf = [0u8; 4];
    conn.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"ping");

    handle.join().unwrap();
}
