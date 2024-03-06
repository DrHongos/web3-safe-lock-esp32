//! It will print out the IP to interact with, you need to be connected in the same WiFi network 

pub mod types;
pub mod helpers;
pub mod config;

use types::*;
use std::{
    sync::Mutex,
    str::FromStr,
    sync::mpsc::channel,
};
use embedded_svc::{
    http::{
        Headers, 
        Method,
        client::Client as HttpClient,
    },
    io::{Read, Write},
};
use esp_idf_svc::{
    hal::{
        prelude::Peripherals,
        gpio::PinDriver
    },
    eventloop::EspSystemEventLoop,
    http::{
        server::EspHttpServer,
        client::EspHttpConnection,
    },
    nvs::EspDefaultNvsPartition,
    wifi::{BlockingWifi, EspWifi},
    systime::EspSystemTime,
};
use log::*;

use alloy_signer::Signature;
use alloy_primitives::Address;
static INDEX_HTML: &str = include_str!("http_server_page.html");

// Max payload length
const MAX_LEN: usize = 134; // 132 is the length of the signed message

// Need lots of stack to parse JSON (extended due to Signature::recover_address_from_prehash() gave stack overflow)
const STACK_SIZE: usize = 20480;// originally 10240;


fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = Peripherals::take()?;
    let sys_loop = EspSystemEventLoop::take()?;
    let nvs = EspDefaultNvsPartition::take()?;

    // Setup Wifi
    let mut wifi = BlockingWifi::wrap(
        EspWifi::new(peripherals.modem, sys_loop.clone(), Some(nvs))?,
        sys_loop,
    )?;
    info!("Connecting wifi");
    helpers::connect_wifi(&mut wifi)?;
    
    // Creates server    
    let server_configuration = esp_idf_svc::http::server::Configuration {
        stack_size: STACK_SIZE,
        ..Default::default()
    };
    
    // Local storage to validate a message
    let data_validation: Mutex<Option<VerifyData>> = Mutex::new(None);

    // Internal channels to handle Callback actions
    let (tx, rx) = channel::<EventType>();

    info!("Start server creation");
    let mut server = EspHttpServer::new(&server_configuration)?;
    server.fn_handler("/", Method::Get, |req| {
        req.into_ok_response()?
            .write_all(INDEX_HTML.as_bytes())
            .map(|_| ())
    })?;

    server.fn_handler::<anyhow::Error, _>("/post", Method::Post, |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;

        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }

        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let mut resp = req.into_ok_response()?;

        if let Ok(form) = serde_json::from_slice::<FormData>(&buf) {
            // Pseudo-random-number (esp_random is unsafe)
            let random_number = EspSystemTime::now(&EspSystemTime {}).subsec_nanos() / 65537;
            let msg = helpers::typed_data_for_document(form.name, form.account, random_number);
            let msg_signing_hash = msg
                .eip712_signing_hash()
                .expect("No signing hash");
            let st = VerifyData {
                account: Address::from_str(form.account).expect("Address incorrect"),
                msg: msg_signing_hash,
                rand: random_number
            };
            // store current validation data
            tx.send(EventType::Load(st)).unwrap();
            // returns the message to be signed
            let tdata = serde_json::json!(msg).to_string();
            write!(resp, "{}", tdata)?;
        } else {
            resp.write_all("JSON error".as_bytes())?;
        }

        Ok(())
    })?;

    server.fn_handler::<anyhow::Error, _>("/verify", Method::Post, |mut req| {
        let len = req.content_len().unwrap_or(0) as usize;
        // aint len fixed on 132 ?
        if len > MAX_LEN {
            req.into_status_response(413)?
                .write_all("Request too big".as_bytes())?;
            return Ok(());
        }
        // check that message_on_validation !None
        let mut buf = vec![0; len];
        req.read_exact(&mut buf)?;
        let signed_message = std::str::from_utf8(&buf).expect("Cannot parse signed message");        
        let signature = Signature::from_str(&signed_message).expect("Could not parse Signature");                

        if let Some(vds) = data_validation.lock().as_deref().unwrap() {
            let rec = signature
                .recover_address_from_prehash(
                    &vds.msg
                ).expect("Could not recover address from msg");
            //info!("Recovered address {}", rec); 
            let is_valid_address = vds.account == rec;
            let mut resp = req.into_ok_response()?;
            write!(resp, "Account validation: {}", is_valid_address)?;
            //info!("Account valid: {}", is_valid_address);
            tx.send(EventType::Erase(is_valid_address)).unwrap();
        }
        Ok(())
    })?;

    let ip_info = wifi.wifi().sta_netif().get_ip_info()?;

    let mut led_val = PinDriver::output(peripherals.pins.gpio19).unwrap();
    let mut client = HttpClient::wrap(EspHttpConnection::new(&Default::default())?);
    
    loop {
        info!("Call it at: {:?}/", ip_info.ip);
        while let Ok(ev) = rx.recv() {
            //info!("{:#?}", ev);
            match ev {
                EventType::Erase(authorized) => {
                    let mut vd = data_validation.lock().unwrap();
                    if let Some(vds) = (*vd).clone() {
                        if authorized {
                            //info!("Calling safe to check account");
                            match helpers::is_safe_owner(&mut client, vds.account) {
                                Ok(is_owner) => {
                                    //info!("Is owner: {}", is_owner);
                                    if is_owner {
                                        info!("Lock OPEN");
                                        led_val.set_high()?;
                                        std::thread::sleep(std::time::Duration::from_secs(10));
                                        led_val.set_low()?;
                                        info!("Lock CLOSED");
                                    } else {
                                        for _ in 0..3 {
                                            led_val.set_high()?;
                                            std::thread::sleep(std::time::Duration::from_millis(300));
                                            led_val.set_low()?;
                                            std::thread::sleep(std::time::Duration::from_millis(300));
                                        };
                                    } 
                                },
                                Err(e) => {info!("Error: {:#?}", e)}            
                            }
                        } else {
                            info!("Invalid request to unlock");
                        }
                    }

                    *vd = None;
                },
                EventType::Load(vd) => {
                    let mut vds = data_validation.lock().unwrap();
                    *vds = Some(vd);
                }
            }
        }
    }

    #[allow(unreachable_code)]  // TODO later create break mechanisms
    Ok(())
}