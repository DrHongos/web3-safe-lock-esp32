use crate::types::*;
use crate::config;

use alloy_sol_types::SolCall;
use core::convert::TryInto;
use embedded_svc::{
    utils::io,
    http::client::Client as HttpClient,
    wifi::{Configuration, ClientConfiguration, AuthMethod},
};
use serde::{Deserialize, Serialize};
use alloy_dyn_abi::eip712::TypedData;
use alloy_sol_macro::sol;
use alloy_primitives::Address;
use log::*;
use esp_idf_svc::{
    io::Write,
    http::client::EspHttpConnection,
    wifi::{BlockingWifi, EspWifi},
};


pub fn connect_wifi(wifi: &mut BlockingWifi<EspWifi<'static>>) -> anyhow::Result<()> {
    let wifi_configuration: Configuration = Configuration::Client(ClientConfiguration {
        ssid: config::SSID.try_into().unwrap(),
        auth_method: AuthMethod::WPA2Personal,
        password: config::PASSWORD.try_into().unwrap(),
        bssid: None,
        channel: None,
    });

    wifi.set_configuration(&wifi_configuration)?;

    wifi.start()?;
    info!("Wifi started");

    wifi.connect()?;
    info!("Wifi connected");

    wifi.wait_netif_up()?;
    info!("Wifi netif up");

    Ok(())
}

pub fn typed_data_for_document(name: &str, account: &str, rand: u32) -> TypedData {
    sol! {
        #[derive(Serialize, Deserialize)]
        struct DocumentSignature {
            string name;
            string account;
            string content;
            uint32 rand;
        }
    };
    let doc = DocumentSignature {
        name: name.to_string(),
        account: account.to_string(),
        content: "Alohomora!".to_string(),
        rand,
    };
    let domain_obj: alloy_dyn_abi::Eip712Domain = alloy_sol_types::eip712_domain!(
        name: "Web3Lock",
        version: "1",
    );

    TypedData::from_struct(&doc, Some(domain_obj))
}

/// Send an HTTP POST request to infura to check if the address is owner of the specified safe.
pub fn is_safe_owner(client: &mut HttpClient<EspHttpConnection>, account: Address) -> anyhow::Result<bool> {

    sol!{
        #[derive(Serialize)]
        function isOwner(address) returns (bool);
    };
    let is_owner_call = isOwnerCall { _0: account };
    let data = is_owner_call.abi_encode();

    let payload_data = TransactionRequestSimplified {
        to: config::SAFE_ADDRESS.to_string(),
        input: data.into()
    };
    let payload = JsonRequest {
        jsonrpc: "2.0".to_string(),
        method: "eth_call".to_string(),
        params: vec![payload_data],
        id: 1
    };
    let request_data_serialized = serde_json::to_string(&payload).expect("Cannot serialize");
    let content_length_header = format!("{}", request_data_serialized.len());
    let headers = [
        ("content-type", "application/json"),
        ("content-length", &*content_length_header),
    ];

    let mut request = client.post(config::RPC_URL, &headers)?;
    request.write_all(request_data_serialized.as_bytes())?;
    request.flush()?;
    
    let mut response = request.submit()?;
    //let status = response.status();

    // i know the answer is a boolean --> FIX THIS
    let mut buf = [0u8; 1024];
    let bytes_read = io::try_read_full(&mut response, &mut buf).map_err(|e| e.0)?;
    let is_owner = match std::str::from_utf8(&buf[0..bytes_read]) {
        Ok(body_string) => {
            let bn: JsonResponse = serde_json::from_slice(body_string.as_bytes()).expect("Cannot deserialize");
            // deserialization does not seem to work, so i simply check like this
            bn.result == "0x0000000000000000000000000000000000000000000000000000000000000001"
        },
        Err(e) => {
            error!("Error decoding response body: {}", e);
            false
        },
    };
    // Drain the remaining response bytes
    while response.read(&mut buf)? > 0 {}

    Ok(is_owner)
}