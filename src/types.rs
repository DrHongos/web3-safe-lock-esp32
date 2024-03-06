use alloy_primitives::{Bytes, FixedBytes, Address};
use serde::{Serialize, Deserialize};

#[derive(Deserialize)]
pub struct FormData<'a> {
    pub name: &'a str,
    pub account: &'a str,
}

#[derive(Debug, Clone)]
pub struct VerifyData {
    pub account: Address,
    pub msg: FixedBytes<32>,
    pub rand: u32,
}

#[derive(Debug)]
pub enum EventType {
    Load(VerifyData),
    Erase(bool)
}

// a simplified version of TransactionRequest to read a contract with an eth_call() request
#[derive(Debug, Serialize)]
pub struct TransactionRequestSimplified {
    pub to: String,
    pub input: Bytes,
}

// Higher level struct to query
#[derive(Debug, Serialize)]
pub struct JsonRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<TransactionRequestSimplified>,
    pub id: usize,
}

// TODO: create the error one.. (changes result for error (is a struct))
#[derive(Debug, Deserialize)]
pub struct JsonResponse<'a> {
    pub jsonrpc: &'a str,
    pub result: &'a str,
    pub id: u64,
}