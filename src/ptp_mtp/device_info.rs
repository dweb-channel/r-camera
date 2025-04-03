#![allow(non_snake_case)]

use std::io::Cursor;
use crate::ptp_mtp::error::Error;
use crate::ptp_mtp::data_types::PtpRead;

/// PTP设备信息结构体
#[allow(non_snake_case)]
#[derive(Debug)]
pub struct PtpDeviceInfo {
    pub Version: u16,                             // PTP版本号
    pub VendorExID: u32,                          // 厂商扩展ID
    pub VendorExVersion: u16,                     // 厂商扩展版本
    pub VendorExtensionDesc: String,              // 厂商扩展描述
    pub FunctionalMode: u16,                      // 功能模式
    pub OperationsSupported: Vec<u16>,            // 支持的操作列表
    pub EventsSupported: Vec<u16>,                // 支持的事件列表
    pub DevicePropertiesSupported: Vec<u16>,      // 支持的设备属性列表
    pub CaptureFormats: Vec<u16>,                 // 支持的捕获格式列表
    pub ImageFormats: Vec<u16>,                   // 支持的图像格式列表
    pub Manufacturer: String,                     // 制造商
    pub Model: String,                            // 型号
    pub DeviceVersion: String,                    // 设备版本
    pub SerialNumber: String,                     // 序列号
}

impl PtpDeviceInfo {
    /// 从字节缓冲区解码PTP设备信息
    pub fn decode(buf: &[u8]) -> Result<PtpDeviceInfo, Error> {
        let mut cur = Cursor::new(buf);

        Ok(PtpDeviceInfo {
            Version: cur.read_ptp_u16()?,
            VendorExID: cur.read_ptp_u32()?,
            VendorExVersion: cur.read_ptp_u16()?,
            VendorExtensionDesc: cur.read_ptp_str()?,
            FunctionalMode: cur.read_ptp_u16()?,
            OperationsSupported: cur.read_ptp_u16_vec()?,
            EventsSupported: cur.read_ptp_u16_vec()?,
            DevicePropertiesSupported: cur.read_ptp_u16_vec()?,
            CaptureFormats: cur.read_ptp_u16_vec()?,
            ImageFormats: cur.read_ptp_u16_vec()?,
            Manufacturer: cur.read_ptp_str()?,
            Model: cur.read_ptp_str()?,
            DeviceVersion: cur.read_ptp_str()?,
            SerialNumber: cur.read_ptp_str()?,
        })
    }
}

/// PTP对象信息结构体
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PtpObjectInfo {
    pub StorageID: u32,               // 存储ID
    pub ObjectFormat: u16,            // 对象格式
    pub ProtectionStatus: u16,        // 保护状态
    pub ObjectCompressedSize: u32,    // 对象压缩后大小
    pub ThumbFormat: u16,             // 缩略图格式
    pub ThumbCompressedSize: u32,     // 缩略图压缩后大小
    pub ThumbPixWidth: u32,           // 缩略图宽度（像素）
    pub ThumbPixHeight: u32,          // 缩略图高度（像素） 
    pub ImagePixWidth: u32,           // 图像宽度（像素）
    pub ImagePixHeight: u32,          // 图像高度（像素）
    pub ImageBitDepth: u32,           // 图像位深度
    pub ParentObject: u32,            // 父对象句柄
    pub AssociationType: u16,         // 关联类型
    pub AssociationDesc: u32,         // 关联描述
    pub SequenceNumber: u32,          // 序列号
    pub Filename: String,             // 文件名
    pub CaptureDate: String,          // 捕获日期
    pub ModificationDate: String,     // 修改日期
    pub Keywords: String,             // 关键词
}

impl PtpObjectInfo {
    /// 从字节缓冲区解码PTP对象信息
    pub fn decode(buf: &[u8]) -> Result<PtpObjectInfo, Error> {
        let mut cur = Cursor::new(buf);

        Ok(PtpObjectInfo {
            StorageID: cur.read_ptp_u32()?,
            ObjectFormat: cur.read_ptp_u16()?,
            ProtectionStatus: cur.read_ptp_u16()?,
            ObjectCompressedSize: cur.read_ptp_u32()?,
            ThumbFormat: cur.read_ptp_u16()?,
            ThumbCompressedSize: cur.read_ptp_u32()?,
            ThumbPixWidth: cur.read_ptp_u32()?,
            ThumbPixHeight: cur.read_ptp_u32()?,
            ImagePixWidth: cur.read_ptp_u32()?,
            ImagePixHeight: cur.read_ptp_u32()?,
            ImageBitDepth: cur.read_ptp_u32()?,
            ParentObject: cur.read_ptp_u32()?,
            AssociationType: cur.read_ptp_u16()?,
            AssociationDesc: cur.read_ptp_u32()?,
            SequenceNumber: cur.read_ptp_u32()?,
            Filename: cur.read_ptp_str()?,
            CaptureDate: cur.read_ptp_str()?,
            ModificationDate: cur.read_ptp_str()?,
            Keywords: cur.read_ptp_str()?,
        })
    }
}

/// PTP存储信息结构体
#[allow(non_snake_case)]
#[derive(Debug)]
pub struct PtpStorageInfo {
    pub StorageType: u16,         // 存储类型 
    pub FilesystemType: u16,      // 文件系统类型
    pub AccessCapability: u16,    // 访问能力
    pub MaxCapacity: u64,         // 最大容量（字节）
    pub FreeSpaceInBytes: u64,    // 可用空间（字节）
    pub FreeSpaceInImages: u32,   // 可存储图像数量
    pub StorageDescription: String, // 存储描述
    pub VolumeLabel: String,      // 卷标
}

impl PtpStorageInfo {
    /// 从数据流中解码PTP存储信息
    pub fn decode<T: PtpRead>(cur: &mut T) -> Result<PtpStorageInfo, Error> {
        Ok(PtpStorageInfo {
            StorageType: cur.read_ptp_u16()?,
            FilesystemType: cur.read_ptp_u16()?,
            AccessCapability: cur.read_ptp_u16()?,
            MaxCapacity: cur.read_ptp_u64()?,
            FreeSpaceInBytes: cur.read_ptp_u64()?,
            FreeSpaceInImages: cur.read_ptp_u32()?,
            StorageDescription: cur.read_ptp_str()?,
            VolumeLabel: cur.read_ptp_str()?,
        })
    }
}

/// PTP属性表单数据枚举
#[allow(non_snake_case)]
#[derive(Debug)]
pub enum PtpFormData {
    None,                         // 无表单数据
    Range {                       // 范围类型
        minValue: crate::ptp_mtp::data_types::PtpDataType,  // 最小值
        maxValue: crate::ptp_mtp::data_types::PtpDataType,  // 最大值
        step: crate::ptp_mtp::data_types::PtpDataType,      // 步长
    },
    Enumeration {                 // 枚举类型
        array: Vec<crate::ptp_mtp::data_types::PtpDataType>, // 可选值数组
    },
}

/// PTP属性信息结构体
#[allow(non_snake_case)]
#[derive(Debug)]
pub struct PtpPropInfo {
    pub PropertyCode: u16,                        // 属性代码
    pub DataType: u16,                            // 数据类型
    pub GetSet: u8,                               // 读写权限（1=只读，2=读写）
    pub IsEnable: u8,                             // 是否启用
    pub FactoryDefault: crate::ptp_mtp::data_types::PtpDataType, // 出厂默认值
    pub Current: crate::ptp_mtp::data_types::PtpDataType,        // 当前值
    pub Form: PtpFormData,                        // 表单数据
}

impl PtpPropInfo {
    /// 从数据流中解码PTP属性信息
    pub fn decode<T: PtpRead>(cur: &mut T) -> Result<PtpPropInfo, Error> {
        use crate::ptp_mtp::data_types::PtpDataType;
        use byteorder::{ReadBytesExt, LittleEndian};
        
        let data_type;
        Ok(PtpPropInfo {
            PropertyCode: cur.read_u16::<LittleEndian>()?,
            DataType: {
                data_type = cur.read_u16::<LittleEndian>()?;
                data_type
            },
            GetSet: cur.read_u8()?,
            IsEnable: cur.read_u8()?,
            FactoryDefault: PtpDataType::read_type(data_type, cur)?,
            Current: PtpDataType::read_type(data_type, cur)?,
            Form: {
                match cur.read_u8()? {
                    // 0x00 => PtpFormData::None,
                    0x01 => {
                        PtpFormData::Range {
                            minValue: PtpDataType::read_type(data_type, cur)?,
                            maxValue: PtpDataType::read_type(data_type, cur)?,
                            step: PtpDataType::read_type(data_type, cur)?,
                        }
                    }
                    0x02 => {
                        PtpFormData::Enumeration {
                            array: {
                                let len = cur.read_u16::<LittleEndian>()? as usize;
                                let mut arr = Vec::with_capacity(len);
                                for _ in 0..len {
                                    arr.push(PtpDataType::read_type(data_type, cur)?);
                                }
                                arr
                            },
                        }
                    }
                    _ => PtpFormData::None,
                }
            },
        })
    }
}

/// PTP对象树结构体
#[derive(Debug, Clone)]
pub struct PtpObjectTree {
    pub handle: u32,                  // 对象句柄
    pub info: PtpObjectInfo,          // 对象信息
    pub children: Option<Vec<PtpObjectTree>>, // 子对象
}

impl PtpObjectTree {
    /// 遍历对象树，返回所有对象的路径和对象信息
    pub fn walk(&self) -> Vec<(String, PtpObjectTree)> {
        let mut input = vec![("".to_owned(), self.clone())];
        let mut output = vec![];

        while !input.is_empty() {
            for (prefix, item) in input.split_off(0) {
                let path = prefix.clone() +
                           (if prefix.is_empty() {
                    ""
                } else {
                    "/"
                }) + &item.info.Filename;

                output.push((path.clone(), item.clone()));

                if let Some(children) = item.children {
                    input.extend(children.into_iter().map(|x| (path.clone(), x)));
                }
            }
        }

        output
    }
}
