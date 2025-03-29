fn main() {
    // 初始化ESP32环境
    // ESP-IDF必要的运行时修补
    esp_idf_svc::sys::link_patches();
    
    // 初始化ESP日志功能
    esp_idf_svc::log::EspLogger::initialize_default();
    
    log::info!("正在启动ESP32相机边拍边传系统...");
    
    // 系统初始化流程
    match run_system() {
        Ok(_) => {
            log::info!("系统运行完成");
        },
        Err(e) => {
            log::error!("系统运行出错: {}", e);
        }
    }
}

/// 主系统流程
fn run_system() -> Result<(), Box<dyn std::error::Error>> {
    use rcamera::camera_connection::CameraDevice;
    use rcamera::ptp_mtp::{create_protocol_handler, ProtocolType, DataProcessor};
    use rcamera::wireless::{WirelessManager, ConnectionType, ConnectionConfig};
    use rcamera::data_transfer::TransferManager;
    
    // 步骤1：连接相机
    log::info!("正在连接相机设备...");
    // 这里需要替换为实际相机的VID和PID
    let mut camera = CameraDevice::new(0x04A9, 0x326F); // 示例: 佳能相机
    camera.connect()?;
    
    // 步骤2：初始化PTP/MTP协议
    log::info!("正在初始化PTP协议...");
    let camera_handle = camera.get_handle().ok_or("相机未连接")?;
    let mut protocol = create_protocol_handler(ProtocolType::PTP, camera_handle);
    protocol.init_session()?;
    
    // 获取相机信息
    let device_info = protocol.get_device_info()?;
    log::info!("已连接的相机: {} {}", device_info.manufacturer, device_info.model);
    
    // 步骤3：设置无线连接
    log::info!("正在初始化WiFi...");
    let mut wireless = WirelessManager::new(ConnectionType::WiFi);
    wireless.initialize()?;
    
    // 配置ESP32作为接入点
    let wifi_config = ConnectionConfig::WiFi(
        "ESP32Camera".into(), // SSID
        "123456".into()  // 密码
    );
    wireless.connect(&wifi_config)?;
    
    // 步骤4：创建数据传输管理器
    log::info!("正在初始化数据传输...");
    let mut transfer = TransferManager::new(10); // 缓冲区最多10个数据包
    
    // 开始数据流传输
    log::info!("正在启动相机实时数据流...");
    protocol.start_live_stream()?;
    
    // 开始数据传输
    transfer.start()?;
    log::info!("已开始边拍边传...");
    
    // 这里应该添加主循环逻辑，例如等待用户输入或事件
    // 在实际应用中，可能需要一个事件循环或任务调度器
    
    // 示例：睡眠一段时间模拟系统运行
    std::thread::sleep(std::time::Duration::from_secs(60));
    
    // 停止传输
    log::info!("正在停止传输...");
    transfer.stop()?;
    protocol.stop_live_stream()?;
    protocol.close_session()?;
    wireless.disconnect()?;
    camera.disconnect();
    
    log::info!("系统已安全关闭");
    Ok(())
}
