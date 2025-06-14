use crate::parameter_dictionary::ParameterDictionary;

#[derive(Debug)]
pub struct OperationResponse { 
    pub operation_code: u8,
    pub return_code: i16,
    pub debug_message: Option<String>,
    pub payload: ParameterDictionary
}