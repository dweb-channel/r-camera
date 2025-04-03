#![allow(non_snake_case)]

use std::io::{self, Cursor};
use std::io::prelude::*;
use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};

use crate::ptp_mtp::error::Error;

/// PTP数据读取特性
pub trait PtpRead: ReadBytesExt {
    // 基本类型读取方法
    fn read_ptp_u8(&mut self) -> Result<u8, Error> {
        Ok(self.read_u8()?)
    }

    fn read_ptp_i8(&mut self) -> Result<i8, Error> {
        Ok(self.read_i8()?)
    }

    fn read_ptp_u16(&mut self) -> Result<u16, Error> {
        Ok(self.read_u16::<LittleEndian>()?)
    }

    fn read_ptp_i16(&mut self) -> Result<i16, Error> {
        Ok(self.read_i16::<LittleEndian>()?)
    }

    fn read_ptp_u32(&mut self) -> Result<u32, Error> {
        Ok(self.read_u32::<LittleEndian>()?)
    }

    fn read_ptp_i32(&mut self) -> Result<i32, Error> {
        Ok(self.read_i32::<LittleEndian>()?)
    }

    fn read_ptp_u64(&mut self) -> Result<u64, Error> {
        Ok(self.read_u64::<LittleEndian>()?)
    }

    fn read_ptp_i64(&mut self) -> Result<i64, Error> {
        Ok(self.read_i64::<LittleEndian>()?)
    }

    fn read_ptp_u128(&mut self) -> Result<(u64, u64), Error> {
        let hi = self.read_u64::<LittleEndian>()?;
        let lo = self.read_u64::<LittleEndian>()?;
        Ok((lo, hi))
    }

    fn read_ptp_i128(&mut self) -> Result<(u64, u64), Error> {
        let hi = self.read_u64::<LittleEndian>()?;
        let lo = self.read_u64::<LittleEndian>()?;
        Ok((lo, hi))
    }

    /// 读取向量数据的辅助方法
    #[inline(always)]
    fn read_ptp_vec<T: Sized, U: Fn(&mut Self) -> Result<T, Error>>(&mut self,
                                                                 func: U)
                                                                 -> Result<Vec<T>, Error> {
        let len = self.read_u32::<LittleEndian>()? as usize;
        (0..len).map(|_| func(self)).collect()
    }

    // 各种类型向量的读取方法
    fn read_ptp_u8_vec(&mut self) -> Result<Vec<u8>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_u8())
    }

    fn read_ptp_i8_vec(&mut self) -> Result<Vec<i8>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_i8())
    }

    fn read_ptp_u16_vec(&mut self) -> Result<Vec<u16>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_u16())
    }

    fn read_ptp_i16_vec(&mut self) -> Result<Vec<i16>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_i16())
    }

    fn read_ptp_u32_vec(&mut self) -> Result<Vec<u32>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_u32())
    }

    fn read_ptp_i32_vec(&mut self) -> Result<Vec<i32>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_i32())
    }

    fn read_ptp_u64_vec(&mut self) -> Result<Vec<u64>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_u64())
    }

    fn read_ptp_i64_vec(&mut self) -> Result<Vec<i64>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_i64())
    }

    fn read_ptp_u128_vec(&mut self) -> Result<Vec<(u64, u64)>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_u128())
    }

    fn read_ptp_i128_vec(&mut self) -> Result<Vec<(u64, u64)>, Error> {
        self.read_ptp_vec(|cur| cur.read_ptp_i128())
    }

    /// 读取PTP字符串
    fn read_ptp_str(&mut self) -> Result<String, Error> {
        let len = self.read_u8()?;
        if len > 0 {
            // len包括结尾的null字符(u16)
            let data: Vec<u16> = (0..(len - 1)).map(|_| self.read_u16::<LittleEndian>()).collect::<Result<_, std::io::Error>>()?;
            self.read_u16::<LittleEndian>()?; // 读取结尾的null
            String::from_utf16(&data).map_err(|_| Error::Malformed(format!("无效的UTF16数据: {:?}", data)))
        } else {
            Ok("".into())
        }
    }

    /// 检查是否已读取到数据流末尾
    fn expect_end(&mut self) -> Result<(), Error>;
}

/// 为Cursor实现PtpRead特性
impl<T: AsRef<[u8]>> PtpRead for Cursor<T> {
    fn expect_end(&mut self) -> Result<(), Error> {
        let len = self.get_ref().as_ref().len();
        if len as u64 != self.position() {
            Err(Error::Malformed(format!("响应长度为{}字节，预期{}字节", len, self.position())))
        } else {
            Ok(())
        }
    }
}

/// PTP数据类型枚举
#[allow(non_snake_case)]
#[derive(Debug, PartialEq)]
pub enum PtpDataType {
    UNDEF,
    INT8(i8),
    UINT8(u8),
    INT16(i16),
    UINT16(u16),
    INT32(i32),
    UINT32(u32),
    INT64(i64),
    UINT64(u64),
    INT128((u64, u64)),
    UINT128((u64, u64)),
    AINT8(Vec<i8>),
    AUINT8(Vec<u8>),
    AINT16(Vec<i16>),
    AUINT16(Vec<u16>),
    AINT32(Vec<i32>),
    AUINT32(Vec<u32>),
    AINT64(Vec<i64>),
    AUINT64(Vec<u64>),
    AINT128(Vec<(u64, u64)>),
    AUINT128(Vec<(u64, u64)>),
    STR(String),
}

impl PtpDataType {
    /// 将PTP数据类型编码为字节序列
    pub fn encode(&self) -> Vec<u8> {
        use self::PtpDataType::*;
        let mut out = vec![];
        match self {
            // UNDEF => {},
            &INT8(val) => {
                out.write_i8(val).ok();
            }
            &UINT8(val) => {
                out.write_u8(val).ok();
            }
            &INT16(val) => {
                out.write_i16::<LittleEndian>(val).ok();
            }
            &UINT16(val) => {
                out.write_u16::<LittleEndian>(val).ok();
            }
            &INT32(val) => {
                out.write_i32::<LittleEndian>(val).ok();
            }
            &UINT32(val) => {
                out.write_u32::<LittleEndian>(val).ok();
            }
            &INT64(val) => {
                out.write_i64::<LittleEndian>(val).ok();
            }
            &UINT64(val) => {
                out.write_u64::<LittleEndian>(val).ok();
            }
            &INT128((hi, lo)) => {
                out.write_u64::<LittleEndian>(lo).ok();
                out.write_u64::<LittleEndian>(hi).ok();
            }
            &UINT128((hi, lo)) => {
                out.write_u64::<LittleEndian>(lo).ok();
                out.write_u64::<LittleEndian>(hi).ok();
            }
            &AINT8(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_i8(*item).ok();
                }
            }
            &AUINT8(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_u8(*item).ok();
                }
            }
            &AINT16(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_i16::<LittleEndian>(*item).ok();
                }
            }
            &AUINT16(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_u16::<LittleEndian>(*item).ok();
                }
            }
            &AINT32(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_i32::<LittleEndian>(*item).ok();
                }
            }
            &AUINT32(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_u32::<LittleEndian>(*item).ok();
                }
            }
            &AINT64(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_i64::<LittleEndian>(*item).ok();
                }
            }
            &AUINT64(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for item in val {
                    out.write_u64::<LittleEndian>(*item).ok();
                }
            }
            &AINT128(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for &(hi, lo) in val {
                    out.write_u64::<LittleEndian>(lo).ok();
                    out.write_u64::<LittleEndian>(hi).ok();
                }
            }
            &AUINT128(ref val) => {
                out.write_u32::<LittleEndian>(val.len() as u32).ok();
                for &(hi, lo) in val {
                    out.write_u64::<LittleEndian>(lo).ok();
                    out.write_u64::<LittleEndian>(hi).ok();
                }
            }
            &STR(ref val) => {
                out.write_u8(((val.len() as u8) * 2) + 1).ok();
                if val.len() > 0 {
                    for e in val.encode_utf16() { out.write_u16::<LittleEndian>(e).ok(); }
                    out.write_all(b"\0\0").ok(); // 写入UTF-16结尾的null
                }
            }
            _ => {}
        }
        out
    }

    /// 根据类型ID从数据流中读取PTP数据类型
    pub fn read_type<T: PtpRead>(kind: u16, reader: &mut T) -> Result<PtpDataType, Error> {
        use self::PtpDataType::*;
        Ok(match kind {
            // 0x0000 => UNDEF,
            0x0001 => INT8(reader.read_ptp_i8()?),
            0x0002 => UINT8(reader.read_ptp_u8()?),
            0x0003 => INT16(reader.read_ptp_i16()?),
            0x0004 => UINT16(reader.read_ptp_u16()?),
            0x0005 => INT32(reader.read_ptp_i32()?),
            0x0006 => UINT32(reader.read_ptp_u32()?),
            0x0007 => INT64(reader.read_ptp_i64()?),
            0x0008 => UINT64(reader.read_ptp_u64()?),
            0x0009 => INT128(reader.read_ptp_i128()?),
            0x000A => UINT128(reader.read_ptp_u128()?),
            0x4001 => AINT8(reader.read_ptp_i8_vec()?),
            0x4002 => AUINT8(reader.read_ptp_u8_vec()?),
            0x4003 => AINT16(reader.read_ptp_i16_vec()?),
            0x4004 => AUINT16(reader.read_ptp_u16_vec()?),
            0x4005 => AINT32(reader.read_ptp_i32_vec()?),
            0x4006 => AUINT32(reader.read_ptp_u32_vec()?),
            0x4007 => AINT64(reader.read_ptp_i64_vec()?),
            0x4008 => AUINT64(reader.read_ptp_u64_vec()?),
            0x4009 => AINT128(reader.read_ptp_i128_vec()?),
            0x400A => AUINT128(reader.read_ptp_u128_vec()?),
            0xFFFF => STR(reader.read_ptp_str()?),
            _ => UNDEF,
        })
    }
}

// 从基本类型转换为PtpDataType的实现
impl<'a> From<i8> for PtpDataType {
    fn from(value: i8) -> Self {
        PtpDataType::INT8(value)
    }
}

impl<'a> From<u8> for PtpDataType {
    fn from(value: u8) -> Self {
        PtpDataType::UINT8(value)
    }
}

impl<'a> From<i16> for PtpDataType {
    fn from(value: i16) -> Self {
        PtpDataType::INT16(value)
    }
}

impl<'a> From<u16> for PtpDataType {
    fn from(value: u16) -> Self {
        PtpDataType::UINT16(value)
    }
}

impl<'a> From<i32> for PtpDataType {
    fn from(value: i32) -> Self {
        PtpDataType::INT32(value)
    }
}

impl<'a> From<u32> for PtpDataType {
    fn from(value: u32) -> Self {
        PtpDataType::UINT32(value)
    }
}

impl<'a> From<i64> for PtpDataType {
    fn from(value: i64) -> Self {
        PtpDataType::INT64(value)
    }
}

impl<'a> From<u64> for PtpDataType {
    fn from(value: u64) -> Self {
        PtpDataType::UINT64(value)
    }
}

impl<'a> From<&'a str> for PtpDataType {
    fn from(value: &'a str) -> Self {
        PtpDataType::STR(value.to_owned())
    }
}

impl<'a> From<String> for PtpDataType {
    fn from(value: String) -> Self {
        PtpDataType::STR(value)
    }
}
