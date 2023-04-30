mod connection;
mod creating_connection;
mod downloading;
mod uploading;
use creating_connection::creating_connection;
use downloading::downloading;

use muzzman_lib::prelude::*;
use std::ops::Range;
use std::{collections::HashMap, io::Seek};
use uploading::uploading;

#[module_link]
pub struct ModuleHttp;

impl ModuleHttp {
    pub fn new() -> Box<dyn TModule> {
        Box::new(Self)
    }
}

pub fn action_download(info: MRef, values: Vec<Type>) {
    let Some(url) = values.get(0)else{return};
    let Ok(url): Result<String, ()> = url.clone().try_into() else{return};
    let splited = url.split('/').collect::<Vec<&str>>();
    if let Some(filename) = splited.last() {
        if let Ok(session) = info.get_session() {
            if let Ok(location) = session.get_default_location() {
                if let Ok(element) = location.create_element(filename) {
                    let _ = element.set_module(Some(info.id()));
                    element.set_url(Some(url));
                    let _ = element.init();
                    let Some(should_enable) = values.get(1) else{return};
                    let Ok(should_enable) = should_enable.clone().try_into()else{return};
                    let _ = element.set_enabled(should_enable, None);
                }
            }
        }
    }
}

impl TModule for ModuleHttp {
    fn init(&self, module_ref: MRef) -> Result<(), SessionError> {
        let _ = module_ref.register_action(
            String::from("download"),
            vec![
                (
                    String::from("url"),
                    Value::new(Type::None, vec![TypeTag::String], vec![], true, ""),
                ),
                (
                    String::from("auto_start"),
                    Value::new(
                        Type::Bool(true),
                        vec![TypeTag::Bool],
                        vec![],
                        true,
                        "If should auto enable",
                    ),
                ),
            ],
            action_download,
        );
        Ok(())
    }

    fn get_name(&self) -> String {
        String::from("Http")
    }

    fn get_desc(&self) -> String {
        String::from("Http Module")
    }

    fn get_uid(&self) -> UID {
        1
    }

    fn get_version(&self) -> String {
        "MuzzManHttp: 1".to_string()
    }

    fn supported_versions(&self) -> std::ops::Range<u64> {
        1..2
    }

    fn init_settings(&self, values: &mut Values) -> Result<(), SessionError> {
        values.add(
            "buffer_size",
            Value::new(
                Type::USize(8192),
                vec![TypeTag::USize],
                vec![],
                true,
                "How much to download/upload per tick",
            ),
        );

        values.add(
            "recv",
            Value::new(
                Type::USize(0),
                vec![TypeTag::USize],
                vec![],
                false,
                "how much has recived!",
            ),
        );

        values.add(
            "sent",
            Value::new(
                Type::USize(0),
                vec![TypeTag::USize],
                vec![],
                false,
                "how much has sent!",
            ),
        );

        values.add(
            "headers",
            Value::new(
                Type::None,
                vec![TypeTag::None, TypeTag::HashMapSS],
                vec![],
                false,
                "response headers!",
            ),
        );
        Ok(())
    }

    fn init_element_settings(&self, values: &mut Values) -> Result<(), SessionError> {
        let mut method_enum = CustomEnum::default();
        method_enum.add("GET");
        method_enum.add("HEAD");
        method_enum.add("POST");
        method_enum.add("PUT");
        method_enum.add("DELETE");
        method_enum.add("CONNECT");
        method_enum.add("OPTIONS");
        method_enum.add("TRACE");
        method_enum.add("PATCH");
        method_enum.set_active(Some(0));

        let mut headers = HashMap::new();
        headers.insert("User-Agent".to_string(), "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/103.0.506 0.70 Safari/537.36 MuzzMan/0.1".to_string());

        //
        // TODO: Implement Https
        //

        values.add("method", Type::CustomEnum(method_enum));
        values.add("headers", Type::HashMapSS(headers));
        values.add(
            "port",
            Value::new(
                Type::None,
                vec![TypeTag::None, TypeTag::U16],
                vec![],
                true,
                "The port that will be used by connection! if none will auto detect from url",
            ),
        );
        values.add(
            "body",
            Value::new(
                Type::None,
                vec![TypeTag::FileOrData, TypeTag::None],
                vec![],
                true,
                "What should be uploaded!",
            ),
        );

        values.add(
            "upload-content-length",
            Value::new(
                Type::None,
                vec![TypeTag::USize, TypeTag::None],
                vec![],
                true,
                "how much to upload! if is none will upload the hole data",
            ),
        );

        values.add(
            "download-content-length",
            Value::new(
                Type::None,
                vec![TypeTag::USize, TypeTag::None],
                vec![],
                true,
                "how much to download! if is none will download all",
            ),
        );
        Ok(())
    }

    fn init_element(&self, element_row: ERow) -> Result<(), SessionError> {
        // the element data and module data should be added by the session when module is added before the init_element or step_element
        // the order is really important!

        element_row.write().unwrap().progress = 0.0;

        let mut element = element_row.write().unwrap();

        let _ = element.data.seek(std::io::SeekFrom::Start(0));

        element.element_data.unlock();
        element.settings.unlock();

        element.settings.set("conn", Type::None);
        element.settings.set("sent", Type::USize(0));
        element.settings.set("recv", Type::USize(0));

        element.statuses.push("Initializeting".to_owned()); // 0
        element.statuses.push("Negotieiting Connection".to_owned()); // 1
        element.statuses.push("Changing protocol/module".to_owned()); // 2
        element.statuses.push("Downloading".to_owned()); // 3
        element.statuses.push("Uploading".to_owned()); // 4
        element.statuses.push("Paused".to_owned()); // 5
        element.statuses.push("Resume".to_owned()); // 6
        element.statuses.push("Sync".to_owned()); // 7
        element.statuses.push("Complited".to_string()); // 8
        element.statuses.push("Error".to_string()); // 9
        element.status = 0;
        Ok(())
    }

    fn step_element(
        &self,
        element_row: ERow,
        control_flow: &mut ControlFlow,
        storage: &mut Storage,
    ) -> Result<(), SessionError> {
        let status = element_row.read().unwrap().status;

        match status {
            0 => {
                // Init
                let v_res_1;
                let v_res_2;

                {
                    let element = element_row.read().unwrap();
                    v_res_1 = element.element_data.validate();
                    v_res_2 = element.settings.validate();
                }

                if let Some(errors) = v_res_1 {
                    error(&element_row, format!("Error: element data: {}", errors));
                    return Ok(());
                }

                if let Some(errors) = v_res_2 {
                    error(&element_row, format!("Error: module data: {}", errors));
                    return Ok(());
                }

                {
                    let mut element = element_row.write().unwrap();
                    element.element_data.lock();
                    element.settings.lock();
                }

                // TODO: Validate url!

                {
                    let mut element = element_row.write().unwrap();
                    match element.element_data.get("port").unwrap().clone() {
                        Type::U16(_) => {}
                        _ => {
                            let mut port = 80;
                            if let Some(url) = element.url.clone() {
                                let v = url.split(':').collect::<Vec<&str>>();
                                if let Some(proto) = v.get(0) {
                                    if proto.trim() == "https" {
                                        port = 443;
                                    }
                                }
                            }
                            element.element_data.set("port", Type::U16(port));
                        }
                    }

                    match element
                        .element_data
                        .get("upload-content-length")
                        .unwrap()
                        .clone()
                    {
                        Type::USize(_) => {}
                        _ => {
                            match element.element_data.get("body").unwrap().clone() {
                                Type::None => {
                                    element
                                        .element_data
                                        .set("upload-content-length", Type::USize(0));
                                }
                                Type::FileOrData(mut ford) => {
                                    if let Ok(current) = ford.seek(std::io::SeekFrom::Current(0)) {
                                        let end = ford.seek(std::io::SeekFrom::End(0)).unwrap();
                                        ford.seek(std::io::SeekFrom::Start(current)).unwrap();

                                        element.element_data.set(
                                            "upload-content-length",
                                            Type::USize(end as usize),
                                        );
                                    } else {
                                        element
                                            .element_data
                                            .set("upload-content-length", Type::USize(0));
                                    }
                                }
                                _ => {}
                            };
                        }
                    }
                }

                element_row.set_status(1);
            }
            1 => {
                creating_connection(&element_row, storage);
            }
            2 => {
                // Change module
                todo!()
            }
            3 => {
                downloading(&element_row, storage);
            }
            4 => {
                uploading(&element_row, storage);
            }
            5 => { // Paused
            }
            6 => {
                // Resume
                todo!()
            }
            7 => {
                // Sync
                todo!()
            }
            8 => {
                // Complited
                element_row.write().unwrap().enabled = false;
                *control_flow = ControlFlow::Break;
            }
            9 => {
                // Error
                element_row.write().unwrap().enabled = false;
                *control_flow = ControlFlow::Break;
            }
            _ => {
                eprintln!("Some thing is rong with the element status for ModuleHTTP!")
            }
        }
        Ok(())
    }

    fn accept_extension(&self, _filename: &str) -> bool {
        // this will be for init location end will not be implemented for this module!
        false
    }

    fn accept_url(&self, url: String) -> bool {
        // if url has http://

        // in the feature https://
        // posibile for other module

        if let Some(protocol) = url.split('/').collect::<Vec<&str>>().first() {
            return matches!(protocol.trim(), "http:" | "https:");
        }
        false
    }

    fn accepted_extensions(&self) -> Vec<String> {
        todo!()
    }

    fn accepted_protocols(&self) -> Vec<String> {
        vec!["http".into(), "https".into()]
    }

    fn init_location(&self, _location_ref: LRef) -> Result<(), SessionError> {
        // For http has noting to do possibile to download everything from a web but is useless now
        Ok(())
    }

    fn step_location(
        &self,
        location_ref: LRow,
        control_flow: &mut ControlFlow,
        storage: &mut Storage,
    ) -> Result<(), SessionError> {
        Ok(())
    }

    fn notify(&self, _ref: Ref, event: Event) -> Result<(), SessionError> {
        Ok(())
    }

    fn c(&self) -> Box<dyn TModule> {
        Box::new(Self)
    }

    fn init_location_settings(&self, data: &mut Values) -> Result<(), SessionError> {
        todo!()
    }
}

pub fn error(element: &ERow, error: impl Into<String>) {
    let error = error.into();
    {
        let mut logger = element.get_logger(None);
        logger.error(error.clone())
    }
    element.write().unwrap().statuses[9] = error;
    element.set_status(9);
}
