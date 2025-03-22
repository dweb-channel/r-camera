// 数据传输模块 - 负责协调相机数据的接收和无线传输
use std::error::Error;
use std::sync::{Arc, Mutex};
use log::{info, error, debug, warn};
use crate::ptp_mtp::{DataPacket, DataListener, PacketType};
use crate::wireless::DataSender;

// TODO
// pub mod buffer;
// pub mod processor;

/// 传输状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransferStatus {
    Idle,       // 空闲状态
    Starting,   // 启动中
    Running,    // 传输中
    Paused,     // 暂停
    Stopping,   // 停止中
    Error,      // 错误状态
}

/// 传输管理器 - 负责协调数据从相机到手机的传输
pub struct TransferManager {
    status: TransferStatus,
    buffer: Arc<Mutex<Vec<DataPacket>>>,
    data_sender: Option<Box<dyn DataSender>>,
    total_bytes_transferred: usize,
    max_buffer_size: usize,
}

impl TransferManager {
    /// 创建新的传输管理器
    pub fn new(max_buffer_size: usize) -> Self {
        TransferManager {
            status: TransferStatus::Idle,
            buffer: Arc::new(Mutex::new(Vec::new())),
            data_sender: None,
            total_bytes_transferred: 0,
            max_buffer_size,
        }
    }
    
    /// 设置数据发送器
    pub fn set_sender(&mut self, sender: Box<dyn DataSender>) {
        self.data_sender = Some(sender);
    }
    
    /// 启动传输
    pub fn start(&mut self) -> Result<(), Box<dyn Error>> {
        match self.status {
            TransferStatus::Idle | TransferStatus::Paused => {
                if self.data_sender.is_none() {
                    return Err("未设置数据发送器".into());
                }
                
                debug!("启动数据传输...");
                self.status = TransferStatus::Starting;
                
                // 启动传输处理线程
                // let buffer_clone = Arc::clone(&self.buffer);
                // let sender = self.data_sender.as_ref().unwrap();
                
                // 这里应该启动一个单独的线程来处理数据发送
                // 在ESP32上可能需要使用任务或其他机制来实现
                
                self.status = TransferStatus::Running;
                info!("数据传输已启动");
                Ok(())
            },
            _ => {
                warn!("无法启动传输：当前状态为 {:?}", self.status);
                Err(format!("无法从 {:?} 状态启动传输", self.status).into())
            }
        }
    }
    
    /// 暂停传输
    pub fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        if self.status == TransferStatus::Running {
            debug!("暂停数据传输...");
            self.status = TransferStatus::Paused;
            info!("数据传输已暂停");
            Ok(())
        } else {
            warn!("无法暂停传输：当前状态为 {:?}", self.status);
            Err(format!("无法从 {:?} 状态暂停传输", self.status).into())
        }
    }
    
    /// 停止传输
    pub fn stop(&mut self) -> Result<(), Box<dyn Error>> {
        if self.status == TransferStatus::Running || self.status == TransferStatus::Paused {
            debug!("停止数据传输...");
            self.status = TransferStatus::Stopping;
            
            // 清空缓冲区
            {
                let mut buffer = self.buffer.lock().unwrap();
                buffer.clear();
            }
            
            // 关闭发送器
            if let Some(sender) = &mut self.data_sender {
                sender.close()?;
            }
            
            self.status = TransferStatus::Idle;
            info!("数据传输已停止");
            Ok(())
        } else {
            warn!("无法停止传输：当前状态为 {:?}", self.status);
            Err(format!("无法从 {:?} 状态停止传输", self.status).into())
        }
    }
    
    /// 获取当前传输状态
    pub fn get_status(&self) -> TransferStatus {
        self.status
    }
    
    /// 获取已传输的总字节数
    pub fn get_bytes_transferred(&self) -> usize {
        self.total_bytes_transferred
    }
    
    /// 添加数据包到传输缓冲区
    fn add_packet_to_buffer(&mut self, packet: DataPacket) -> Result<(), Box<dyn Error>> {
        let mut buffer = self.buffer.lock().unwrap();
        
        // 检查缓冲区大小
        if buffer.len() >= self.max_buffer_size {
            // 缓冲区已满，移除最旧的数据包
            buffer.remove(0);
            warn!("缓冲区已满，移除最旧的数据包");
        }
        
        // 添加新数据包
        buffer.push(packet);
        Ok(())
    }
    
    /// 处理发送数据包
    fn process_buffer(&mut self) -> Result<(), Box<dyn Error>> {
        if self.status != TransferStatus::Running {
            return Ok(());
        }
        
        let sender = match &self.data_sender {
            Some(s) => s,
            None => return Err("未设置数据发送器".into()),
        };
        
        // 获取缓冲区中的数据包
        let mut packets_to_send = Vec::new();
        {
            let mut buffer = self.buffer.lock().unwrap();
            if !buffer.is_empty() {
                // 移动所有数据包到发送列表
                packets_to_send.append(&mut *buffer);
            }
        }
        
        // 发送数据包
        for packet in packets_to_send {
            // 根据包类型进行不同处理
            match packet.packet_type {
                PacketType::Image => {
                    debug!("发送图像数据包 ({} 字节)", packet.data.len());
                },
                PacketType::Thumbnail => {
                    debug!("发送缩略图数据包 ({} 字节)", packet.data.len());
                },
                PacketType::Metadata => {
                    debug!("发送元数据包 ({} 字节)", packet.data.len());
                },
                _ => {
                    debug!("发送其他类型数据包 ({} 字节)", packet.data.len());
                }
            }
            
            // 发送数据
            let bytes_sent = sender.send_data(&packet.data)?;
            self.total_bytes_transferred += bytes_sent;
        }
        
        Ok(())
    }
}

// 实现数据监听器接口，接收从相机来的数据
impl DataListener for TransferManager {
    fn on_data_received(&mut self, packet: &DataPacket) {
        if self.status != TransferStatus::Running {
            return;
        }
        
        // 将数据包添加到缓冲区
        match self.add_packet_to_buffer(packet.clone()) {
            Ok(_) => {
                // 触发处理缓冲区数据
                if let Err(e) = self.process_buffer() {
                    error!("处理数据包错误: {}", e);
                    self.status = TransferStatus::Error;
                }
            },
            Err(e) => {
                error!("添加数据包到缓冲区错误: {}", e);
                self.status = TransferStatus::Error;
            }
        }
    }
    
    fn on_error(&mut self, e: &dyn Error) {
        error!("数据传输错误: {}", e);
        self.status = TransferStatus::Error;
    }
}
