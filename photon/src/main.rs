use std::net::TcpStream;
use std::string::ToString;
use std::time::Instant;
use websocket::{ClientBuilder, OwnedMessage};
use websocket::sync::{Writer};
use once_cell::sync::Lazy;
use crate::message_type::EgMessageType;
use crate::parameter_codes::{ADDRESS, REGION};
use crate::parameter_dictionary::{ParameterDictionary, Value};
use crate::parameter_dictionary::Value::Int;
use crate::pinger::Pinger;
use crate::protocol_v18::{deserialize_operation_response, serialize_operation_request};
use crate::stream_buffer::StreamBuffer;

mod protocol_v18;
mod stream_buffer;
mod parameter_dictionary;
mod photon_codes;
mod message_type;
mod operation_response;
mod parameter_codes;
mod pinger;
mod gp_type;

const MESSAGE_HEADER: [u8; 2] = [243, 2];
static START_TIME: Lazy<Instant> = Lazy::new(Instant::now);
const APP_ID: &str = "0d501af7-d643-47dd-811a-cfc25ef543be";

#[inline]
fn millis_since_start() -> u64 {
    START_TIME.elapsed().as_millis() as u64
}

fn serialize_operation_to_message(opcode: u8, param_dict: ParameterDictionary, message_type: EgMessageType) -> Vec<u8> {
    let mut buffer = StreamBuffer::with_capacity(0);
    buffer.write(&MESSAGE_HEADER);
    serialize_operation_request(&mut buffer, opcode, param_dict, false);

    // Now, for some reason, photon just decides that we need to replace the second byte
    let buffer_ref = buffer.get_buffer();
    let mut raw_buffer = Vec::with_capacity(buffer.length());
    raw_buffer.extend_from_slice(&buffer_ref[0..buffer.length()]);

    // Check if we need to replace the second byte
    if raw_buffer[0] != EgMessageType::Operation as u8 {
        raw_buffer[MESSAGE_HEADER.len() - 1] = message_type as u8;
    }

    raw_buffer
}

fn init_callback() -> Vec<u8> {
    // AKA SendPing
    println!("Initializing callback");
    let mut ping_param_dict = ParameterDictionary::new();
    ping_param_dict.set(1, Int(millis_since_start() as i32));

    serialize_operation_to_message(photon_codes::PING, ping_param_dict, EgMessageType::InternalOperationRequest)
}

fn read_ping_result(operation_response: &operation_response::OperationResponse) {
    let server_timestamp = match operation_response.payload.get(2) {
        Some(Int(num)) => num,
        _ => {
            println!("No ping result received");
            return
        }
    };
    let last_timestamp =  match operation_response.payload.get(1) {
        Some(Int(num)) => *num as u64,
        _ => {
            println!("No ping result received");
            return
        }
    };

    let last_round_trip_time = millis_since_start().saturating_sub(last_timestamp);
    println!("Ping result: {}ms. Server timestamp: {}", last_round_trip_time, server_timestamp);
}

fn get_regions() -> Vec<u8> {
    let mut parameters = ParameterDictionary::new();
    parameters.set(224, Value::String(APP_ID.to_string()));
    serialize_operation_to_message(220, parameters, EgMessageType::Operation)
}

fn deserialize_message_and_callback(stream: &mut StreamBuffer, sender: &mut Writer<TcpStream>) {
    let b = stream.read_byte();
    if b != 243 && b != 253 {
        // No regular operation UDP message
        return;
    }

    let b2 = stream.read_byte();
    let b3 = b2 & 0x7F;
    let flag = (b2 & 0x80) > 0;

    // Handle encryption
    if b3 != 1 {
        if flag {
            // Throw as we have no implementation of decryption
            panic!("Decryption not implemented.")
        }
        else {
            stream.seek(2);
        }
    }

    // Parse operation response type
    match b3 {
        1 => {
            // Initial Callback
            sender.send_message(&OwnedMessage::Binary(init_callback())).unwrap();
            sender.send_message(&OwnedMessage::Binary(get_regions())).unwrap();
        }
        7 => {
            // Operation response
            let operation_response = deserialize_operation_response(stream);
            println!("Operation Response: {:?}", operation_response);
            match operation_response.operation_code {
                photon_codes::PING => {
                    read_ping_result(&operation_response);
                }
                _ => {}
            }
        }
        3 => {
            let op_res = protocol_v18::deserialize_operation_response(stream);
            if op_res.return_code != 0 {
                println!("Operation failed: {:?}", op_res);
                return;           
            }
            match op_res.operation_code {
                220 => {
                    let regions = match op_res.payload.get(REGION) {
                        Some(Value::StringArray(regions)) => regions,
                        _ => {
                            println!("No regions received");
                            return
                        }       
                    };
                    let addresses = match op_res.payload.get(ADDRESS) {
                        Some(Value::StringArray(addresses)) => addresses,
                        _ => {
                            println!("No addresses received");
                            return
                        }       
                    };
                    let intended_region = "us";
                    let region_index = regions.iter().position(|region| region == intended_region).unwrap();
                    let mut pinger = Pinger::new(&addresses[region_index], 5055, &regions[region_index]);
                    let results = pinger.start_ping(10);
                    println!("Regions");
                }
                _ => {}           
            }
            println!("Operation Response: {:?}", op_res);
        }
        5 => {
            // Disconnect
            println!("Disconnect");
        }
        _ => {panic!("Unknown operation response type")}
    }
}

fn main() {
    // Open a websocket to ws://ns.photonengine.io:80 with a subprotocol with name "GpBinaryV18"
    let client = ClientBuilder::new("wss://ns.photonengine.io:80")
        .unwrap()
        .add_protocol("GpBinaryV18")
        .connect_insecure()
        .unwrap();

    // Split the client into a sender and receiver
    let (mut receiver, mut sender) = client.split().unwrap();

    println!("Connected to Photon server. Waiting for messages...");

    // Read messages from the websocket connection
    for message in receiver.incoming_messages() {
        match message {
            Ok(msg) => {
                match msg {
                    OwnedMessage::Binary(data) => {
                        println!("Received binary message of {} bytes", data.len());

                        let mut buffer = StreamBuffer::with_capacity(data.len());

                        // Write the received data into the buffer
                        buffer.write(&data);

                        // Reset the buffer position to read from the beginning
                        buffer.reset_position();

                        deserialize_message_and_callback(&mut buffer, &mut sender)
                    },
                    OwnedMessage::Close(_) => {
                        println!("Connection closed");
                        break;
                    },
                    _ => {
                        println!("Received non-binary message: {:?}", msg);
                    }
                }
            },
            Err(e) => {
                println!("Error receiving message: {:?}", e);
                break;
            }
        }
    }

    println!("Disconnected from Photon server");
}
