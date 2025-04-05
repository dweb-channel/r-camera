// 相机连接模块 - 负责USB连接和与相机的通信
use std::error::Error;
use std::time::Duration;
use std::future::Future;
use log::{info, error, debug};

// Embassy相关导入
use embassy_usb::host::{UsbHost, UsbHostController, DeviceInfo, UsbDevice, ConfigDescriptor};
use esp_idf_svc::hal::usb::UsbHostDriver;
use embassy_executor::Executor;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration as EmbassyDuration, Timer};

// 定义错误类型
#[derive(Debug)]
pub enum CameraError {
    NotFound,
    ConnectionFailed(String),
    IoError(String),
}

impl std::fmt::Display for CameraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CameraError::NotFound => write!(f, "未找到相机设备"),
            CameraError::ConnectionFailed(e) => write!(f, "连接相机失败: {}", e),
            CameraError::IoError(e) => write!(f, "IO错误: {}", e),
        }
    }
}

impl std::error::Error for CameraError {}

// TODO
// pub mod device;
// pub mod connection;

/// 相机设备信息
pub struct CameraDevice {
    /// 厂商ID
    vendor_id: u16,
    /// 产品ID
    product_id: u16,
    /// Embassy USB设备句柄
    device_handle: Option<UsbDevice<'static>>,
    /// USB主机控制器
    usb_host: Option<UsbHost<'static, UsbHostDriver<'static>>>,
}

impl CameraDevice {
    /// 创建新的相机设备实例
    pub fn new(vendor_id: u16, product_id: u16) -> Self {
        CameraDevice {
            vendor_id,
            product_id,
            device_handle: None,
            usb_host: None,
        }
    }
    
    /// 初始化USB主机控制器
    /// 
    /// 这个方法需要在Embassy执行器中运行
    pub async fn init_usb_host(&mut self) -> Result<(), Box<dyn Error>> {
        // 注意: 实际使用时需要从ESP-IDF的外设获取USB控制器
        // 这里只是示例代码结构
        let usb_driver = unsafe { UsbHostDriver::new_static() };
        
        // 创建USB主机控制器
        let usb_host = UsbHost::new(usb_driver);
        self.usb_host = Some(usb_host);
        
        info!("USB主机控制器初始化完成");
        Ok(())
    }
    
    /// 搜索并列出所有连接的相机
    /// 
    /// 这个方法需要在Embassy执行器中运行
    pub async fn find_cameras(usb_host: &UsbHost<'static, UsbHostDriver<'static>>) -> Result<Vec<CameraDevice>, Box<dyn Error>> {
        let mut cameras = Vec::new();
        
        // 使用Embassy的异步API扫描设备
        // 遍历所有连接的USB设备
        for device_info in usb_host.devices().await {
            // 获取设备描述符
            let desc = device_info.device_descriptor();
            
            // 这里可以添加对常见相机设备的识别逻辑
            // 目前仅作为示例
            if is_camera_device(desc.vendor_id(), desc.product_id()) {
                info!("发现相机设备: VID={:04x}, PID={:04x}", 
                      desc.vendor_id(), desc.product_id());
                
                cameras.push(CameraDevice::new(
                    desc.vendor_id(),
                    desc.product_id()
                ));
            }
        }

        Ok(cameras)
    }

    /// 连接到相机设备
    /// 
    /// 这个方法需要在Embassy执行器中运行
    pub async fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        // 确保USB主机已初始化
        if self.usb_host.is_none() {
            return Err(Box::new(CameraError::ConnectionFailed("USB主机未初始化".into())));
        }
        
        let usb_host = self.usb_host.as_ref().unwrap();
        
        // 查找符合VID和PID的设备
        for device_info in usb_host.devices().await {
            let desc = device_info.device_descriptor();
            
            if desc.vendor_id() == self.vendor_id && desc.product_id() == self.product_id {
                // 找到匹配设备，尝试打开连接
                debug!("找到相机设备: {:04x}:{:04x}", self.vendor_id, self.product_id);
                
                // 使用Embassy-USB打开设备
                match usb_host.open_device(&device_info).await {
                    Ok(handle) => {
                        self.device_handle = Some(handle);
                        info!("成功连接到相机设备");
                        return Ok(());
                    },
                    Err(e) => {
                        let err_msg = format!("无法打开相机设备: {:?}", e);
                        error!("{}", err_msg);
                        return Err(Box::new(CameraError::ConnectionFailed(err_msg)));
                    }
                }
            }
        }
        
        error!("未找到匹配的相机设备");
        Err(Box::new(CameraError::NotFound))
    }
    
    /// 断开与相机的连接
    pub fn disconnect(&mut self) {
        if self.device_handle.is_some() {
            // Embassy-USB会在设备handle被drop时自动关闭连接
            self.device_handle = None;
            info!("已断开与相机的连接");
        }
    }
    
    /// 检查相机是否已连接
    pub fn is_connected(&self) -> bool {
        self.device_handle.is_some()
    }
    
    /// 返回设备句柄引用（如果已连接）
    pub fn get_handle(&self) -> Option<&UsbDevice<'static>> {
        self.device_handle.as_ref()
    }
    
    /// 执行批量传输 - 发送数据
    /// 
    /// 向指定端点发送数据并等待完成
    pub async fn bulk_write(&self, ep_addr: u8, data: &[u8], timeout: Duration) -> Result<usize, Box<dyn Error>> {
        if let Some(device) = &self.device_handle {
            // 将标准库Duration转换为Embassy的Duration
            let embassy_timeout = EmbassyDuration::from_millis(timeout.as_millis() as u64);
            
            // 使用Embassy-USB的批量写入API
            match device.bulk_out(ep_addr, data, embassy_timeout).await {
                Ok(len) => Ok(len),
                Err(e) => Err(Box::new(CameraError::IoError(format!("批量写入失败: {:?}", e))))
            }
        } else {
            Err(Box::new(CameraError::ConnectionFailed("设备未连接".into())))
        }
    }
    
    /// 执行批量传输 - 接收数据
    /// 
    /// 从指定端点接收数据并等待完成
    pub async fn bulk_read(&self, ep_addr: u8, buffer: &mut [u8], timeout: Duration) -> Result<usize, Box<dyn Error>> {
        if let Some(device) = &self.device_handle {
            // 将标准库Duration转换为Embassy的Duration
            let embassy_timeout = EmbassyDuration::from_millis(timeout.as_millis() as u64);
            
            // 使用Embassy-USB的批量读取API
            match device.bulk_in(ep_addr, buffer, embassy_timeout).await {
                Ok(len) => Ok(len),
                Err(e) => Err(Box::new(CameraError::IoError(format!("批量读取失败: {:?}", e))))
            }
        } else {
            Err(Box::new(CameraError::ConnectionFailed("设备未连接".into())))
        }
    }
}

impl Drop for CameraDevice {
    fn drop(&mut self) {
        // 确保在对象销毁时释放资源
        self.disconnect();
        // Embassy-USB的资源会在各自的Drop实现中自动释放
    }
}

/// 判断设备是否为相机设备
/// 
/// 基于厂商ID和产品ID检查设备是否可能是PTP/MTP相机
fn is_camera_device(vendor_id: u16, product_id: u16) -> bool {
    // 常见相机厂商的VID
    // 索尼、佳能、尼康、富士、松下等相机厂商的VID
    const CAMERA_VENDORS: &[u16] = &[
        0x054C, // Sony
        0x04A9, // Canon
        0x04B0, // Nikon
        0x04CB, // Fujifilm
        0x04DA, // Panasonic
        0x04B4, // Olympus
        0x4CB, // 富士
    ];
    
    // 检查是否为已知相机厂商
    // 实际应用中可能需要更复杂的逻辑，包括特定厂商+产品ID的组合
    if CAMERA_VENDORS.contains(&vendor_id) {
        debug!("检测到可能的相机设备: VID={:04x}, PID={:04x}", vendor_id, product_id);
        return true;
    }
    
    // 如果有特定的VID+PID组合需要识别，可以在这里添加
    
    false
}
