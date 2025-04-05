// PTP/MTP适配器模块 - 将PTP/MTP协议与Embassy-USB和ESP-IDF集成
use std::sync::Arc;
use std::sync::Mutex;
use log::{debug, error, info, warn};
use embassy_time::{Duration, Timer};
use embassy_usb::host::{UsbHost, UsbHostError};
use esp_idf_svc::hal::usb::UsbHostDriver;

use crate::usb_host::embassy::create_embassy_usb_host;
use crate::usb_host::embassy::wait_for_usb_device;
use crate::usb_host::filters::is_ptp_mtp_device;
use crate::usb_host::filters::device_by_vid_pid;
use crate::ptp_mtp::usb_transport::{PtpUsbTransport, find_ptp_device};
use crate::ptp_mtp::camera::PtpCamera;
use crate::ptp_mtp::error::Error;

/// PTP/MTP相机连接管理器
/// 负责发现、连接和管理PTP/MTP相机设备
pub struct PtpCameraAdapter {
    // USB主机实例
    usb_host: UsbHost<'static, UsbHostDriver<'static>>,
    // 已连接的相机实例
    camera: Option<Arc<Mutex<PtpCamera>>>,
    // 相机状态
    status: CameraStatus,
}

/// 相机连接状态
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraStatus {
    // 断开连接
    Disconnected,
    // 已连接但未初始化
    Connected,
    // 已初始化，会话打开
    SessionOpen,
    // 错误状态
    Error,
}

impl PtpCameraAdapter {
    /// 创建新的PTP相机适配器
    pub fn new() -> Result<Self, String> {
        debug!("初始化PTP相机适配器");
        
        // 创建Embassy USB主机
        let usb_host = create_embassy_usb_host()
            .map_err(|e| format!("无法创建USB主机: {}", e))?;
        
        // 创建适配器实例
        Ok(Self {
            usb_host,
            camera: None,
            status: CameraStatus::Disconnected,
        })
    }
    
    /// 扫描并连接PTP/MTP相机
    /// 
    /// vid - 可选的厂商ID过滤器
    /// pid - 可选的产品ID过滤器
    /// timeout_ms - 扫描超时时间（毫秒）
    pub async fn connect_camera(
        &mut self,
        vid: Option<u16>,
        pid: Option<u16>,
        timeout_ms: Option<u64>
    ) -> Result<(), Error> {
        // 断开任何现有连接
        self.disconnect().await;
        
        info!("扫描PTP/MTP相机设备...");
        
        // 创建过滤器函数
        let filter = |device_info: &embassy_usb::host::DeviceInfo| {
            // 首先检查VID/PID过滤器(如果提供)
            if let (Some(vid), Some(pid)) = (vid, pid) {
                if device_info.device_descriptor().vendor_id() != vid ||
                   device_info.device_descriptor().product_id() != pid {
                    return false;
                }
            } else if let Some(vid) = vid {
                if device_info.device_descriptor().vendor_id() != vid {
                    return false;
                }
            } else if let Some(pid) = pid {
                if device_info.device_descriptor().product_id() != pid {
                    return false;
                }
            }
            
            // 然后检查是否为PTP/MTP设备
            is_ptp_mtp_device(device_info)
        };
        
        // 等待并查找符合条件的设备
        match wait_for_usb_device(&self.usb_host, timeout_ms, filter).await {
            Some(device_info) => {
                // 找到设备，尝试创建PTP传输
                let v_id = device_info.device_descriptor().vendor_id();
                let p_id = device_info.device_descriptor().product_id();
                
                info!("发现PTP/MTP设备: VID={:04x}, PID={:04x}", v_id, p_id);
                
                // 创建PTP传输层
                match find_ptp_device(&self.usb_host, Some(v_id), Some(p_id)).await {
                    Ok(transport) => {
                        // 创建PTP相机实例
                        let camera = PtpCamera::new(transport);
                        self.camera = Some(Arc::new(Mutex::new(camera)));
                        self.status = CameraStatus::Connected;
                        
                        info!("已连接PTP/MTP相机设备");
                        Ok(())
                    },
                    Err(e) => {
                        error!("无法创建PTP传输层: {}", e);
                        self.status = CameraStatus::Error;
                        Err(e)
                    }
                }
            },
            None => {
                warn!("未找到PTP/MTP相机设备");
                self.status = CameraStatus::Disconnected;
                Err("未找到PTP/MTP相机设备".into())
            }
        }
    }
    
    /// 打开PTP会话
    pub async fn open_session(&mut self) -> Result<(), Error> {
        if self.status != CameraStatus::Connected {
            return Err("相机未连接或已打开会话".into());
        }
        
        let camera = self.camera.as_ref().ok_or("相机未连接")?;
        let mut camera_guard = camera.lock().unwrap();
        
        // 打开PTP会话
        match camera_guard.open_session(None).await {
            Ok(_) => {
                self.status = CameraStatus::SessionOpen;
                info!("PTP会话已成功打开");
                Ok(())
            },
            Err(e) => {
                error!("无法打开PTP会话: {}", e);
                self.status = CameraStatus::Error;
                Err(e)
            }
        }
    }
    
    /// 关闭PTP会话
    pub async fn close_session(&mut self) -> Result<(), Error> {
        if self.status != CameraStatus::SessionOpen {
            return Err("没有活动的PTP会话".into());
        }
        
        let camera = self.camera.as_ref().ok_or("相机未连接")?;
        let mut camera_guard = camera.lock().unwrap();
        
        // 关闭PTP会话
        match camera_guard.close_session(None).await {
            Ok(_) => {
                self.status = CameraStatus::Connected;
                info!("PTP会话已成功关闭");
                Ok(())
            },
            Err(e) => {
                error!("无法关闭PTP会话: {}", e);
                // 即使关闭会话失败，我们也将状态设置为Connected
                // 因为这样可以尝试重新打开会话
                self.status = CameraStatus::Connected;
                Err(e)
            }
        }
    }
    
    /// 断开相机连接
    pub async fn disconnect(&mut self) {
        // 如果有会话打开，尝试关闭
        if self.status == CameraStatus::SessionOpen {
            let _ = self.close_session().await;
        }
        
        // 清除相机实例
        self.camera = None;
        self.status = CameraStatus::Disconnected;
        
        info!("相机已断开连接");
    }
    
    /// 获取相机访问权
    /// 返回相机实例的Arc<Mutex<>>，可以用于外部访问
    pub fn get_camera(&self) -> Option<Arc<Mutex<PtpCamera>>> {
        self.camera.clone()
    }
    
    /// 获取相机当前状态
    pub fn status(&self) -> CameraStatus {
        self.status
    }
    
    /// 检查相机是否已连接
    pub fn is_connected(&self) -> bool {
        self.status == CameraStatus::Connected || self.status == CameraStatus::SessionOpen
    }
    
    /// 检查相机是否有活动的PTP会话
    pub fn has_session(&self) -> bool {
        self.status == CameraStatus::SessionOpen
    }
}

/// 辅助函数：扫描并打印所有PTP/MTP设备信息
pub async fn scan_and_list_ptp_devices() -> Result<Vec<(u16, u16, String)>, Error> {
    // 创建USB主机
    let usb_host = create_embassy_usb_host()
        .map_err(|e| Error::USB(format!("无法创建USB主机: {}", e)))?;
    
    let mut result = Vec::new();
    
    // 获取所有设备
    let devices = usb_host.devices().await;
    debug!("发现 {} 个USB设备", devices.len());
    
    for device_info in devices {
        let device_desc = device_info.device_descriptor();
        let vid = device_desc.vendor_id();
        let pid = device_desc.product_id();
        
        // 只检查PTP/MTP设备
        if is_ptp_mtp_device(&device_info) {
            info!("发现PTP/MTP设备: VID={:04x}, PID={:04x}", vid, pid);
            
            // 尝试获取设备描述字符串
            let device = device_info.device();
            let mut device_name = format!("未知设备 {:04x}:{:04x}", vid, pid);
            
            if let Some(manufacturer) = device.manufacturer_string().await.ok() {
                if let Some(product) = device.product_string().await.ok() {
                    device_name = format!("{} {}", manufacturer, product);
                }
            }
            
            result.push((vid, pid, device_name));
        }
    }
    
    Ok(result)
}
