use crate::gp_type::{GpType};
use crate::operation_response::OperationResponse;
use crate::parameter_dictionary::{ParameterDictionary, Value};
use crate::stream_buffer::StreamBuffer;

fn read_byte(stream: &mut StreamBuffer) -> u8 {
    stream.read_byte()
}

fn read_int16(stream: &mut StreamBuffer) -> i16 {
    let byte1 = stream.read_byte();
    let byte2 = stream.read_byte();

    // Combine the bytes to form a 16-bit integer (little-endian)
    // byte1 is the low byte, byte2 is the high byte
    (byte1 as i16) | ((byte2 as i16) << 8)
}

fn write_byte(stream: &mut StreamBuffer, value: u8, write_type: bool) {
    if write_type {
        if value == 0 {
            stream.write_gp_type(GpType::FloatZero);
            return;
        }
        stream.write_gp_type(GpType::Byte);
    }

    stream.write_byte(value);
}

fn write_ushort(stream: &mut StreamBuffer, value: u16) {
    stream.write_byte(value as u8);
    stream.write_byte((value >> 8) as u8);
}

fn encode_zigzag32(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

fn decode_zigzag32(value: u32) -> i32 {
    ((value >> 1) as i32) ^ (-((value & 1) as i32))
}

fn write_compressed_uint32(buffer: &mut [u8], value: u32) -> usize {
    let mut num = 0;
    buffer[num] = (value & 0x7F) as u8;
    let mut value = value >> 7;

    while value != 0 {
        buffer[num] |= 128;
        buffer[num + 1] = (value & 0x7F) as u8;
        num += 1;
        value >>= 7;
    }

    num + 1
}

fn write_compressed_uint32_to_stream(stream: &mut StreamBuffer, value: u32) {
    let mut mem_compressed_uint32 = [0u8; 5];

    // Using a block to simulate the lock in C#
    {
        let bytes_written = write_compressed_uint32(&mut mem_compressed_uint32, value);
        stream.write(&mem_compressed_uint32[0..bytes_written]);
    }
}

fn write_compressed_int(stream: &mut StreamBuffer, value: i32, write_type: bool) {
    if write_type {
        if value == 0 {
            stream.write_gp_type(GpType::IntZero);
            return;
        }
        if value > 0 {
            if value <= 255 {
                stream.write_gp_type(GpType::Int1);
                stream.write_byte(value as u8);
                return;
            }
            if value <= 65535 {
                stream.write_gp_type(GpType::Int2);
                write_ushort(stream, value as u16);
                return;
            }
        }
        else if value >= -65535 {
            if value >= -255 {
                stream.write_gp_type(GpType::Int2_);
                stream.write_byte((-value) as u8);
                return;
            }
            if value >= -65535 {
                stream.write_gp_type(GpType::Int2_);
                write_ushort(stream, (-value) as u16);
                return;
            }
        }
    }

    if write_type {
        stream.write_gp_type(GpType::CompressedInt);
    }

    let value2 = encode_zigzag32(value);
    write_compressed_uint32_to_stream(stream, value2);
}

fn write_int_length(stream: &mut StreamBuffer, value: usize) {
    write_compressed_uint32_to_stream(stream, value as u32);
}

fn write_string(stream: &mut StreamBuffer, value: &str, write_type: bool) {
    if write_type { 
        stream.write_gp_type(GpType::String);
    }
    
    let count = value.len();
    if count > 32767 {
        panic!("String length exceeds maximum allowed length");
    }
    write_int_length(stream, count);
    stream.write(value.as_bytes());
}

fn write(stream: &mut StreamBuffer, value: &Value, write_type: bool) {
    match value {
        Value::Int(value) => {
            write_compressed_int(stream, *value, write_type);
        }
        Value::String(value) => {
            write_string(stream, value, write_type);
        }
        _ => {panic!("Not implemented");}
    }
}

fn write_parameter_table(stream: &mut StreamBuffer, parameters: ParameterDictionary) {
    let parameters_length = parameters.iter().len() as u8;
    if parameters_length == 0 {
        write_byte(stream, 0, false);
        return;
    }

    write_byte(stream, parameters_length, false);
    for parameter in parameters.iter() {
        stream.write_byte(*parameter.0);
        write(stream, parameter.1, true);
    }
}

pub fn serialize_operation_request(stream: &mut StreamBuffer, opcode: u8, parameters: ParameterDictionary, set_type: bool) {
    if set_type {
        stream.write_gp_type(GpType::OperationRequest);
    }

    stream.write_byte(opcode);
    write_parameter_table(stream, parameters);
}

fn read_compressed_uint32(stream: &mut StreamBuffer) -> u32 {
    let mut num1: u32 = 0;
    let mut num2: i32 = 0;

    // Get a copy of the buffer and other necessary values
    let buffer = stream.get_buffer().to_vec();
    let buffer_len = buffer.len();
    let stream_length = stream.length();
    let mut position = stream.position();

    while num2 != 35 {
        if position >= stream_length {
            // Update the stream position before panicking
            stream.seek(stream_length);

            // Format the error message to match the C# implementation
            let error_msg = format!(
                "Failed to read full uint. offset: {} stream.Length: {} data.Length: {} stream.Available: {}",
                position, stream_length, buffer_len, stream_length - position
            );
            panic!("{}", error_msg);
        }

        let num4 = buffer[position];
        position += 1;
        num1 |= ((num4 & 0x7F) as u32) << num2;
        num2 += 7;

        if (num4 & 0x80) == 0 {
            break;
        }
    }

    // Update the stream position
    stream.seek(position);
    num1
}

fn read_compressed_int32(stream: &mut StreamBuffer) -> i32 {
    decode_zigzag32(read_compressed_uint32(stream))
}

fn read_string_array(stream: &mut StreamBuffer) -> Vec<String> {
    let length = read_compressed_uint32(stream) as usize;
    let mut strings = Vec::with_capacity(length);
    for _ in 0..length {
        let length = read_compressed_uint32(stream) as usize;
        let bytes = stream.read(length);
        match String::from_utf8(bytes) {
            Ok(s) => strings.push(s),
            Err(_) => panic!("Invalid UTF-8 string data"),
        }
    }
    strings
}

fn read(stream: &mut StreamBuffer, gp_type: u8) -> Value {
    if gp_type >= 128 && gp_type <= 228 {
        // Custom type
        panic!("Custom types not implemented")
    }

    match GpType::try_from(gp_type).unwrap() {
        GpType::Int1 => Value::Int(read_byte(stream) as i32),
        GpType::Byte => Value::Byte(read_byte(stream)),
        GpType::Short => Value::Int(read_int16(stream) as i32),
        GpType::String => {
            // Read string length as a compressed int
            let length = read_compressed_uint32(stream) as usize;
            if length == 0 {
                return Value::String(String::new());
            }

            // Read string data
            let bytes = stream.read(length);
            match String::from_utf8(bytes) {
                Ok(s) => Value::String(s),
                Err(_) => panic!("Invalid UTF-8 string data"),
            }
        }
        GpType::Null => Value::Null, // Null type
        GpType::CompressedInt => Value::Int(read_compressed_int32(stream)),
        GpType::IntZero => Value::Int(0),
        GpType::StringArray => Value::StringArray(read_string_array(stream)),
        _ => {panic!("Not implemented: {}", gp_type);}
    }
}

fn read_parameter_dictionary(stream: &mut StreamBuffer) -> ParameterDictionary {
    let capacity = read_byte(stream) as usize;
    let mut parameters = ParameterDictionary::with_capacity(capacity);

    for _ in 0..capacity {
        let code = stream.read_byte();
        let gp_type = stream.read_byte();
        let value = read(stream, gp_type);
        parameters.set(code, value);
    }

    parameters
}

pub fn deserialize_operation_response(stream: &mut StreamBuffer) -> OperationResponse {
    let operation_code = read_byte(stream);
    let return_code = read_int16(stream);

    // Read the debug message type first, then pass it to the read function
    let debug_message_type = read_byte(stream);
    let debug_message = match read(stream, debug_message_type) {
        Value::String(value) => Some(value),
        _ => None,
    };

    let payload = read_parameter_dictionary(stream);

    OperationResponse {
        operation_code,
        return_code,
        debug_message,
        payload
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_string() {
        let mut buffer = StreamBuffer::with_capacity(20);

        // Write a string type (7)
        buffer.write_byte(7);

        // Write string length as compressed int (5)
        buffer.write_byte(5); // Direct length, not zigzag encoded

        // Write string data "hello"
        buffer.write_byte(b'h');
        buffer.write_byte(b'e');
        buffer.write_byte(b'l');
        buffer.write_byte(b'l');
        buffer.write_byte(b'o');

        buffer.reset_position();

        // Skip the type byte as it would be read by the caller
        buffer.read_byte();

        // Read the string
        match read(&mut buffer, 7) {
            Value::String(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected String value"),
        }
    }

    #[test]
    fn test_deserialize_operation_response_with_debug_message() {
        let mut buffer = StreamBuffer::with_capacity(50);

        // Write operation code
        buffer.write_byte(1);

        // Write return code
        buffer.write_byte(0);
        buffer.write_byte(0);

        // Write debug message type (7 = String)
        buffer.write_byte(7);

        // Write debug message length as compressed int (5)
        buffer.write_byte(10); // 5 in zigzag encoding

        // Write debug message data "hello"
        buffer.write_byte(b'h');
        buffer.write_byte(b'e');
        buffer.write_byte(b'l');
        buffer.write_byte(b'l');
        buffer.write_byte(b'o');

        // Write empty parameter dictionary
        buffer.write_byte(0);

        buffer.reset_position();

        // Deserialize the operation response
        let response = deserialize_operation_response(&mut buffer);

        assert_eq!(response.operation_code, 1);
        assert_eq!(response.return_code, 0);
        assert_eq!(response.debug_message, Some("hello".to_string()));
        assert_eq!(response.payload.count(), 0);
    }

    #[test]
    fn test_deserialize_operation_response_without_debug_message() {
        let mut buffer = StreamBuffer::with_capacity(50);

        // Write operation code
        buffer.write_byte(1);

        // Write return code
        buffer.write_byte(0);
        buffer.write_byte(0);

        // Write debug message type (8 = Null)
        buffer.write_byte(8);

        // Write empty parameter dictionary
        buffer.write_byte(0);

        buffer.reset_position();

        // Deserialize the operation response
        let response = deserialize_operation_response(&mut buffer);

        assert_eq!(response.operation_code, 1);
        assert_eq!(response.return_code, 0);
        assert_eq!(response.debug_message, None);
        assert_eq!(response.payload.count(), 0);
    }

    #[test]
    fn test_write_ushort() {
        let mut buffer = StreamBuffer::with_capacity(2);
        write_ushort(&mut buffer, 0x1234);

        buffer.reset_position();
        assert_eq!(buffer.read_byte(), 0x34); // Low byte
        assert_eq!(buffer.read_byte(), 0x12); // High byte
    }

    #[test]
    fn test_read_int16() {
        let mut buffer = StreamBuffer::with_capacity(2);

        // Write a 16-bit integer (0x1234) to the buffer
        buffer.write_byte(0x34); // Low byte
        buffer.write_byte(0x12); // High byte

        buffer.reset_position();

        // Read it back using read_int16
        let value = read_int16(&mut buffer);

        // Verify that the value is read correctly
        assert_eq!(value, 0x1234);
    }

    #[test]
    fn test_encode_zigzag32() {
        // Test positive numbers
        assert_eq!(encode_zigzag32(0), 0);
        assert_eq!(encode_zigzag32(1), 2);
        assert_eq!(encode_zigzag32(2), 4);

        // Test negative numbers
        assert_eq!(encode_zigzag32(-1), 1);
        assert_eq!(encode_zigzag32(-2), 3);

        // Test larger numbers
        assert_eq!(encode_zigzag32(0x3FFFFFFF), 0x7FFFFFFE);
        assert_eq!(encode_zigzag32(-0x40000000), 0x7FFFFFFF);
    }

    #[test]
    fn test_decode_zigzag32() {
        // Test positive numbers
        assert_eq!(decode_zigzag32(0), 0);
        assert_eq!(decode_zigzag32(2), 1);
        assert_eq!(decode_zigzag32(4), 2);

        // Test negative numbers
        assert_eq!(decode_zigzag32(1), -1);
        assert_eq!(decode_zigzag32(3), -2);

        // Test larger numbers
        assert_eq!(decode_zigzag32(0x7FFFFFFE), 0x3FFFFFFF);
        assert_eq!(decode_zigzag32(0x7FFFFFFF), -0x40000000);

        // Test round-trip encoding and decoding
        let original = 12345;
        let encoded = encode_zigzag32(original);
        let decoded = decode_zigzag32(encoded);
        assert_eq!(decoded, original);

        let original = -12345;
        let encoded = encode_zigzag32(original);
        let decoded = decode_zigzag32(encoded);
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_write_compressed_uint32() {
        // Test small values (1 byte)
        let mut buffer = [0u8; 5];
        let bytes_written = write_compressed_uint32(&mut buffer, 1);
        assert_eq!(bytes_written, 1);
        assert_eq!(buffer[0], 1);

        // Test medium values (2 bytes)
        let mut buffer = [0u8; 5];
        let bytes_written = write_compressed_uint32(&mut buffer, 128);
        assert_eq!(bytes_written, 2);
        assert_eq!(buffer[0], 128 | 0);
        assert_eq!(buffer[1], 1);

        // Test larger values
        let mut buffer = [0u8; 5];
        let bytes_written = write_compressed_uint32(&mut buffer, 0x4000);
        assert_eq!(bytes_written, 3);
        assert_eq!(buffer[0], 128 | 0);
        assert_eq!(buffer[1], 128 | 0);
        assert_eq!(buffer[2], 1);
    }

    #[test]
    fn test_read_compressed_uint32() {
        // Test small values (1 byte)
        let mut buffer = StreamBuffer::with_capacity(5);
        buffer.write_byte(1); // Value 1, no continuation bit
        buffer.reset_position();
        assert_eq!(read_compressed_uint32(&mut buffer), 1);

        // Test medium values (2 bytes)
        let mut buffer = StreamBuffer::with_capacity(5);
        buffer.write_byte(128 | 0); // First byte with continuation bit
        buffer.write_byte(1);       // Second byte without continuation bit
        buffer.reset_position();
        assert_eq!(read_compressed_uint32(&mut buffer), 128);

        // Test larger values (3 bytes)
        let mut buffer = StreamBuffer::with_capacity(5);
        buffer.write_byte(128 | 0); // First byte with continuation bit
        buffer.write_byte(128 | 0); // Second byte with continuation bit
        buffer.write_byte(1);       // Third byte without continuation bit
        buffer.reset_position();
        assert_eq!(read_compressed_uint32(&mut buffer), 0x4000);
    }

    #[test]
    fn test_read_compressed_int32() {
        // Test zero
        let mut buffer = StreamBuffer::with_capacity(5);
        buffer.write_byte(0); // Encoded value for 0
        buffer.reset_position();
        assert_eq!(read_compressed_int32(&mut buffer), 0);

        // Test positive value
        let mut buffer = StreamBuffer::with_capacity(5);
        buffer.write_byte(2); // Encoded value for 1
        buffer.reset_position();
        assert_eq!(read_compressed_int32(&mut buffer), 1);

        // Test negative value
        let mut buffer = StreamBuffer::with_capacity(5);
        buffer.write_byte(1); // Encoded value for -1
        buffer.reset_position();
        assert_eq!(read_compressed_int32(&mut buffer), -1);

        // Test round-trip for a larger value
        let original = 12345;
        let mut buffer = StreamBuffer::with_capacity(10);
        write_compressed_int(&mut buffer, original, false);
        buffer.reset_position();
        assert_eq!(read_compressed_int32(&mut buffer), original);

        // Test round-trip for a negative value
        let original = -12345;
        let mut buffer = StreamBuffer::with_capacity(10);
        write_compressed_int(&mut buffer, original, false);
        buffer.reset_position();
        assert_eq!(read_compressed_int32(&mut buffer), original);
    }

    #[test]
    fn test_write_compressed_int() {
        // Test zero
        let mut buffer = StreamBuffer::with_capacity(10);
        write_compressed_int(&mut buffer, 0, true);
        buffer.reset_position();
        assert_eq!(buffer.read_byte(), 30); // IntZero

        // Test small positive value
        let mut buffer = StreamBuffer::with_capacity(10);
        write_compressed_int(&mut buffer, 42, true);
        buffer.reset_position();
        assert_eq!(buffer.read_byte(), 11); // Int1
        assert_eq!(buffer.read_byte(), 42);

        // Test small negative value
        let mut buffer = StreamBuffer::with_capacity(10);
        write_compressed_int(&mut buffer, -42, true);
        buffer.reset_position();
        assert_eq!(buffer.read_byte(), 12); // Int1_
        assert_eq!(buffer.read_byte(), 42);

        // Test medium positive value
        let mut buffer = StreamBuffer::with_capacity(10);
        let test_value = 1000u16;
        write_compressed_int(&mut buffer, test_value as i32, true);
        buffer.reset_position();
        assert_eq!(buffer.read_byte(), 13); // Int2
        assert_eq!(buffer.read_byte(), (test_value & 0xFF) as u8);
        assert_eq!(buffer.read_byte(), ((test_value >> 8) & 0xFF) as u8);

        // Test medium negative value
        let mut buffer = StreamBuffer::with_capacity(10);
        let test_value = 1000u16;
        write_compressed_int(&mut buffer, -(test_value as i32), true);
        buffer.reset_position();
        assert_eq!(buffer.read_byte(), 14); // Int2_
        assert_eq!(buffer.read_byte(), (test_value & 0xFF) as u8);
        assert_eq!(buffer.read_byte(), ((test_value >> 8) & 0xFF) as u8);

        // Test large value (compressed)
        let mut buffer = StreamBuffer::with_capacity(10);
        write_compressed_int(&mut buffer, 1000000, true);
        buffer.reset_position();
        assert_eq!(buffer.read_byte(), 9); // CompressedInt
        // We don't check the compressed bytes here as that's tested separately
    }
}
