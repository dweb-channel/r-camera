// 无线连接模块 - 负责ESP32与手机之间的蓝牙/WiFi通信
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_hal::peripheral;
use esp_idf_svc::bt::BtDriver;
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::EspWifi;
use log::{debug, info};
use std::error::Error;
use std::sync::{Arc, Mutex};
use esp_idf_svc::bt::{BtDriver, BleConnectParams, BleDevice, BleScanParams, BleService, BtUuid, EspBle};
use esp_idf_svc::bt::gatt_server::{GattServer, GattServiceEvent, GattServiceEventHandler};
use esp_idf_svc::bt::BtAddr;

// TODO
// pub mod wifi;
// pub mod bluetooth;
// pub mod server;

/// 无线连接类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionType {
    WiFi,
    Bluetooth,
}

/// 无线连接管理器
pub struct WirelessManager {
    conn_type: ConnectionType,
    wifi_driver: Option<EspWifi<'static>>,
    bt_driver: Option<BtDriver<'static>>,
    connected: bool,
}

impl WirelessManager {
    /// 创建新的无线连接管理器
    pub fn new(conn_type: ConnectionType) -> Self {
        WirelessManager {
            conn_type,
            wifi_driver: None,
            bt_driver: None,
            connected: false,
        }
    }

    /// 初始化无线连接
    pub fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        match self.conn_type {
            ConnectionType::WiFi => {
                self.init_wifi()?;
            }
            ConnectionType::Bluetooth => {
                self.init_bluetooth()?;
            }
        }
        Ok(())
    }

    /// 连接到网络或开启服务
    pub fn connect(&mut self, config: &ConnectionConfig) -> Result<(), Box<dyn Error>> {
        match self.conn_type {
            ConnectionType::WiFi => {
                if let Some(wifi) = &mut self.wifi_driver {
                    self.connect_wifi(wifi, config)?;
                } else {
                    return Err("WiFi驱动未初始化".into());
                }
            }
            ConnectionType::Bluetooth => {
                if self.bt_driver.is_some() {
                    self.start_bluetooth_server(config)?;
                } else {
                    return Err("蓝牙驱动未初始化".into());
                }
            }
        }

        self.connected = true;
        Ok(())
    }

    /// 断开连接
    pub fn disconnect(&mut self) -> Result<(), Box<dyn Error>> {
        match self.conn_type {
            ConnectionType::WiFi => {
                if let Some(wifi) = &mut self.wifi_driver {
                    wifi.stop()?;
                    info!("WiFi连接已断开");
                }
            }
            ConnectionType::Bluetooth => {
                // 停止蓝牙服务
                info!("蓝牙服务已停止");
            }
        }

        self.connected = false;
        Ok(())
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// 初始化WiFi
    fn init_wifi(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("初始化WiFi...");

        // 获取ESP32系统事件循环
        let sys_loop = EspSystemEventLoop::take()?;

        // 获取非易失性存储分区
        let nvs = EspDefaultNvsPartition::take()?;

        // 初始化WiFi驱动
        let wifi = EspWifi::new(
            unsafe { peripheral::Peripheral::new() },
            unsafe { peripheral::Peripheral::new() },
            // sys_loop,
            Some(nvs),
        )?;

        self.wifi_driver = Some(wifi);
        info!("WiFi初始化成功");

        Ok(())
    }

    /// 初始化蓝牙
    fn init_bluetooth(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("初始化蓝牙...");

        // 获取非易失性存储分区
        let nvs = EspDefaultNvsPartition::take()?;

        // 初始化蓝牙驱动
        let bt = BtDriver::new(unsafe { peripheral::Peripheral::new() })?;
        
        // 在这里检查蓝牙是否已启用
        if !bt.is_enabled() {
            info!("蓝牙未启用，尝试启用蓝牙...");
            // 尝试启用蓝牙
            bt.enable()?;
            
            if !bt.is_enabled() {
                return Err("无法启用蓝牙，请确保CONFIG_BT_ENABLED=y已配置".into());
            }
        }

        self.bt_driver = Some(bt);
        info!("蓝牙初始化成功");

        Ok(())
    }

    /// 连接到WiFi网络
    fn connect_wifi(
        &self,
        wifi: &mut EspWifi<'static>,
        config: &ConnectionConfig,
    ) -> Result<(), Box<dyn Error>> {
        if let ConnectionConfig::WiFi(ssid, pass) = config {
            debug!("连接到WiFi网络: {}", ssid);

            let wifi_config = Configuration::Client(ClientConfiguration {
                ssid: ssid.clone(),
                password: pass.clone(),
                auth_method: AuthMethod::WPA2Personal,
                ..Default::default()
            });

            wifi.set_configuration(&wifi_config)?;
            wifi.start()?;
            wifi.connect()?;

            info!("已连接到WiFi网络: {}", ssid);
            Ok(())
        } else {
            Err("无效的WiFi配置".into())
        }
    }

    /// 启动蓝牙服务器
    fn start_bluetooth_server(&self, config: &ConnectionConfig) -> Result<(), Box<dyn Error>> {
        if let ConnectionConfig::Bluetooth(device_name) = config {
            debug!("启动蓝牙服务: {}", device_name);

            // 获取蓝牙驱动的引用
            if let Some(bt_driver) = &self.bt_driver {
                // 创建BLE实例
                let mut ble = EspBle::new(device_name, bt_driver)?;
                
                // 设置设备为可发现模式
                ble.set_scan_rsp_data(&[
                    0x02, 0x01, 0x06,                               // 标志: LE General Discoverable Mode
                    0x03, 0x03, 0x00, 0x18,                         // 完整的16位UUID列表: 设备信息服务
                    device_name.len() as u8 + 1, 0x09               // 完整的本地名称
                ])?;
                
                // 设置广播数据
                ble.set_adv_data(&[
                    0x02, 0x01, 0x06,                               // 标志: LE General Discoverable Mode
                    0x03, 0x03, 0x00, 0x18,                         // 完整的16位UUID列表: 设备信息服务
                    device_name.len() as u8 + 1, 0x09               // 完整的本地名称
                ])?;
                
                // 开始广播
                ble.start_advertise()?;
                
                // 创建GATT服务器
                let gatt_server = GattServer::new()?;
                
                // 创建一个自定义服务
                let service_uuid = BtUuid::from_uuid16(0xFF00); // 自定义服务UUID
                let service = gatt_server.create_service(service_uuid, true)?;
                
                // 创建特征值
                let char_uuid = BtUuid::from_uuid16(0xFF01); // 自定义特征UUID
                let characteristic = service.create_characteristic(
                    char_uuid,
                    esp_idf_svc::bt::gatt_server::CharacteristicProperties::READ 
                    | esp_idf_svc::bt::gatt_server::CharacteristicProperties::WRITE 
                    | esp_idf_svc::bt::gatt_server::CharacteristicProperties::NOTIFY,
                    esp_idf_svc::bt::gatt_server::AttributePermissions::READABLE 
                    | esp_idf_svc::bt::gatt_server::AttributePermissions::WRITABLE,
                    None,
                )?;
                
                // 设置初始值
                characteristic.set_value(&[0x00])?;
                
                // 启动服务
                service.start()?;
                
                info!("蓝牙服务已启动: {}", device_name);
                Ok(())
            } else {
                Err("蓝牙驱动未初始化".into())
            }
        } else {
            Err("无效的蓝牙配置".into())
        }
    }
}

/// 连接配置
pub enum ConnectionConfig {
    WiFi(String, String), // SSID, 密码
    Bluetooth(String),    // 设备名称
}

/// 数据发送接口
pub trait DataSender {
    /// 发送数据
    fn send_data(&self, data: &[u8]) -> Result<usize, Box<dyn Error>>;

    /// 关闭发送器
    fn close(&mut self) -> Result<(), Box<dyn Error>>;
}

/// WiFi数据发送器
pub struct WifiSender {
    // TODO
}

/// 蓝牙数据发送器
pub struct BluetoothSender {
    // 蓝牙发送器的具体实现
    device_name: String,
    characteristic: Option<Arc<Mutex<esp_idf_svc::bt::gatt_server::Characteristic>>>,
}

impl BluetoothSender {
    /// 创建新的蓝牙发送器
    pub fn new(device_name: String, characteristic: Arc<Mutex<esp_idf_svc::bt::gatt_server::Characteristic>>) -> Self {
        BluetoothSender {
            device_name,
            characteristic: Some(characteristic),
        }
    }
}

impl DataSender for WifiSender {
    fn send_data(&self, data: &[u8]) -> Result<usize, Box<dyn Error>> {
        // 通过WiFi发送数据
        debug!("通过WiFi发送{}字节的数据", data.len());
        Ok(data.len())
    }

    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        // 关闭WiFi发送器
        Ok(())
    }
}

impl DataSender for BluetoothSender {
    fn send_data(&self, data: &[u8]) -> Result<usize, Box<dyn Error>> {
        // 通过蓝牙发送数据
        debug!("通过蓝牙发送{}字节的数据", data.len());
        
        if let Some(char_ref) = &self.characteristic {
            // 获取特征值的锁
            if let Ok(mut characteristic) = char_ref.lock() {
                // 设置特征值
                characteristic.set_value(data)?;
                
                // 发送通知
                characteristic.notify(None)?;
                
                return Ok(data.len());
            }
        }
        
        Err("蓝牙特征值未初始化或无法访问".into())
    }

    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        // 关闭蓝牙发送器
        debug!("关闭蓝牙发送器: {}", self.device_name);
        
        // 释放特征值引用
        self.characteristic = None;
        
        Ok(())
    }
}
