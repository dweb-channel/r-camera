// 相机连接模块 - 负责USB连接和与相机的通信
use std::error::Error;
use std::time::Duration;
use libusb::{Context, Device, DeviceHandle};
use log::{info, error, debug};

// TODO
// pub mod device;
// pub mod connection;

/// 相机设备信息
pub struct CameraDevice {
    /// 厂商ID
    vendor_id: u16,
    /// 产品ID
    product_id: u16,
    /// 设备句柄
    device_handle: Option<DeviceHandle<Context>>,
}

impl CameraDevice {
    /// 创建新的相机设备实例
    pub fn new(vendor_id: u16, product_id: u16) -> Self {
        CameraDevice {
            vendor_id,
            product_id,
            device_handle: None,
        }
    }
    
    /// 搜索并列出所有连接的相机
    pub fn find_cameras() -> Result<Vec<CameraDevice>, Box<dyn Error>> {
        // 初始化 libusb 上下文
        let context = Context::new()?;
        let devices = context.devices()?;
        let mut cameras = Vec::new();

        for device in devices.iter() {
            let device_desc = device.device_descriptor()?;
            
            // 这里可以添加对常见相机设备的识别逻辑
            // 目前仅作为示例
            if is_camera_device(device_desc.vendor_id(), device_desc.product_id()) {
                info!("发现相机设备: VID={:04x}, PID={:04x}", 
                      device_desc.vendor_id(), device_desc.product_id());
                cameras.push(CameraDevice::new(
                    device_desc.vendor_id(),
                    device_desc.product_id()
                ));
            }
        }

        Ok(cameras)
    }

    /// 连接到相机设备
    pub fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        // 初始化USB上下文
        let context = Context::new()?;
        
        // 查找符合VID和PID的设备
        for device in context.devices()?.iter() {
            let device_desc = device.device_descriptor()?;
            
            if device_desc.vendor_id() == self.vendor_id && device_desc.product_id() == self.product_id {
                // 找到匹配设备，尝试打开连接
                debug!("找到相机设备: {:04x}:{:04x}", self.vendor_id, self.product_id);
                
                // 打开设备句柄
                let handle = match device.open() {
                    Ok(h) => h,
                    Err(e) => {
                        error!("无法打开相机设备: {}", e);
                        return Err(Box::new(e));
                    }
                };
                
                self.device_handle = Some(handle);
                info!("成功连接到相机设备");
                return Ok(());
            }
        }
        
        error!("未找到匹配的相机设备");
        Err("未找到相机设备".into())
    }
    
    /// 断开与相机的连接
    pub fn disconnect(&mut self) {
        if self.device_handle.is_some() {
            self.device_handle = None;
            info!("已断开与相机的连接");
        }
    }
    
    /// 检查相机是否已连接
    pub fn is_connected(&self) -> bool {
        self.device_handle.is_some()
    }
    
    /// 返回设备句柄引用（如果已连接）
    pub fn get_handle(&self) -> Option<&DeviceHandle<Context>> {
        self.device_handle.as_ref()
    }
}

impl Drop for CameraDevice {
    fn drop(&mut self) {
        // 确保在对象销毁时释放资源
        self.disconnect();
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
