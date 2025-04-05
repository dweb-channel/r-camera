#![allow(non_snake_case)]

use std::fmt;
use std::io;

/// PTP 命令错误类型
#[derive(Debug)]
pub enum Error {
    /// PTP 设备返回非 Ok 的状态码，可能是标准响应码或厂商定义的代码
    Response(u16),

    /// 收到的数据格式错误
    Malformed(String),

    /// USB 相关错误(通用错误描述)
    USB(String),
    
    /// 在查找或使用资源时出错
    NotFound(String),

    /// IO 操作错误
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Response(r) => write!(f, "{} (0x{:04x})", crate::ptp_mtp::standard_codes::StandardResponseCode::name(*r).unwrap_or("未知错误"), r),
            Error::USB(e) => write!(f, "USB 错误: {}", e),
            Error::Io(e) => write!(f, "IO 错误: {}", e),
            Error::Malformed(e) => write!(f, "{}", e),
            Error::NotFound(e) => write!(f, "未找到: {}", e),
        }
    }
}

impl ::std::error::Error for Error {
    fn source(&self) -> Option<&(dyn ::std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

// 从字符串创建USB错误
impl From<String> for Error {
    fn from(e: String) -> Error {
        Error::USB(e)
    }
}

// 从&str创建USB错误
impl From<&str> for Error {
    fn from(e: &str) -> Error {
        Error::USB(e.to_string())
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        match e.kind() {
            io::ErrorKind::UnexpectedEof => Error::Malformed(format!("意外的消息结束")),
            _ => Error::Io(e),
        }
    }
}
