#![allow(non_snake_case)]

// 子模块
mod error;
mod standard_codes;
mod data_types;
mod device_info;
mod camera;

// 重导出所有公共项
pub use error::Error;
pub use standard_codes::{
    PtpContainerType,
    StandardResponseCode, 
    StandardCommandCode,
    CommandCode,
    ResponseCode
};
pub use data_types::{PtpRead, PtpDataType};
pub use device_info::{
    PtpDeviceInfo, 
    PtpObjectInfo, 
    PtpStorageInfo, 
    PtpFormData,
    PtpPropInfo, 
    PtpObjectTree
};
pub use camera::PtpCamera;

// 导入必要的依赖
use log::{error, debug};
use std::error::Error as StdError;
use libusb;
use std::time::SystemTime;

/// 支持的传输协议类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProtocolType {
    PTP,
    MTP,
}

/// 协议处理器特性
pub trait ProtocolHandler {
    /// 初始化协议会话
    fn init_session(&mut self) -> Result<(), Box<dyn StdError>>;
    
    /// 获取设备信息
    fn get_device_info(&self) -> Result<DeviceInfo, Box<dyn StdError>>;
    
    /// 开始实时数据流传输
    fn start_live_stream(&mut self) -> Result<(), Box<dyn StdError>>;
    
    /// 停止实时数据流传输
    fn stop_live_stream(&mut self) -> Result<(), Box<dyn StdError>>;
    
    /// 关闭会话
    fn close_session(&mut self) -> Result<(), Box<dyn StdError>>;
}

/// 设备信息结构
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_name: String,       // 设备名称
    pub manufacturer: String,      // 制造商
    pub model: String,             // 型号
    pub serial_number: String,     // 序列号
    pub protocol_version: String,  // 协议版本
    pub supported_operations: Vec<u16>, // 支持的操作
}

/// 传输数据包
#[derive(Debug, Clone)]
pub struct DataPacket {
    pub data: Vec<u8>,            // 数据内容
    pub timestamp: SystemTime,    // 时间戳
    pub packet_type: PacketType,  // 数据包类型
}

/// 数据包类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PacketType {
    Image,      // 图像数据
    Thumbnail,  // 缩略图数据
    Metadata,   // 元数据
    Command,    // 命令
    Response,   // 响应
}

/// 创建协议处理器
/// 注意: 此函数目前需要更新实现
pub fn create_protocol_handler(_protocol_type: ProtocolType, _device_handle: &libusb::DeviceHandle) -> Box<dyn ProtocolHandler> {
    // 暂时使用模拟实现，实际应根据PTP或MTP创建相应的处理器
    Box::new(MockProtocolHandler {})
}

// 临时的模拟处理器实现
struct MockProtocolHandler {}

impl ProtocolHandler for MockProtocolHandler {
    fn init_session(&mut self) -> Result<(), Box<dyn StdError>> {
        debug!("初始化会话");
        Ok(())
    }
    
    fn get_device_info(&self) -> Result<DeviceInfo, Box<dyn StdError>> {
        debug!("获取设备信息");
        Ok(DeviceInfo {
            device_name: "模拟相机".to_string(),
            manufacturer: "RCamera".to_string(),
            model: "模拟型号".to_string(),
            serial_number: "12345678".to_string(),
            protocol_version: "1.0".to_string(),
            supported_operations: vec![],
        })
    }
    
    fn start_live_stream(&mut self) -> Result<(), Box<dyn StdError>> {
        debug!("开始实时数据流");
        Ok(())
    }
    
    fn stop_live_stream(&mut self) -> Result<(), Box<dyn StdError>> {
        debug!("停止实时数据流");
        Ok(())
    }
    
    fn close_session(&mut self) -> Result<(), Box<dyn StdError>> {
        debug!("关闭会话");
        Ok(())
    }
}

/// 数据监听器特性
pub trait DataListener {
    /// 处理接收到的数据包
    fn on_data_received(&mut self, packet: &DataPacket);
    
    /// 处理错误
    fn on_error(&mut self, error: &dyn StdError);
}

/// 数据包处理器 - 负责处理从相机接收的数据
pub struct DataProcessor {
    listeners: Vec<Box<dyn DataListener>>,
}

impl DataProcessor {
    /// 创建新的数据处理器
    pub fn new() -> Self {
        DataProcessor {
            listeners: Vec::new(),
        }
    }
    
    /// 添加数据监听器
    pub fn add_listener(&mut self, listener: Box<dyn DataListener>) {
        self.listeners.push(listener);
    }
    
    /// 处理收到的数据包
    pub fn process_packet(&mut self, packet: DataPacket) {
        debug!("处理数据包: {:?}", packet.packet_type);
        
        // 通知所有监听器
        for listener in &mut self.listeners {
            listener.on_data_received(&packet);
        }
    }
    
    /// 处理错误
    pub fn handle_error(&mut self, error: &dyn StdError) {
        error!("数据处理错误: {}", error);
        
        // 通知所有监听器
        for listener in &mut self.listeners {
            listener.on_error(error);
        }
    }
}
