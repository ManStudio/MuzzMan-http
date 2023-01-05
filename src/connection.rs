use std::{
    io::{Read, Write},
    net::TcpStream,
};

use rustls::ClientConnection;

pub enum Connection {
    TCP(TcpStream),
    TLSClient(ClientConnection, TcpStream),
}

impl Write for Connection {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Connection::TCP(conn) => {
                let res = conn.write(buf);
                let _ = self.flush();
                res
            }
            Connection::TLSClient(trans, tcp) => trans.writer().write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Connection::TCP(conn) => conn.flush(),
            Connection::TLSClient(trans, conn) => {
                if trans.wants_read() {
                    let _ = trans.read_tls(conn);
                    let _ = trans.process_new_packets();
                }
                if trans.wants_write() {
                    let _ = trans.write_tls(conn);
                }
                Ok(())
            }
        }
    }
}

impl Read for Connection {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let _ = self.flush();
        match self {
            Connection::TCP(conn) => conn.read(buf),
            Connection::TLSClient(trans, conn) => trans.reader().read(buf),
        }
    }
}
