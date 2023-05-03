use std::io::{Read, Write};

use muzzman_lib::prelude::*;

use crate::{connection::Connection, error};

pub fn uploading(element: &ERow, storage: &mut Storage) -> Result<(), SessionError> {
    let mut sent = 0;
    if let Some(Type::USize(ptr)) = element.read().unwrap().settings.get("sent") {
        sent = *ptr;
    }

    let mut buffer_size = get_buffer_size(element)?;

    let content_length = get_upload_content_length(element)?;

    if buffer_size + sent > content_length {
        buffer_size = content_length - sent;
    }

    log::info!("Sent: {}", sent);
    log::info!("Buffer size: {}", buffer_size);

    let mut bytes = vec![0; buffer_size];
    let mut add = 0;

    if let Some(Type::FileOrData(ford)) = element.write().unwrap().element_data.get_mut("body") {
        add = ford.read(&mut bytes).unwrap();
    }

    if let Some(conn) = storage.get_mut::<Connection>() {
        let _res = conn.write(&bytes[0..add]);
    } else {
        element.set_status(1);
        return Ok(());
    }

    sent += add;

    if let Some(Type::USize(ptr)) = element.write().unwrap().settings.get_mut("sent") {
        *ptr = sent;
    }

    log::info!("New sent: {}", sent);

    if add == 0 {
        element.set_status(3);
    }
    Ok(())
}

pub fn get_buffer_size(element: &ERow) -> Result<usize, SessionError> {
    let error_i;

    if let Some(buffer_size) = element.read().unwrap().settings.get("buffer_size") {
        if let Type::USize(buffer_size) = buffer_size {
            return Ok(*buffer_size);
        } else {
            error_i = 1;
        }
    } else {
        error_i = 0;
    }

    Err(error(
        element,
        match error_i {
            0 => "Error: module data has no `buffer_size`, you should add at module data a attribute named `buffer_usize` with time usize!",
            1 => "Error: module data `buffer_size` is not usize!",
            _ => "IDK",
        },
    ))
}

fn get_upload_content_length(element: &ERow) -> Result<usize, SessionError> {
    let error_i;

    if let Some(content_length) = element
        .read()
        .unwrap()
        .element_data
        .get("upload-content-length")
    {
        if let Type::USize(content_length) = content_length {
            return Ok(*content_length);
        } else {
            error_i = 1;
        }
    } else {
        error_i = 0;
    }

    Err(error(
        element,
        match error_i {
            0 => "Error: element data has no `upload-content-length`",
            1 => "Error: element data `upload-content-length` is not usize",
            _ => "IDK",
        },
    ))
}
