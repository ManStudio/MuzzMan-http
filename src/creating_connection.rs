use std::{
    collections::HashMap,
    io::{Read, Seek, SeekFrom, Write},
    net::TcpStream,
};

use muzzman_lib::prelude::*;

use crate::error;

pub fn creating_connection(element: &ERow, storage: &mut Storage) {
    let Some(url) = get_url(element) else {
        return
    };

    let Some(method) = get_method(element) else{
        return;
    };

    let Some(port) = get_port(element) else {
        return;
    };

    let headers = get_headers(element);

    let Ok(adresses) = url.socket_addrs(|| Some(port))else{
        error(element, "Error: cannot resolv host, is probably a invalid url or your dns is blocking it!");
        return;
    };

    let mut conn = None;
    for adress in adresses {
        if let Ok(connection) = TcpStream::connect(adress) {
            conn = Some(connection);
            break;
        }
    }

    let Some(mut conn) = conn else{
        error(element, "Error: cannot connect to host!");
        return;
    };

    let send = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\n",
        method,
        url.path(),
        url.domain().unwrap()
    );
    let send = send.as_bytes();

    let Ok(size) = conn.write(send) else{
        error(element, "Error: Connection faild!");
        return;
    };

    if size != send.len() {
        error(
            element,
            "Error: Cannot write to connection, that means that the url or port is invalid!",
        );
        return;
    }

    for header in headers {
        let send = format!("{}: {}\r\n", header.0, header.1);
        let send = send.as_bytes();
        let Ok(size) = conn.write(send)else{
            error(element, "Error: Connection faild!");
            return;
        };

        if size != send.len() {
            error(element, "Error: Cannot write to connection!");
            return;
        }
    }

    if let Some(data) = element.write().unwrap().element_data.get_mut("body") {
        if let Type::FileOrData(ford) = data {
            let _ = conn
                .write(
                    format!("Content-Length: {}", {
                        let res = {
                            if let Ok(cur) = ford.seek(SeekFrom::Current(0)) {
                                let res = ford.seek(SeekFrom::End(0)).unwrap();
                                ford.seek(SeekFrom::Start(cur)).unwrap();
                                res
                            } else {
                                0
                            }
                        };
                        res
                    })
                    .as_bytes(),
                )
                .unwrap();
        }
    }
    conn.write(b"\r\n\r\n").unwrap();

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
            let read = conn.read(&mut bytes).unwrap();
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

        if status != 200 {
            error(
                element,
                format!("Http Error: status: {} {}", status, status_str),
            );
            return;
        }

        let content_length;
        if let Some(data) = headers.get("Content-Length") {
            if let Ok(cl) = data.trim().parse::<usize>() {
                content_length = cl;
            } else {
                error(element, "Error: Cannot parse Content-Length");
                return;
            }
        } else {
            content_length = usize::MAX;
        }

        {
            let mut element = element.write().unwrap();
            element
                .element_data
                .set("download-content-length", Type::USize(content_length));
            element.module_data.set("headers", Type::HashMapSS(headers));
        }
    }

    storage.set(conn);
    element.set_status(4)
}

pub fn get_url(element: &ERow) -> Option<Url> {
    let error_i: u8;

    if let Some(url) = element.read().unwrap().element_data.get("url") {
        if let Type::String(url) = url {
            if let Ok(url) = Url::parse(url.clone().as_str()) {
                return Some(url);
            } else {
                error_i = 2;
            }
        } else {
            error_i = 1;
        }
    } else {
        error_i = 0;
    }

    //
    // Errors is posibile to be useless because every attribute are checked at init stage.
    // Errors is usefull only when a client ignore safty!
    //

    error(
        element,
        match error_i {
            0 => "Error: has no url!",
            1 => "Error: url should be string!",
            2 => "Error: invalid url!",
            _ => "Error: IDK",
        },
    );

    None
}

pub fn get_method(element: &ERow) -> Option<String> {
    let error_i: u8;

    if let Some(method) = element.read().unwrap().element_data.get("method") {
        if let Type::CustomEnum(method) = method {
            if let Some(selected) = method.get_active() {
                return Some(selected);
            } else {
                error_i = 2;
            }
        } else {
            error_i = 1;
        }
    } else {
        error_i = 0;
    }

    error(
        element,
        match error_i {
            0 => "Error: has no method!",
            1 => "Error: method should be custom_enum!",
            2 => "Error: method has noting selected!",
            _ => "IDK",
        },
    );

    None
}

pub fn get_headers(element: &ERow) -> HashMap<String, String> {
    let mut headers = HashMap::new();
    if let Some(data) = element.read().unwrap().element_data.get("headers") {
        match data {
            Type::HashMapSS(tmp_headers) => headers = tmp_headers.clone(),
            _ => {}
        }
    }
    headers
}

pub fn get_port(element: &ERow) -> Option<u16> {
    let error_i: u8;

    if let Some(data) = element.read().unwrap().element_data.get("port") {
        if let Type::U16(port) = data {
            return Some(*port);
        } else {
            error_i = 1;
        }
    } else {
        error_i = 0;
    }

    error(
        element,
        match error_i {
            0 => "Error: has no port",
            1 => "Error: port should be u8",
            _ => "IDK",
        },
    );

    None
}