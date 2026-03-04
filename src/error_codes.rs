// JSON-RPC 2.0 标准错误码
#[allow(dead_code)]
pub const PARSE_ERROR: i32 = -32700;
pub const INVALID_REQUEST: i32 = -32600;
pub const METHOD_NOT_FOUND: i32 = -32601;
pub const INVALID_PARAMS: i32 = -32602;
pub const INTERNAL_ERROR: i32 = -32603;

// 自定义错误码
pub const SESSION_NOT_FOUND: i32 = -32001;
pub const SEARCH_ERROR: i32 = -32000;
