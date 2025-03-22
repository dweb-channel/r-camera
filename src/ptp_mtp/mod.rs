// PTP/MTP协议处理模块 - 负责与相机进行PTP和MTP协议通信
use log::{info, error, debug};
use std::error::Error;

// TODO
// pub mod ptp;
// pub mod mtp;
// pub mod streaming;

/// 支持的传输协议类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProtocolType {
    PTP,
    MTP,
}

/// 协议处理器特性
pub trait ProtocolHandler {
    /// 初始化协议会话
    fn init_session(&mut self) -> Result<(), Box<dyn Error>>;
    
    /// 获取设备信息
    fn get_device_info(&self) -> Result<DeviceInfo, Box<dyn Error>>;
    
    /// 开始实时数据流传输
    fn start_live_stream(&mut self) -> Result<(), Box<dyn Error>>;
    
    /// 停止实时数据流传输
    fn stop_live_stream(&mut self) -> Result<(), Box<dyn Error>>;
    
    /// 关闭会话
    fn close_session(&mut self) -> Result<(), Box<dyn Error>>;
}

/// 设备信息结构
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub device_name: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub protocol_version: String,
    pub supported_operations: Vec<u16>,
}

/// 传输数据包
#[derive(Debug, Clone)]
pub struct DataPacket {
    pub data: Vec<u8>,
    pub timestamp: std::time::SystemTime,
    pub packet_type: PacketType,
}

/// 数据包类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PacketType {
    Image,
    Thumbnail,
    Metadata,
    Command,
    Response,
}

/// 创建协议处理器
pub fn create_protocol_handler(protocol_type: ProtocolType, device_handle: &rusb::DeviceHandle<rusb::Context>) -> Box<dyn ProtocolHandler> {
    match protocol_type {
        ProtocolType::PTP => {
            debug!("创建PTP协议处理器");
            Box::new(ptp::PtpHandler::new(device_handle))
        },
        ProtocolType::MTP => {
            debug!("创建MTP协议处理器");
            Box::new(mtp::MtpHandler::new(device_handle))
        }
    }
}

/// 数据监听器特性
pub trait DataListener {
    /// 处理接收到的数据包
    fn on_data_received(&mut self, packet: &DataPacket);
    
    /// 处理错误
    fn on_error(&mut self, error: &dyn Error);
}

// 数据包处理器 - 负责处理从相机接收的数据
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
    fn handle_error(&mut self, error: &dyn Error) {
        error!("数据处理错误: {}", error);
        
        // 通知所有监听器
        for listener in &mut self.listeners {
            listener.on_error(error);
        }
    }
}
