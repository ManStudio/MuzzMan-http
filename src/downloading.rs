use std::io::{Read, Write};

use muzzman_lib::prelude::*;

use crate::{connection::Connection, error};

pub fn downloading(element: &ERow, storage: &mut Storage) {
    let mut content_length: usize = 0;
    if let Some(data) = element
        .read()
        .unwrap()
        .element_data
        .get("download-content-length")
    {
        if let Type::USize(data) = data {
            content_length = *data;
        }
    }

    if content_length > 0 {
        let mut buffer_size = 0;
        if let Some(data) = element.read().unwrap().module_data.get("buffer_size") {
            if let Type::USize(len) = data {
                buffer_size = *len;
            }
        }

        let mut buffer = vec![0; buffer_size];
        let len;

        {
            let Some(conn) = storage.get_mut::<Connection>()else{
                return;
            };

            match conn.read(&mut buffer) {
                Ok(size) => {
                    len = size;
                }
                Err(err) => {
                    match err.kind() {
                        std::io::ErrorKind::WouldBlock => {}
                        _ => error(element, "Error: Connection close unexpected!"),
                    }
                    return;
                }
            }
        }

        let recived;

        'd: {
            let mut element = element.write().unwrap();
            if let Some(data) = element.module_data.get_mut("recv") {
                if let Type::USize(recived_b) = data {
                    *recived_b += len;
                    recived = *recived_b;
                    break 'd;
                }
            }

            recived = len;
            element.module_data.set("recv", Type::USize(len));
        }

        element
            .write()
            .unwrap()
            .data
            .write_all(&buffer[0..len])
            .unwrap();

        let progress;

        if content_length > 0 {
            progress = ((recived as f64) / (content_length as f64)) as f32;
        } else {
            progress = 50.0;
        }

        element.write().unwrap().progress = progress;
        if len == 0 {
            element.set_status(8);
        }

        if recived == content_length {
            element.set_status(8);
        }

        return;
    }
}
