// 相机连接模块 - 负责USB连接和与相机的通信
use std::error::Error;
use rusb::{Context, Device, DeviceHandle, UsbContext};
use log::{info, error, debug};

// TODO
// pub mod device;
// pub mod connection;

/// 相机设备信息
pub struct CameraDevice {
    vendor_id: u16,
    product_id: u16,
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
