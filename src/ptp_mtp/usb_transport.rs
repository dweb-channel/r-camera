// PTP/MTP 协议的USB传输层实现
use std::sync::Arc;
use log::{error, debug, info, warn};
use embassy_usb::host::{DeviceInfo, Device, Interface, UsbHostError, UsbHost};
use embassy_futures::join::join;
use embassy_time::{Duration, Timer};
use embassy_sync::{mutex::Mutex, blocking_mutex::raw::NoopRawMutex};
use esp_idf_svc::hal::usb::UsbHostDriver;

use crate::usb_host::EspUsbHostController;
use crate::ptp_mtp::error::Error;

// PTP协议常量
const PTP_CLASS: u8 = 6;         // 图像类
const PTP_SUBCLASS: u8 = 1;      // 静态捕获设备
const PTP_PROTOCOL: u8 = 1;      // 图片传输协议

// 端点传输超时
const EP_TRANSFER_TIMEOUT_MS: u64 = 5000;

/// PTP/MTP USB传输管理器
/// 负责与USB设备的低级通信，为PTP/MTP协议提供传输层支持
pub struct PtpUsbTransport {
    // 设备接口
    interface: Interface<'static, UsbHostDriver<'static>>,
    // 批量输入端点 (从设备到主机)
    bulk_in_ep: Option<u8>,
    // 批量输出端点 (从主机到设备)
    bulk_out_ep: Option<u8>,
    // 中断端点 (事件通知)
    intr_ep: Option<u8>,
    // 设备VID
    vendor_id: u16,
    // 设备PID
    product_id: u16,
}

impl PtpUsbTransport {
    /// 创建新的PTP/MTP USB传输管理器
    /// device - USB设备信息
    /// iface - 已初始化的USB接口
    pub fn new(
        device: &DeviceInfo,
        iface: Interface<'static, UsbHostDriver<'static>>
    ) -> Result<Self, Error> {
        let device_desc = device.device_descriptor();
        let vendor_id = device_desc.vendor_id();
        let product_id = device_desc.product_id();
        
        debug!("初始化PTP/MTP传输层: VID={:04x}, PID={:04x}", vendor_id, product_id);
        
        // 创建传输管理器实例
        let mut transport = Self {
            interface: iface,
            bulk_in_ep: None,
            bulk_out_ep: None,
            intr_ep: None,
            vendor_id,
            product_id,
        };
        
        // 查找并配置所有必要的端点
        transport.configure_endpoints()?;
        
        Ok(transport)
    }
    
    /// 发现并配置PTP/MTP设备的端点
    fn configure_endpoints(&mut self) -> Result<(), Error> {
        let alt_setting = self.interface.current_alt_setting();
        debug!("配置PTP/MTP端点: 接口={}, 设置={}", 
               self.interface.interface_number(), 
               alt_setting.alt_setting_number());
        
        // 遍历接口上的所有端点
        for ep in alt_setting.endpoints() {
            let ep_addr = ep.endpoint_address();
            let ep_dir_in = (ep_addr & 0x80) != 0; // 最高位判断方向(1=IN, 0=OUT)
            let ep_number = ep_addr & 0x0F;  // 低4位为端点号
            
            match ep.transfer_type() {
                embassy_usb::host::TransferType::Bulk => {
                    if ep_dir_in {
                        // 批量输入端点 (设备->主机)
                        debug!("发现批量输入端点: 0x{:02x}", ep_addr);
                        self.bulk_in_ep = Some(ep_addr);
                    } else {
                        // 批量输出端点 (主机->设备)
                        debug!("发现批量输出端点: 0x{:02x}", ep_addr);
                        self.bulk_out_ep = Some(ep_addr);
                    }
                },
                embassy_usb::host::TransferType::Interrupt => {
                    if ep_dir_in {
                        // 中断端点 (事件通知)
                        debug!("发现中断端点: 0x{:02x}", ep_addr);
                        self.intr_ep = Some(ep_addr);
                    }
                },
                _ => {} // 忽略其他类型的端点
            }
        }
        
        // 验证是否找到了所有必要的端点
        if self.bulk_in_ep.is_none() {
            return Err("未找到批量输入端点".into());
        }
        if self.bulk_out_ep.is_none() {
            return Err("未找到批量输出端点".into());
        }
        
        // 中断端点不是必须的，但通常存在
        if self.intr_ep.is_none() {
            warn!("未找到中断端点，事件通知功能将不可用");
        }
        
        info!("PTP/MTP端点配置完成: IN=0x{:02x}, OUT=0x{:02x}, INTR={:?}",
              self.bulk_in_ep.unwrap(),
              self.bulk_out_ep.unwrap(),
              self.intr_ep);
        
        Ok(())
    }
    
    /// 执行批量写入操作 (主机到设备)
    /// data - 要写入的数据
    pub async fn bulk_write(&mut self, data: &[u8]) -> Result<usize, Error> {
        let ep_addr = self.bulk_out_ep.ok_or("批量输出端点未配置")?;
        debug!("批量写入 {} 字节数据到端点 0x{:02x}", data.len(), ep_addr);
        
        match self.interface.write_bulk(ep_addr, data, Duration::from_millis(EP_TRANSFER_TIMEOUT_MS)).await {
            Ok(transferred) => {
                debug!("成功写入 {} 字节", transferred);
                Ok(transferred)
            },
            Err(e) => {
                error!("批量写入失败: {:?}", e);
                Err(format!("批量写入错误: {:?}", e).into())
            }
        }
    }
    
    /// 执行批量读取操作 (设备到主机)
    /// buffer - 读取数据的缓冲区
    /// timeout_ms - 超时时间 (毫秒)
    pub async fn bulk_read(&mut self, buffer: &mut [u8], timeout_ms: Option<u64>) -> Result<usize, Error> {
        let ep_addr = self.bulk_in_ep.ok_or("批量输入端点未配置")?;
        let timeout = Duration::from_millis(timeout_ms.unwrap_or(EP_TRANSFER_TIMEOUT_MS));
        
        debug!("从端点 0x{:02x} 批量读取，最大 {} 字节，超时 {} ms", 
               ep_addr, buffer.len(), timeout.as_millis());
        
        match self.interface.read_bulk(ep_addr, buffer, timeout).await {
            Ok(transferred) => {
                debug!("成功读取 {} 字节", transferred);
                Ok(transferred)
            },
            Err(e) => {
                error!("批量读取失败: {:?}", e);
                Err(format!("批量读取错误: {:?}", e).into())
            }
        }
    }
    
    /// 执行控制传输 (用于PTP设备控制)
    /// request_type - 请求类型
    /// request - 请求代码
    /// value - 请求值
    /// index - 索引值
    /// data - 数据缓冲区
    pub async fn control_transfer(
        &mut self,
        request_type: u8,
        request: u8,
        value: u16,
        index: u16,
        data: &mut [u8]
    ) -> Result<usize, Error> {
        debug!("控制传输: type=0x{:02x}, req=0x{:02x}, val=0x{:04x}, idx=0x{:04x}, len={}",
              request_type, request, value, index, data.len());
        
        match self.interface.device().control(
            request_type,
            request,
            value,
            index,
            data,
            Duration::from_millis(EP_TRANSFER_TIMEOUT_MS)
        ).await {
            Ok(transferred) => {
                debug!("控制传输成功，传输 {} 字节", transferred);
                Ok(transferred)
            },
            Err(e) => {
                error!("控制传输失败: {:?}", e);
                Err(format!("控制传输错误: {:?}", e).into())
            }
        }
    }
    
    /// 从中断端点读取事件 (非阻塞)
    /// buffer - 事件数据缓冲区
    pub async fn read_interrupt_event(&mut self, buffer: &mut [u8]) -> Result<usize, Error> {
        let ep_addr = self.intr_ep.ok_or("中断端点未配置")?;
        
        match self.interface.read_interrupt(
            ep_addr,
            buffer,
            Duration::from_millis(100) // 使用短超时以保持非阻塞特性
        ).await {
            Ok(transferred) => {
                if transferred > 0 {
                    debug!("接收到中断事件，{} 字节", transferred);
                }
                Ok(transferred)
            },
            Err(e) => {
                // 超时通常不被视为错误，因为中断事件是可选的
                if matches!(e, UsbHostError::Timeout) {
                    Ok(0)
                } else {
                    error!("中断读取失败: {:?}", e);
                    Err(format!("中断读取错误: {:?}", e).into())
                }
            }
        }
    }
    
    /// 获取设备VID
    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }
    
    /// 获取设备PID
    pub fn product_id(&self) -> u16 {
        self.product_id
    }
    
    /// 获取接口号
    pub fn interface_number(&self) -> u8 {
        self.interface.interface_number()
    }
    
    /// 重置设备 - 在发生错误后恢复设备状态
    pub async fn reset(&mut self) -> Result<(), Error> {
        debug!("重置USB设备...");
        
        // 尝试重置设备
        if let Err(e) = self.interface.device().reset().await {
            error!("设备重置失败: {:?}", e);
            return Err(format!("设备重置错误: {:?}", e).into());
        }
        
        // 重新配置端点
        self.configure_endpoints()?;
        
        debug!("USB设备重置完成");
        Ok(())
    }
}

/// 查找并打开PTP/MTP设备
/// usb_host - USB主机控制器
/// vendor_id - 可选的厂商ID过滤器
/// product_id - 可选的产品ID过滤器
pub async fn find_ptp_device(
    usb_host: &UsbHost<'static, UsbHostDriver<'static>>,
    vendor_id: Option<u16>,
    product_id: Option<u16>
) -> Result<PtpUsbTransport, Error> {
    debug!("正在查找PTP/MTP设备...");
    
    // 扫描设备
    let devices = usb_host.devices().await;
    
    for device_info in devices {
        let device_desc = device_info.device_descriptor();
        let vid = device_desc.vendor_id();
        let pid = device_desc.product_id();
        
        // 检查VID/PID过滤器
        if let Some(filter_vid) = vendor_id {
            if vid != filter_vid {
                continue;
            }
        }
        
        if let Some(filter_pid) = product_id {
            if pid != filter_pid {
                continue;
            }
        }
        
        debug!("检查设备 VID={:04x}, PID={:04x}", vid, pid);
        
        // 获取设备配置信息
        let config = device_info.current_config_descriptor();
        
        // 查找PTP/MTP接口
        for iface_num in 0..config.num_interfaces() {
            let iface = match device_info.device().interface(iface_num) {
                Ok(i) => i,
                Err(_) => continue,
            };
            
            // 检查当前接口设置
            let alt_setting = iface.current_alt_setting();
            
            // 检查是否是PTP类
            if alt_setting.class_code() == PTP_CLASS && 
               alt_setting.sub_class_code() == PTP_SUBCLASS && 
               alt_setting.protocol_code() == PTP_PROTOCOL {
                info!("发现PTP/MTP设备: VID={:04x}, PID={:04x}, 接口={}", 
                      vid, pid, iface_num);
                
                // 创建传输管理器
                let transport = PtpUsbTransport::new(&device_info, iface)?;
                return Ok(transport);
            }
        }
    }
    
    error!("未找到符合条件的PTP/MTP设备");
    Err("未找到PTP/MTP设备".into())
}

/// PTP/MTP设备连接监听器
/// 持续监听并等待PTP/MTP设备连接
pub async fn monitor_ptp_devices(
    usb_host: UsbHost<'static, UsbHostDriver<'static>>,
    connection_callback: impl Fn(PtpUsbTransport) -> ()
) {
    info!("开始监听PTP/MTP设备连接...");
    
    loop {
        // 等待并检查设备连接
        match find_ptp_device(&usb_host, None, None).await {
            Ok(transport) => {
                info!("PTP/MTP设备已连接");
                
                // 调用回调函数处理连接的设备
                connection_callback(transport);
            },
            Err(_) => {
                // 没有找到设备，等待一段时间后重试
                Timer::after(Duration::from_millis(1000)).await;
            }
        }
    }
}
