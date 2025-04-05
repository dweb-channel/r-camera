// USB主机管理模块 - 负责ESP-IDF USB主机驱动的初始化和管理
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::Once;
use std::time::Duration;
use embassy_time::{Duration as EmbassyDuration, Timer};
use log::{info, error, debug, warn};

use esp_idf_hal::{peripheral::Peripheral, prelude::*};
use esp_idf_hal::usb::{self, UsbHost};
use esp_idf_sys::EspError;
use esp_idf_svc::hal::usb::{UsbHostDriver, UsbHostConfiguration};
use embassy_usb::host::{UsbHostController, DeviceInfo};

// USB主机驱动状态
pub enum UsbHostState {
    Uninitialized,  // 未初始化
    Ready,          // 准备就绪
    Running,        // 运行中
    Error(String),  // 错误状态
}

// USB主机控制器 - 单例模式
pub struct EspUsbHostController {
    // 内部驱动实例
    driver: UsbHostDriver<'static>,
    // 当前状态
    state: UsbHostState,
}

// 全局单例实例
static mut USB_HOST: Option<Arc<Mutex<EspUsbHostController>>> = None;
static INIT: Once = Once::new();

impl EspUsbHostController {
    /// 获取USB主机控制器单例实例
    pub fn instance() -> Arc<Mutex<EspUsbHostController>> {
        unsafe {
            INIT.call_once(|| {
                // 创建USB主机驱动配置
                // 注意: 实际使用时可能需要根据不同的ESP32型号进行调整
                let config = UsbHostConfiguration {
                    global_intr_flags: esp_idf_sys::ESP_INTR_FLAG_LEVEL1,
                    max_power_usage: 500, // 500mA
                };
                
                // 尝试初始化USB主机驱动
                match UsbHostDriver::new_static(config) {
                    Ok(driver) => {
                        USB_HOST = Some(Arc::new(Mutex::new(EspUsbHostController {
                            driver,
                            state: UsbHostState::Ready,
                        })));
                        info!("ESP-IDF USB主机控制器已初始化");
                    },
                    Err(e) => {
                        // 初始化失败，创建错误状态的实例
                        error!("无法初始化ESP-IDF USB主机控制器: {:?}", e);
                        let empty_driver = unsafe {
                            // 创建一个不可用的驱动实例仅作为占位符
                            UsbHostDriver::new_static_uninit()
                        };
                        USB_HOST = Some(Arc::new(Mutex::new(EspUsbHostController {
                            driver: empty_driver,
                            state: UsbHostState::Error(format!("初始化失败: {:?}", e)),
                        })));
                    }
                }
            });
            
            USB_HOST.as_ref().unwrap().clone()
        }
    }
    
    /// 获取内部驱动的引用
    pub fn get_driver(&self) -> &UsbHostDriver<'static> {
        &self.driver
    }
    
    /// 获取内部驱动的可变引用
    pub fn get_driver_mut(&mut self) -> &mut UsbHostDriver<'static> {
        &mut self.driver
    }
    
    /// 获取当前状态
    pub fn get_state(&self) -> &UsbHostState {
        &self.state
    }
    
    /// 设置控制器状态
    pub fn set_state(&mut self, state: UsbHostState) {
        self.state = state;
    }
    
    /// 启动USB主机控制器
    pub fn start(&mut self) -> Result<(), EspError> {
        match self.state {
            UsbHostState::Ready => {
                debug!("正在启动USB主机控制器...");
                self.driver.initialize_host()?;
                self.state = UsbHostState::Running;
                info!("USB主机控制器已启动");
                Ok(())
            },
            UsbHostState::Running => {
                debug!("USB主机控制器已在运行中");
                Ok(())
            },
            UsbHostState::Error(ref e) => {
                error!("无法启动USB主机控制器，处于错误状态: {}", e);
                Err(EspError::from_non_zero(esp_idf_sys::ESP_ERR_INVALID_STATE as i32))
            },
            UsbHostState::Uninitialized => {
                error!("无法启动未初始化的USB主机控制器");
                Err(EspError::from_non_zero(esp_idf_sys::ESP_ERR_INVALID_STATE as i32))
            }
        }
    }
    
    /// 停止USB主机控制器
    pub fn stop(&mut self) -> Result<(), EspError> {
        match self.state {
            UsbHostState::Running => {
                debug!("正在停止USB主机控制器...");
                self.driver.deinitialize_host()?;
                self.state = UsbHostState::Ready;
                info!("USB主机控制器已停止");
                Ok(())
            },
            UsbHostState::Ready => {
                debug!("USB主机控制器已经是停止状态");
                Ok(())
            },
            UsbHostState::Error(ref e) => {
                warn!("尝试停止错误状态的USB主机控制器: {}", e);
                Ok(())
            },
            UsbHostState::Uninitialized => {
                warn!("尝试停止未初始化的USB主机控制器");
                Ok(())
            }
        }
    }
}

// 为了在Embassy运行时中使用，我们需要实现辅助函数
pub mod embassy {
    use super::*;
    use embassy_usb::host::{UsbHost, UsbHostController};
    
    /// 创建Embassy USB主机实例
    pub fn create_embassy_usb_host() -> Result<UsbHost<'static, UsbHostDriver<'static>>, String> {
        let controller = EspUsbHostController::instance();
        let mut controller_lock = controller.lock().unwrap();
        
        // 检查控制器状态
        match controller_lock.get_state() {
            UsbHostState::Ready => {
                // 如果准备就绪，启动控制器
                if let Err(e) = controller_lock.start() {
                    return Err(format!("无法启动USB主机控制器: {:?}", e));
                }
            },
            UsbHostState::Running => {
                // 已经在运行，继续使用
            },
            UsbHostState::Error(e) => {
                return Err(format!("USB主机控制器处于错误状态: {}", e));
            },
            UsbHostState::Uninitialized => {
                return Err("USB主机控制器未初始化".into());
            }
        }
        
        // 使用控制器驱动创建Embassy USB主机
        // 注意：我们需要克隆控制器引用以便传递所有权
        let driver = controller_lock.get_driver_mut();
        
        // 创建Embassy USB主机
        let usb_host = UsbHost::new(unsafe {
            // 这里需要将驱动的引用转换为'static
            // 因为单例模式保证了驱动的生命周期是'static的
            core::mem::transmute::<&mut UsbHostDriver<'static>, &mut UsbHostDriver<'static>>(driver)
        });
        
        Ok(usb_host)
    }
    
    /// 在异步上下文中扫描并等待USB设备连接
    pub async fn wait_for_usb_device(
        usb_host: &UsbHost<'static, UsbHostDriver<'static>>,
        timeout_ms: Option<u64>,
        filter: impl Fn(&DeviceInfo) -> bool,
    ) -> Option<DeviceInfo> {
        debug!("等待USB设备连接...");
        
        // 设置超时时间
        let deadline = timeout_ms.map(|ms| 
            embassy_time::Instant::now() + EmbassyDuration::from_millis(ms)
        );
        
        loop {
            // 检查是否超时
            if let Some(deadline) = deadline {
                if embassy_time::Instant::now() >= deadline {
                    debug!("等待USB设备连接超时");
                    return None;
                }
            }
            
            // 获取所有连接的设备
            for device in usb_host.devices().await {
                // 使用过滤器检查设备
                if filter(&device) {
                    info!("发现匹配的USB设备: VID={:04x}, PID={:04x}", 
                          device.device_descriptor().vendor_id(),
                          device.device_descriptor().product_id());
                    return Some(device);
                }
            }
            
            // 短暂延迟后再次尝试
            Timer::after(EmbassyDuration::from_millis(200)).await;
        }
    }
}

// PTP/MTP设备过滤器
pub mod filters {
    use embassy_usb::host::{DeviceInfo, ConfigDescriptor};
    
    /// 检查设备是否为PTP/MTP设备
    pub fn is_ptp_mtp_device(device: &DeviceInfo) -> bool {
        // 检查所有配置
        let config = device.current_config_descriptor();
        
        // 遍历所有接口
        for iface in config.interfaces() {
            // 遍历接口的所有设置
            for alt_setting in iface.alt_settings() {
                // 检查是否是PTP/MTP类 (类代码6表示图像类，子类1表示静态捕获，协议1表示PTP)
                if alt_setting.class_code() == 6 {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// 检查是否为指定厂商和产品ID的设备
    pub fn device_by_vid_pid(vid: u16, pid: u16) -> impl Fn(&DeviceInfo) -> bool {
        move |device: &DeviceInfo| {
            let desc = device.device_descriptor();
            desc.vendor_id() == vid && desc.product_id() == pid
        }
    }
    
    /// 检查是否为已知的相机厂商
    pub fn is_camera_vendor(device: &DeviceInfo) -> bool {
        // 常见相机厂商的VID
        const CAMERA_VENDORS: &[u16] = &[
            0x054C, // Sony
            0x04A9, // Canon
            0x04B0, // Nikon
            0x04CB, // Fujifilm
            0x04DA, // Panasonic
            0x04B4, // Olympus
        ];
        
        let vid = device.device_descriptor().vendor_id();
        CAMERA_VENDORS.contains(&vid)
    }
}
