use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom, Write},
    net::TcpStream,
};

use url::Url;

use muzzman_lib::prelude::*;

use crate::{connection::Connection, error};

pub fn creating_connection(element: &ERow, storage: &mut Storage) -> Result<(), SessionError> {
    let mut logger = element.get_logger(None);

    let Some(url) = element.read().unwrap().url.clone() else {
        return Err(error(element, "No url"));
    };

    let Ok(url) = Url::parse(&url)else{
        return Err(error(element, "Cannot parse url"));
    };

    let method = get_method(element)?;

    let port = get_port(element)?;

    let headers = get_headers(element);

    let Ok(adresses) = url.socket_addrs(|| Some(port))else{
        return Err(error(element, "Error: cannot resolv host, is probably a invalid url or your dns is blocking it!"));
    };

    let mut conn = None;

    if port == 443 {
        logger.info("Start connection on port 433 that means that should be tls");
        logger.info("Try to create tls connection!");
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS
                .0
                .iter()
                .map(|e| {
                    rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                        e.subject,
                        e.spki,
                        e.name_constraints,
                    )
                })
                .collect(),
        };
        let config = rustls::client::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let server_name = if let Some(domain) = url.domain() {
            if let Ok(server_name) = rustls::ServerName::try_from(domain) {
                server_name
            } else {
                return Err(error(element, "Cannot resolv host name"));
            }
        } else if let Ok(server_name) =
            rustls::ServerName::try_from(adresses[0].ip().to_string().as_ref())
        {
            server_name
        } else {
            return Err(error(element, "Invaild url"));
        };

        logger.info("Tls setup success!");

        if let Ok(connection) =
            rustls::client::ClientConnection::new(std::sync::Arc::new(config), server_name)
        {
            let mut tcp = None;
            for adress in adresses {
                if let Ok(connection) = TcpStream::connect(adress) {
                    tcp = Some(connection);
                    break;
                }
            }

            if let Some(tcp) = tcp {
                logger.info("Tls Connected");
                conn = Some(Connection::TLSClient(connection, tcp))
            }
        }
    } else {
        logger.info("Starting Tcp Connection");
        for adress in adresses {
            if let Ok(connection) = TcpStream::connect(adress) {
                logger.info("Connection succesfuly!");
                conn = Some(Connection::TCP(connection));
                break;
            }
        }
    }

    let Some(mut conn) = conn else{
        return Err(error(element, "Error: cannot connect to host!"));
    };

    let send = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\n",
        method,
        url.path(),
        url.domain().unwrap()
    );
    logger.info(format!("Sending Request: {}", send));
    let send = send.as_bytes();

    let Ok(size) = conn.write(send) else{
        return Err(error(element, "Error: Connection faild!"));
    };

    if size != send.len() {
        return Err(error(
            element,
            "Error: Cannot write to connection, that means that the url or port is invalid!",
        ));
    }

    logger.info(format!("Headers: {:?}", headers));

    for header in headers {
        let send = format!("{}: {}\r\n", header.0, header.1);
        let send = send.as_bytes();
        let Ok(_) = conn.write_all(send)else{
            return Err(error(element, "Error: Connection faild!"));
        };
    }

    if let Some(Type::FileOrData(ford)) = element.write().unwrap().element_data.get_mut("body") {
        let send = format!("Content-Length: {}", {
            if let Ok(cur) = ford.seek(SeekFrom::Current(0)) {
                let res = ford.seek(SeekFrom::End(0)).unwrap();
                ford.seek(SeekFrom::Start(cur)).unwrap();
                res
            } else {
                0
            }
        });
        logger.info(format!("Add header: {}", send));
        let res = conn.write_all(send.as_bytes());

        if let Err(err) = res {
            logger.error("Cannot Send Contelt-Length header!");
            return Err(error(element, err.to_string()));
        }
    }
    conn.write_all(b"\r\n\r\n").unwrap();
    logger.info("Response beagin reading");

    {
        let mut bytes = [0; 1];

        let mut r: u8 = 0;
        let mut n: u8 = 0;

        let mut count = 0;

        let mut buffer = String::new();

        let mut status: u16 = 0;
        let mut status_str: String = String::new();
        let mut headers = HashMap::new();

        loop {
            if r == 2 && n == 2 {
                break;
            }
            match conn.read(&mut bytes) {
                Ok(read) => {
                    if read == 1 {
                        match bytes[0] {
                            b'\r' => r += 1,
                            b'\n' => n += 1,
                            _ => {
                                if r > 0 && n > 0 {
                                    match count {
                                        0 => {
                                            let spaces = buffer.split(' ').collect::<Vec<&str>>();
                                            status = spaces[1].parse().unwrap();
                                            let mut data = String::new();
                                            for s in spaces[2..].iter() {
                                                data.push_str(s);
                                                data.push(' ');
                                            }
                                            status_str = data;
                                        }
                                        _ => {
                                            let map = buffer.split(':').collect::<Vec<&str>>();
                                            headers.insert(map[0].to_owned(), map[1].to_owned());
                                        }
                                    }
                                    count += 1;
                                    buffer.clear();
                                }
                                buffer.push(bytes[0] as char);

                                r = 0;
                                n = 0;
                            }
                        }
                    } else {
                        break;
                    }
                }

                Err(err) => match err.kind() {
                    std::io::ErrorKind::WouldBlock => {}
                    _ => {
                        return Err(error(element, format!("Error: {:?}", err)));
                    }
                },
            }
        }

        logger.info(format!("Status: {} {}", status, status_str));
        logger.info(format!("Response Headers: {:?}", headers));

        if status != 200 {
            return Err(error(
                element,
                format!("Http Error: status: {} {}", status, status_str),
            ));
        }

        let content_length;
        if let Some(data) = headers.get("Content-Length") {
            if let Ok(cl) = data.trim().parse::<usize>() {
                content_length = cl;
            } else {
                return Err(error(element, "Error: Cannot parse Content-Length"));
            }
        } else {
            logger.info("No Content Length finded!");
            content_length = usize::MAX;
        }

        logger.info(format!("Content-Length set to {}", content_length));

        {
            let mut element = element.write().unwrap();
            element
                .element_data
                .set("download-content-length", Type::USize(content_length));
            element.settings.set("headers", Type::HashMapSS(headers));
        }
    }

    storage.set(conn);
    element.set_status(4);
    Ok(())
}

pub fn get_method(element: &ERow) -> Result<String, SessionError> {
    let error_i: u8;

    if let Some(method) = element.read().unwrap().element_data.get("method") {
        if let Type::CustomEnum(method) = method {
            if let Some(selected) = method.get_active() {
                return Ok(selected);
            } else {
                error_i = 2;
            }
        } else {
            error_i = 1;
        }
    } else {
        error_i = 0;
    }

    return Err(error(
        element,
        match error_i {
            0 => "Error: has no method!",
            1 => "Error: method should be custom_enum!",
            2 => "Error: method has noting selected!",
            _ => "IDK",
        },
    ));
}

pub fn get_headers(element: &ERow) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    if let Some(Type::HashMapSS(tmp_headers)) = element.read().unwrap().element_data.get("headers")
    {
        headers = tmp_headers.clone();
    }
    headers
}

pub fn get_port(element: &ERow) -> Result<u16, SessionError> {
    let error_i: u8;

    if let Some(data) = element.read().unwrap().element_data.get("port") {
        if let Type::U16(port) = data {
            return Ok(*port);
        } else {
            error_i = 1;
        }
    } else {
        error_i = 0;
    }

    Err(error(
        element,
        match error_i {
            0 => "Error: has no port",
            1 => "Error: port should be u8",
            _ => "IDK",
        },
    ))
}
