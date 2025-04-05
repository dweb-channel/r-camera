// PTP相机示例 - 展示如何使用Embassy-USB和ESP-IDF主机驱动连接PTP相机
#![allow(unused_imports)]
use std::sync::Arc;
use std::time::Duration;
use log::{info, debug, error, warn};

use esp_idf_hal::delay::FreeRtos;
use esp_idf_svc::sys;
use embassy_executor::Executor;
use embassy_executor::_export::StaticCell;
use embassy_time::Timer;

use crate::usb_host::embassy::create_embassy_usb_host;
use crate::ptp_mtp::adapter::PtpCameraAdapter;
use crate::ptp_mtp::adapter::scan_and_list_ptp_devices;

/// 主要相机示例任务
/// 此任务演示了如何扫描、连接和控制PTP相机
#[embassy_executor::task]
async fn camera_task() {
    // 延迟一段时间，确保系统启动完成
    Timer::after_secs(1).await;
    
    info!("开始PTP相机示例...");
    
    // 首先扫描所有PTP设备
    match scan_and_list_ptp_devices().await {
        Ok(devices) => {
            if devices.is_empty() {
                info!("未找到PTP相机设备，请连接支持PTP的相机");
            } else {
                info!("发现 {} 个PTP相机设备:", devices.len());
                for (i, (vid, pid, name)) in devices.iter().enumerate() {
                    info!("  {}: {} (VID: {:04x}, PID: {:04x})", i+1, name, vid, pid);
                }
                
                // 选择第一个发现的相机
                if let Some((vid, pid, _)) = devices.first() {
                    connect_to_camera(*vid, *pid).await;
                }
            }
        },
        Err(e) => {
            error!("扫描PTP设备时出错: {}", e);
        }
    }
    
    info!("PTP相机示例结束");
}

/// 连接并控制相机的辅助函数
async fn connect_to_camera(vid: u16, pid: u16) {
    info!("尝试连接到相机 VID={:04x}, PID={:04x}...", vid, pid);
    
    // 创建PTP相机适配器
    match PtpCameraAdapter::new() {
        Ok(mut adapter) => {
            // 尝试连接相机
            match adapter.connect_camera(Some(vid), Some(pid), Some(10000)).await {
                Ok(_) => {
                    info!("已成功连接到相机");
                    
                    // 打开PTP会话
                    if let Err(e) = adapter.open_session().await {
                        error!("无法打开PTP会话: {}", e);
                        return;
                    }
                    
                    info!("PTP会话已打开");
                    
                    // 获取相机实例用于操作
                    if let Some(camera) = adapter.get_camera() {
                        // 获取锁并执行相机操作
                        let mut camera_guard = camera.lock().unwrap();
                        
                        // 获取设备信息
                        info!("获取相机信息...");
                        match camera_guard.get_device_info(None).await {
                            Ok(device_info) => {
                                info!("相机信息:");
                                info!("  厂商: {}", device_info.vendor);
                                info!("  型号: {}", device_info.model);
                                info!("  版本: {}", device_info.device_version);
                                if let Some(serial) = &device_info.serial_number {
                                    info!("  序列号: {}", serial);
                                }
                                
                                // 获取存储ID
                                info!("获取存储ID...");
                                match camera_guard.get_storageids(None).await {
                                    Ok(storage_ids) => {
                                        info!("发现 {} 个存储设备", storage_ids.len());
                                        
                                        // 遍历所有存储
                                        for (i, storage_id) in storage_ids.iter().enumerate() {
                                            info!("存储 #{}: ID=0x{:08x}", i+1, storage_id);
                                            
                                            // 获取存储信息
                                            if let Ok(storage_info) = camera_guard.get_storage_info(*storage_id, None).await {
                                                info!("  描述: {}", storage_info.storage_description);
                                                info!("  卷标: {}", storage_info.volume_label);
                                                info!("  容量: {}MB", storage_info.max_capacity / (1024*1024));
                                                info!("  可用: {}MB", storage_info.free_space / (1024*1024));
                                            }
                                            
                                            // 获取根对象数量
                                            if let Ok(num_objects) = camera_guard.get_numobjects_roots(*storage_id, None, None).await {
                                                info!("  根目录对象数量: {}", num_objects);
                                                
                                                // 如果有对象，获取句柄
                                                if num_objects > 0 {
                                                    if let Ok(handles) = camera_guard.get_objecthandles_root(*storage_id, None, None).await {
                                                        info!("  发现 {} 个对象", handles.len());
                                                        
                                                        // 列出前5个对象的信息
                                                        let max_to_show = std::cmp::min(5, handles.len());
                                                        for j in 0..max_to_show {
                                                            let handle = handles[j];
                                                            info!("  对象 #{}: 句柄=0x{:08x}", j+1, handle);
                                                            
                                                            if let Ok(obj_info) = camera_guard.get_objectinfo(handle, None).await {
                                                                info!("    文件名: {}", obj_info.filename);
                                                                info!("    大小: {} 字节", obj_info.object_compressed_size);
                                                                info!("    类型: 0x{:04x}", obj_info.object_format);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        error!("获取存储ID失败: {}", e);
                                    }
                                }
                            },
                            Err(e) => {
                                error!("获取设备信息失败: {}", e);
                            }
                        }
                    }
                    
                    // 关闭会话
                    info!("关闭PTP会话...");
                    if let Err(e) = adapter.close_session().await {
                        error!("关闭PTP会话失败: {}", e);
                    }
                    
                    // 断开连接
                    info!("断开相机连接...");
                    adapter.disconnect().await;
                },
                Err(e) => {
                    error!("连接相机失败: {}", e);
                }
            }
        },
        Err(e) => {
            error!("创建PTP相机适配器失败: {}", e);
        }
    }
}

/// 运行PTP相机示例
pub fn run_ptp_camera_example() {
    info!("初始化PTP相机示例...");
    
    // 创建静态Embassy执行器
    static EXECUTOR: StaticCell<Executor> = StaticCell::new();
    let executor = EXECUTOR.init(Executor::new());
    
    // 运行执行器并启动相机任务
    info!("启动Embassy执行器...");
    executor.run(|spawner| {
        // 生成相机任务
        if let Err(e) = spawner.spawn(camera_task()) {
            error!("无法启动相机任务: {:?}", e);
        }
    });
}
