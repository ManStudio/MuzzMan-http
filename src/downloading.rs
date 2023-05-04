use std::io::{Read, Write};

use muzzman_lib::prelude::*;

use crate::{connection::Connection, error};

pub fn downloading(element: &ERow, storage: &mut Storage) -> Result<(), SessionError> {
    let mut content_length: usize = 0;
    if let Some(Type::USize(data)) = element
        .read()
        .unwrap()
        .element_data
        .get("download-content-length")
    {
        content_length = *data;
    }

    if content_length > 0 {
        let mut buffer_size = 0;
        if let Some(Type::USize(len)) = element.read().unwrap().settings.get("buffer_size") {
            buffer_size = *len;
        }

        let mut buffer = vec![0; buffer_size];
        let len;

        {
            let Some(conn) = storage.get_mut::<Connection>()else{
                return Ok(());
            };

            match conn.read(&mut buffer) {
                Ok(size) => {
                    len = size;
                }
                Err(err) => {
                    match err.kind() {
                        std::io::ErrorKind::WouldBlock => {}
                        _ => return Err(error(element, "Error: Connection close unexpected!")),
                    }
                    return Ok(());
                }
            }
        }

        let recived;

        'd: {
            let mut element = element.write().unwrap();
            if let Some(Type::USize(recived_b)) = element.settings.get_mut("recv") {
                *recived_b += len;
                recived = *recived_b;
                break 'd;
            }

            recived = len;
            element.settings.set("recv", Type::USize(len));
        }

        element
            .write()
            .unwrap()
            .data
            .write_all(&buffer[0..len])
            .unwrap();

        let progress = if content_length > 0 {
            ((recived as f64) / (content_length as f64)) as f32
        } else {
            50.0
        };

        element.write().unwrap().progress = progress;
        if len == 0 {
            element.set_status(8);
        }

        if recived == content_length {
            element.set_status(8);
        }
    }

    Ok(())
}
