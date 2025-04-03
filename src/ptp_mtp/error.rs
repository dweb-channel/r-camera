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

    /// USB 相关错误
    Usb(libusb::Error),

    /// IO 操作错误
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Response(r) => write!(f, "{} (0x{:04x})", crate::ptp_mtp::standard_codes::StandardResponseCode::name(r).unwrap_or("未知错误"), r),
            Error::Usb(ref e) => write!(f, "USB 错误: {}", e),
            Error::Io(ref e) => write!(f, "IO 错误: {}", e),
            Error::Malformed(ref e) => write!(f, "{}", e),
        }
    }
}

impl ::std::error::Error for Error {
    fn cause(&self) -> Option<& dyn ::std::error::Error> {
        match *self {
            Error::Usb(ref e) => Some(e),
            Error::Io(ref e) => Some(e),
            _ => None,
        }
    }
}

impl From<libusb::Error> for Error {
    fn from(e: libusb::Error) -> Error {
        Error::Usb(e)
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
