mod creating_connection;
mod downloading;
mod uploading;
use creating_connection::creating_connection;
use downloading::downloading;

use muzzman_lib::prelude::*;
use std::{collections::HashMap, io::Seek};
use uploading::uploading;

#[module_link]
pub struct ModuleHttp;

impl ModuleHttp {
    pub fn new() -> Box<dyn TModule> {
        Box::new(Self)
    }
}

pub fn action_download(info: MInfo, values: Vec<Type>) {
    if let Some(url) = values.get(0) {
        if let Type::String(url) = url {
            let s = url.clone();
            let splited = s.split("/").collect::<Vec<&str>>();
            if let Some(filename) = splited.get(splited.len() - 1) {
                if let Ok(session) = info.get_session() {
                    if let Ok(location) = session.get_default_location() {
                        if let Ok(element) = location.create_element(&filename.to_string()) {
                            let _ = element.set_module(Some(info.clone()));
                            let _ = element.init();
                            if let Ok(mut data) = element.get_element_data() {
                                data.set("url", Type::String(url.to_owned()));
                                let _ = element.set_element_data(data);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl TModule for ModuleHttp {
    fn init(&self, info: MInfo) -> Result<(), String> {
        let _ = info.register_action(
            String::from("download"),
            vec![(
                String::from("url"),
                Value::new(Type::None, vec![TypeTag::String], vec![], true, ""),
            )],
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

    fn init_settings(&self, data: &mut Data) {
        data.add(
            "buffer_size",
            Value::new(
                Type::USize(8192),
                vec![TypeTag::USize],
                vec![],
                true,
                "How much to download/upload per tick",
            ),
        );

        data.add(
            "recv",
            Value::new(
                Type::USize(0),
                vec![TypeTag::USize],
                vec![],
                false,
                "how much has recived!",
            ),
        );

        data.add(
            "sent",
            Value::new(
                Type::USize(0),
                vec![TypeTag::USize],
                vec![],
                false,
                "how much has sent!",
            ),
        );

        data.add(
            "headers",
            Value::new(
                Type::None,
                vec![TypeTag::None, TypeTag::HashMapSS],
                vec![],
                false,
                "response headers!",
            ),
        );
    }

    fn init_element_settings(&self, data: &mut Data) {
        let mut method_enum = CustomEnum::new();
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

        data.add(
            "url",
            Value::new(
                Type::None,
                vec![TypeTag::String, TypeTag::Url],
                vec![],
                true,
                "The url that will be used to upload/download",
            ),
        );

        data.add("method", Type::CustomEnum(method_enum));
        data.add("headers", Type::HashMapSS(headers));
        data.add(
            "port",
            Value::new(
                Type::None,
                vec![TypeTag::None, TypeTag::U16],
                vec![],
                true,
                "The port that will be used by connection! if none will auto detect from url",
            ),
        );
        data.add(
            "body",
            Value::new(
                Type::None,
                vec![TypeTag::FileOrData, TypeTag::None],
                vec![],
                true,
                "What should be uploaded!",
            ),
        );

        data.add(
            "upload-content-length",
            Value::new(
                Type::None,
                vec![TypeTag::USize, TypeTag::None],
                vec![],
                true,
                "how much to upload! if is none will upload the hole data",
            ),
        );

        data.add(
            "download-content-length",
            Value::new(
                Type::None,
                vec![TypeTag::USize, TypeTag::None],
                vec![],
                true,
                "how much to download! if is none will download all",
            ),
        );
    }

    fn init_element(&self, element: ERow) {
        // the element data and module data should be added by the session when module is added before the init_element or step_element
        // the order is really important!

        element.write().unwrap().progress = 0.0;

        let mut element = element.write().unwrap();

        let _ = element.data.seek(std::io::SeekFrom::Start(0));

        element.element_data.unlock();
        element.module_data.unlock();

        element.module_data.set("conn", Type::None);
        element.module_data.set("sent", Type::USize(0));
        element.module_data.set("recv", Type::USize(0));

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
    }

    fn step_element(&self, element: ERow, control_flow: &mut ControlFlow, storage: &mut Storage) {
        let status = element.read().unwrap().status;

        match status {
            0 => {
                // Init
                let v_res_1;
                let v_res_2;

                {
                    let element = element.read().unwrap();
                    v_res_1 = element.element_data.validate();
                    v_res_2 = element.module_data.validate();
                }

                if let Some(errors) = v_res_1 {
                    error(&element, format!("Error: element data: {}", errors));
                    return;
                }

                if let Some(errors) = v_res_2 {
                    error(&element, format!("Error: module data: {}", errors));
                    return;
                }

                {
                    let mut element = element.write().unwrap();
                    element.element_data.lock();
                    element.module_data.lock();
                }

                // TODO: Validate url!

                {
                    let mut element = element.write().unwrap();
                    match element.element_data.get("port").unwrap().clone() {
                        Type::U16(_) => {}
                        _ => {
                            let mut port = 80;
                            if let Some(url) = element.element_data.get("url") {
                                if let Type::String(url) = url {
                                    let v = url.split(":").collect::<Vec<&str>>();
                                    if let Some(proto) = v.get(0) {
                                        if proto.trim() == "https" {
                                            port = 443;
                                        }
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

                element.set_status(1);
            }
            1 => {
                creating_connection(&element, storage);
            }
            2 => {
                // Change module
                todo!()
            }
            3 => {
                downloading(&element, storage);
            }
            4 => {
                uploading(&element, storage);
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
                element.write().unwrap().enabled = false;
                *control_flow = ControlFlow::Break;
            }
            9 => {
                // Error
                element.write().unwrap().enabled = false;
                *control_flow = ControlFlow::Break;
            }
            _ => {
                eprintln!("Some thing is rong with the element status for ModuleHTTP!")
            }
        }
    }

    fn accept_extension(&self, _filename: &str) -> bool {
        // this will be for init location end will not be implemented for this module!
        false
    }

    fn accept_url(&self, url: Url) -> bool {
        // if url has http://

        // in the feature https://
        // posibile for other module

        if let Some(protocol) = url.as_str().split('/').collect::<Vec<&str>>().get(0) {
            if protocol.trim() == "http:".trim() {
                return true;
            }
        }

        false
    }

    fn init_location(&self, _location: LInfo, _data: FileOrData) {
        // For http has noting to do possibile to download everything from a web but is useless now
    }

    fn c(&self) -> Box<dyn TModule> {
        Box::new(Self)
    }
}

pub fn error(element: &ERow, error: impl Into<String>) {
    element.write().unwrap().statuses[9] = String::from(error.into());
    element.set_status(9);
}