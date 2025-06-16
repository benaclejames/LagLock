#[allow(dead_code)]
pub enum EgMessageType {
    Init,
    InitResponse,
    Operation,
    OperationResponse,
    Event,
    DisconnectReason,
    InternalOperationRequest,
    InternalOperationResponse,
    Message,
    RawMessage
}