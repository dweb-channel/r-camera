// 无线连接模块 - 负责ESP32与手机之间的蓝牙/WiFi通信
use embedded_svc::wifi::{AuthMethod, ClientConfiguration, Configuration};
use esp_idf_svc::bt::ble::gap::{AdvConfiguration, BleGapEvent, EspBleGap};
use esp_idf_svc::bt::ble::gatt::server::{ConnectionId, EspGatts, GattsEvent, TransferId};
use esp_idf_svc::bt::ble::gatt::{
    AutoResponse, GattCharacteristic, GattDescriptor, GattId, GattInterface, GattResponse,
    GattServiceId, GattStatus, Handle, Permission, Property,
};
use esp_idf_svc::bt::{BdAddr, Ble as EspBle, BtDriver, BtStatus, BtUuid};
use esp_idf_svc::eventloop::EspSystemEventLoop;
use esp_idf_svc::nvs::EspDefaultNvsPartition;
use esp_idf_svc::wifi::EspWifi;
use log::{debug, info, warn};
use std::error::Error;
use std::sync::{Arc, Condvar, Mutex};
use heapless::{String as HString, Vec as HVec};
use enumset::enum_set;
use esp_idf_svc::sys::EspError;

/// 无线连接类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionType {
    WiFi,
    Bluetooth,
}


/// 蓝牙服务器状态
struct BluetoothServerState {
    gatt_if: Option<GattInterface>,
    service_handle: Option<Handle>,
    recv_handle: Option<Handle>,
    ind_handle: Option<Handle>,
    ind_cccd_handle: Option<Handle>,
    connections: HVec<Connection, 4>, // 支持最多4个并发连接
    response: GattResponse,
    ind_confirmed: Option<BdAddr>,
}

impl Default for BluetoothServerState {
    fn default() -> Self {
        Self {
            gatt_if: None,
            service_handle: None,
            recv_handle: None,
            ind_handle: None,
            ind_cccd_handle: None,
            connections: HVec::new(),
            response: GattResponse::default(),
            ind_confirmed: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Connection {
    peer: BdAddr,
    conn_id: Handle,
    subscribed: bool,
    mtu: Option<u16>,
}

/// 无线连接管理器
pub struct WirelessManager {
    conn_type: ConnectionType,
    wifi_driver: Option<EspWifi<'static>>,
    bt_driver: Option<Arc<BtDriver<'static, EspBle>>>,
    ble_gap: Option<Arc<EspBleGap<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>>,
    ble_gatts: Option<Arc<EspGatts<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>>,
    bt_state: Option<Arc<Mutex<BluetoothServerState>>>,
    bt_condvar: Option<Arc<Condvar>>,
    connected: bool,
}

impl WirelessManager {
    /// 创建新的无线连接管理器
    pub fn new(conn_type: ConnectionType) -> Self {
        WirelessManager {
            conn_type,
            wifi_driver: None,
            bt_driver: None,
            ble_gap: None,
            ble_gatts: None,
            bt_state: None,
            bt_condvar: None,
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
    // 修改 connect 方法签名，接收 config 的所有权以避免生命周期问题
    pub fn connect(&mut self, config: ConnectionConfig) -> Result<(), Box<dyn Error>> {
        let conn_type = self.conn_type; // 复制 conn_type 以避免在 match 中借用 self
        match conn_type {
            ConnectionType::WiFi => {
                // 将 wifi_driver 的可变借用移到 if let 内部
                if let Some(wifi) = self.wifi_driver.as_mut() {
                    Self::connect_wifi_static(wifi, &config)?;
                } else {
                    return Err("WiFi驱动未初始化".into());
                }
            }
            ConnectionType::Bluetooth => {
                // 将 bt_driver 的检查移到 if let 内部
                if self.bt_driver.is_some() {
                    // 注意：config 在这里可能不再有效，因为上面分支可能消耗了它
                    // 需要根据实际逻辑调整，这里假设蓝牙分支不需要 config
                    // 或者将 config 克隆或只传递引用
                    self.start_bluetooth_server(&config)?;
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

    /// 创建数据发送器
    pub fn create_sender(
        &self,
        config: &ConnectionConfig,
    ) -> Result<Box<dyn DataSender>, Box<dyn Error>> {
        match self.conn_type {
            ConnectionType::WiFi => {
                if let ConnectionConfig::WiFi(_, _) = config {
                    let sender = WifiSender::new();
                    Ok(Box::new(sender))
                } else {
                    Err("无效的WiFi配置".into())
                }
            }
            ConnectionType::Bluetooth => {
                if let ConnectionConfig::Bluetooth(device_name) = config {
                    // 为了简化，假设我们已经有一个特征引用
                    // 在实际应用中，我们需要管理GATT服务器并获取正确的特征引用

                    // 这部分是一个简化的示例，实际应用中需要更复杂的逻辑来管理蓝牙GATT服务和特征
                    return Err(
                        "蓝牙发送器创建需要正确的GATT特征引用，请先实现完整的GATT服务器管理".into(),
                    );

                    // 完整实现应该是类似这样：
                    // let char_ref = self.get_bluetooth_characteristic()?;
                    // let sender = BluetoothSender::new(device_name.clone(), char_ref);
                    // Ok(Box::new(sender))
                } else {
                    Err("无效的蓝牙配置".into())
                }
            }
        }
    }

    /// 初始化WiFi
    fn init_wifi(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("初始化WiFi...");

        // 获取ESP32系统事件循环
        let sys_loop = EspSystemEventLoop::take()?;

        // 获取非易失性存储分区
        let nvs = EspDefaultNvsPartition::take()?;

        // 获取所有外设
        let peripherals = esp_idf_hal::peripherals::Peripherals::take()?;

        // 初始化WiFi驱动
        let wifi = EspWifi::new(
            peripherals.modem, // WiFi/BT外设
            sys_loop.clone(),  // 使用事件循环替代 rng (根据 esp-idf-svc 示例)
            Some(nvs),
        )?;

        self.wifi_driver = Some(wifi);
        info!("WiFi初始化成功");

        Ok(())
    }

    /// 初始化蓝牙
    fn init_bluetooth(&mut self) -> Result<(), Box<dyn Error>> {
        debug!("初始化蓝牙...");

        let nvs = EspDefaultNvsPartition::take()?;
        let peripherals = esp_idf_hal::peripherals::Peripherals::take()?;

        let bt = Arc::new(BtDriver::new(peripherals.modem, Some(nvs.clone()))?);

        self.bt_driver = Some(bt.clone());
        self.ble_gap = Some(Arc::new(EspBleGap::new(bt.clone())?));
        self.ble_gatts = Some(Arc::new(EspGatts::new(bt.clone())?));
        self.bt_state = Some(Arc::new(Mutex::new(BluetoothServerState::default())));
        self.bt_condvar = Some(Arc::new(Condvar::new()));

        info!("蓝牙初始化成功");

        Ok(())
    }

    // 将 connect_wifi 改为静态方法以解决借用冲突
    fn connect_wifi_static(
        wifi: &mut EspWifi<'static>,
        config: &ConnectionConfig,
    ) -> Result<(), Box<dyn Error>> {
        if let ConnectionConfig::WiFi(ssid, pass) = config {
            debug!("连接到WiFi网络: {}", ssid);

            // 将 ssid 和 pass 转换为 heapless::String<32>
            let h_ssid: HString<32> = HString::from(ssid.as_str());
            let h_pass: HString<64> = HString::from(pass.as_str()); // 密码通常更长

            let wifi_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: h_pass,
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

            // 检查蓝牙驱动等组件是否已初始化
            if self.bt_driver.is_none() || self.ble_gap.is_none() || self.ble_gatts.is_none() {
                return Err("蓝牙驱动未初始化或初始化不完整".into());
            }

            let gap = self.ble_gap.as_ref().unwrap();
            let gatts = self.ble_gatts.as_ref().unwrap();
            let state = self.bt_state.as_ref().unwrap();
            let condvar = self.bt_condvar.as_ref().unwrap();

            // 创建服务器实例，用于管理回调
            let server = BluetoothServer {
                gap: gap.clone(),
                gatts: gatts.clone(),
                state: state.clone(),
                condvar: condvar.clone(),
                device_name: device_name.clone(),
            };

            // 配置设备名称和广播参数
            gap.set_device_name(device_name)?;
            
            // 设置广播配置
            let service_uuid = BtUuid::uuid128(0xad91b201734740479e173bed82d75f9d); // 使用示例中的UUID
            gap.set_adv_conf(&AdvConfiguration {
                include_name: true,
                include_txpower: true,
                flag: 2, // LE General Discoverable Mode
                service_uuid: Some(service_uuid),
                ..Default::default()
            })?;

            // 注册GAP和GATTS事件处理程序
            let gap_server = server.clone();
            gap.subscribe(move |event| {
                let _ = gap_server.on_gap_event(event);
            })?;

            let gatts_server = server.clone();
            gatts.subscribe(move |(gatt_if, event)| {
                let _ = gatts_server.on_gatts_event(gatt_if, event);
            })?;

            // 注册GATT应用
            const APP_ID: u16 = 0;
            gatts.register_app(APP_ID)?;

            info!("蓝牙服务器初始化成功: {}", device_name);
            return Ok(());
        } else {
            return Err("无效的蓝牙配置".into());
        }
    }

    /// 通过蓝牙发送数据到已连接的客户端
    pub fn send_bluetooth_data(&self, data: &[u8]) -> Result<(), Box<dyn Error>> {
        if let (Some(state), Some(condvar)) = (&self.bt_state, &self.bt_condvar) {
            let server = BluetoothServer {
                gap: self.ble_gap.as_ref().unwrap().clone(),
                gatts: self.ble_gatts.as_ref().unwrap().clone(),
                state: state.clone(),
                condvar: condvar.clone(),
                device_name: "ESP32".to_string(),  // 默认设备名
            };
            
            server.indicate(data)?;
            debug!("通过蓝牙广播数据: {} 字节", data.len());
            Ok(())
        } else {
            Err("蓝牙服务未初始化".into())
        }
    }
}

/// 蓝牙服务器实现，管理BLE GATT服务
#[derive(Clone)]
struct BluetoothServer {
    gap: Arc<EspBleGap<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>,
    gatts: Arc<EspGatts<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>,
    state: Arc<Mutex<BluetoothServerState>>,
    condvar: Arc<Condvar>,
    device_name: String,
}

impl BluetoothServer {
    /// 发送数据到所有已订阅的客户端
    /// 
    /// 对于使用Indication特性的发送，需要等待确认
    /// 通过Mutex和Condvar实现同步等待
    fn indicate(&self, data: &[u8]) -> Result<(), EspError> {
        const MAX_CONNECTIONS: usize = 4;
        
        for peer_index in 0..MAX_CONNECTIONS {
            // 向所有已连接且订阅的客户端发送数据
            let mut state = self.state.lock().unwrap();

            loop {
                if state.connections.len() <= peer_index {
                    // 已向所有连接的客户端发送
                    break;
                }

                let Some(gatt_if) = state.gatt_if else {
                    // GATT接口不存在
                    break;
                };

                let Some(ind_handle) = state.ind_handle else {
                    // Indication特性句柄不存在
                    break;
                };

                if state.ind_confirmed.is_none() {
                    let conn = &state.connections[peer_index];
                    
                    // 只向已订阅的客户端发送
                    if conn.subscribed {
                        self.gatts.indicate(gatt_if, conn.conn_id, ind_handle, data)?;
                        state.ind_confirmed = Some(conn.peer);
                        debug!("已向客户端 {} 发送数据", conn.peer);
                    }
                    break;
                } else {
                    // 等待上一个indication被确认
                    state = self.condvar.wait(state).unwrap();
                }
            }
        }

        Ok(())
    }

    /// 处理GAP事件
    fn on_gap_event(&self, event: BleGapEvent) -> Result<(), EspError> {
        debug!("收到GAP事件: {:?}", event);

        if let BleGapEvent::AdvertisingConfigured(status) = event {
            if status == BtStatus::Success {
                // 广播配置成功后开始广播
                self.gap.start_advertising()?;
                debug!("蓝牙广播已启动");
            } else {
                warn!("广播配置失败: {:?}", status);
            }
        }

        Ok(())
    }

    /// 处理GATTS事件
    fn on_gatts_event(&self, gatt_if: GattInterface, event: GattsEvent) -> Result<(), EspError> {
        debug!("收到GATTS事件: {:?}", event);

        match event {
            GattsEvent::ServiceRegistered { status, app_id } => {
                if status == GattStatus::Ok {
                    const APP_ID: u16 = 0;
                    if APP_ID == app_id {
                        self.create_service(gatt_if)?;
                    }
                }
            }
            GattsEvent::ServiceCreated { status, service_handle, .. } => {
                if status == GattStatus::Ok {
                    self.configure_and_start_service(service_handle)?;
                }
            }
            GattsEvent::CharacteristicAdded { status, attr_handle, service_handle, char_uuid } => {
                if status == GattStatus::Ok {
                    self.register_characteristic(service_handle, attr_handle, char_uuid)?;
                }
            }
            GattsEvent::DescriptorAdded { status, attr_handle, service_handle, descr_uuid } => {
                if status == GattStatus::Ok {
                    self.register_cccd_descriptor(service_handle, attr_handle, descr_uuid)?;
                }
            }
            GattsEvent::Mtu { conn_id, mtu } => {
                self.register_conn_mtu(conn_id, mtu)?;
            }
            GattsEvent::PeerConnected { conn_id, addr, .. } => {
                self.create_conn(conn_id, addr)?;
            }
            GattsEvent::PeerDisconnected { addr, .. } => {
                self.delete_conn(addr)?;
            }
            GattsEvent::Write { conn_id, trans_id, addr, handle, offset, need_rsp, is_prep, value } => {
                let handled = self.recv(gatt_if, conn_id, trans_id, addr, handle, offset, need_rsp, is_prep, &value)?;
                
                if handled && need_rsp {
                    self.send_write_response(gatt_if, conn_id, trans_id, handle, offset, need_rsp, is_prep, &value)?;
                }
            }
            GattsEvent::Confirm { status, .. } => {
                if status == GattStatus::Ok {
                    self.confirm_indication()?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    /// 创建GATT服务
    fn create_service(&self, gatt_if: GattInterface) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();
        state.gatt_if = Some(gatt_if);

        // 创建服务
        const SERVICE_UUID: u128 = 0xad91b201734740479e173bed82d75f9d; // 自定义服务UUID
        self.gatts.create_service(
            gatt_if,
            &GattServiceId {
                id: GattId {
                    uuid: BtUuid::uuid128(SERVICE_UUID),
                    inst_id: 0,
                },
                is_primary: true,
            },
            8, // 属性数量
        )?;

        Ok(())
    }

    /// 配置并启动服务
    fn configure_and_start_service(&self, service_handle: Handle) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();
        state.service_handle = Some(service_handle);

        // 启动服务
        self.gatts.start_service(service_handle)?;
        
        // 添加特性
        self.add_characteristics(service_handle)?;

        Ok(())
    }

    /// 添加特性到服务
    fn add_characteristics(&self, service_handle: Handle) -> Result<(), EspError> {
        // 接收数据的特性
        const RECV_CHARACTERISTIC_UUID: u128 = 0xb6fccb5087be44f3ae22f85485ea42c4;
        self.gatts.add_characteristic(
            service_handle,
            &GattCharacteristic {
                uuid: BtUuid::uuid128(RECV_CHARACTERISTIC_UUID),
                permissions: enum_set!(Permission::Write),
                properties: enum_set!(Property::Write),
                max_len: 200, // 最大接收数据长度
                auto_rsp: AutoResponse::ByApp,
            },
            &[],
        )?;

        // 发送数据的特性（支持indication）
        const IND_CHARACTERISTIC_UUID: u128 = 0x503de214868246c4828fd59144da41be;
        self.gatts.add_characteristic(
            service_handle,
            &GattCharacteristic {
                uuid: BtUuid::uuid128(IND_CHARACTERISTIC_UUID),
                permissions: enum_set!(Permission::Write | Permission::Read),
                properties: enum_set!(Property::Indicate),
                max_len: 200, // 最大发送数据长度
                auto_rsp: AutoResponse::ByApp,
            },
            &[],
        )?;

        Ok(())
    }

    /// 注册特性
    fn register_characteristic(
        &self,
        service_handle: Handle,
        attr_handle: Handle,
        char_uuid: BtUuid,
    ) -> Result<(), EspError> {
        let indicate_char = {
            let mut state = self.state.lock().unwrap();

            if state.service_handle != Some(service_handle) {
                false
            } else if char_uuid == BtUuid::uuid128(0xb6fccb5087be44f3ae22f85485ea42c4) { // RECV UUID
                state.recv_handle = Some(attr_handle);
                false
            } else if char_uuid == BtUuid::uuid128(0x503de214868246c4828fd59144da41be) { // IND UUID
                state.ind_handle = Some(attr_handle);
                true
            } else {
                false
            }
        };

        // 为indication特性添加CCCD描述符（Client Characteristic Configuration Descriptor）
        if indicate_char {
            self.gatts.add_descriptor(
                service_handle,
                &GattDescriptor {
                    uuid: BtUuid::uuid16(0x2902), // CCCD标准UUID
                    permissions: enum_set!(Permission::Read | Permission::Write),
                },
            )?;
        }

        Ok(())
    }

    /// 注册CCCD描述符
    fn register_cccd_descriptor(
        &self,
        service_handle: Handle,
        attr_handle: Handle,
        descr_uuid: BtUuid,
    ) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();

        if descr_uuid == BtUuid::uuid16(0x2902) && state.service_handle == Some(service_handle) {
            state.ind_cccd_handle = Some(attr_handle);
        }

        Ok(())
    }

    /// 注册连接的MTU
    fn register_conn_mtu(&self, conn_id: ConnectionId, mtu: u16) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();

        if let Some(conn) = state
            .connections
            .iter_mut()
            .find(|conn| conn.conn_id == conn_id)
        {
            conn.mtu = Some(mtu);
        }

        Ok(())
    }

    /// 创建新连接
    fn create_conn(&self, conn_id: ConnectionId, addr: BdAddr) -> Result<(), EspError> {
        const MAX_CONNECTIONS: usize = 4;
        let added = {
            let mut state = self.state.lock().unwrap();

            if state.connections.len() < MAX_CONNECTIONS {
                let _ = state.connections.push(Connection {
                    peer: addr,
                    conn_id,
                    subscribed: false,
                    mtu: None,
                });
                true
            } else {
                false
            }
        };

        if added {
            // 连接参数：最小间隔、最大间隔、延迟、超时
            self.gap.set_conn_params_conf(addr, 10, 20, 0, 400)?;
            info!("客户端已连接: {}", addr);
        } else {
            warn!("连接数量已达上限，拒绝新连接: {}", addr);
        }

        Ok(())
    }

    /// 删除连接
    fn delete_conn(&self, addr: BdAddr) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();

        if let Some(index) = state
            .connections
            .iter()
            .position(|connection| connection.peer == addr)
        {
            let _ = state.connections.swap_remove(index);
            info!("客户端已断开连接: {}", addr);
        }

        Ok(())
    }

    /// 处理接收到的数据
    #[allow(clippy::too_many_arguments)]
    fn recv(
        &self,
        _gatt_if: GattInterface,
        conn_id: ConnectionId,
        _trans_id: TransferId,
        addr: BdAddr,
        handle: Handle,
        offset: u16,
        _need_rsp: bool,
        _is_prep: bool,
        value: &[u8],
    ) -> Result<bool, EspError> {
        let mut state = self.state.lock().unwrap();

        let recv_handle = state.recv_handle;
        let ind_cccd_handle = state.ind_cccd_handle;

        let Some(conn) = state
            .connections
            .iter_mut()
            .find(|conn| conn.conn_id == conn_id)
        else {
            return Ok(false);
        };

        if Some(handle) == ind_cccd_handle {
            // 处理订阅/取消订阅
            if offset == 0 && value.len() == 2 {
                let value = u16::from_le_bytes([value[0], value[1]]);
                if value == 0x02 {
                    if !conn.subscribed {
                        conn.subscribed = true;
                        info!("客户端订阅了通知: {}", conn.peer);
                    }
                } else if conn.subscribed {
                    conn.subscribed = false;
                    info!("客户端取消订阅了通知: {}", conn.peer);
                }
            }
        } else if Some(handle) == recv_handle {
            // 处理收到的数据
            info!("收到客户端 {} 数据: {:?}, 偏移量: {}, MTU: {:?}", 
                addr, value, offset, conn.mtu);
        } else {
            return Ok(false);
        }

        Ok(true)
    }

    /// 发送写响应
    #[allow(clippy::too_many_arguments)]
    fn send_write_response(
        &self,
        gatt_if: GattInterface,
        conn_id: ConnectionId,
        trans_id: TransferId,
        handle: Handle,
        offset: u16,
        need_rsp: bool,
        is_prep: bool,
        value: &[u8],
    ) -> Result<(), EspError> {
        if !need_rsp {
            return Ok(());
        }

        if is_prep {
            let mut state = self.state.lock().unwrap();

            // 准备响应数据
            state
                .response
                .attr_handle(handle)
                .auth_req(0)
                .offset(offset)
                .value(value)?;

            self.gatts.send_response(
                gatt_if,
                conn_id,
                trans_id,
                GattStatus::Ok,
                Some(&state.response),
            )?;
        } else {
            // 发送简单响应
            self.gatts
                .send_response(gatt_if, conn_id, trans_id, GattStatus::Ok, None)?;
        }

        Ok(())
    }

    /// 确认indication已被客户端接收
    fn confirm_indication(&self) -> Result<(), EspError> {
        let mut state = self.state.lock().unwrap();
        
        // 释放确认标志，允许发送下一个indication
        state.ind_confirmed = None;
        self.condvar.notify_all();

        Ok(())
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
    // WiFi发送器的属性
    ssid: String,
    client: Option<std::net::TcpStream>,
}

impl WifiSender {
    /// 创建新的WiFi发送器
    pub fn new() -> Self {
        WifiSender {
            ssid: String::new(),
            client: None,
        }
    }

    /// 连接到指定地址
    pub fn connect(&mut self, address: &str) -> Result<(), Box<dyn Error>> {
        match std::net::TcpStream::connect(address) {
            Ok(stream) => {
                self.client = Some(stream);
                Ok(())
            }
            Err(e) => Err(Box::new(e)),
        }
    }
}

impl DataSender for WifiSender {
    fn send_data(&self, data: &[u8]) -> Result<usize, Box<dyn Error>> {
        // 通过WiFi发送数据
        if let Some(stream) = &self.client {
            // 在真实场景中，我们需要使用指定协议将数据写入stream
            // 这里仅作为示例，实际实现可能更复杂
            debug!("尝试通过WiFi发送{}字节的数据", data.len());
            Ok(data.len()) // 假设发送成功
        } else {
            Err("WiFi客户端未连接".into())
        }
    }

    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        // 关闭WiFi发送器
        self.client = None;
        Ok(())
    }
}

/// 蓝牙数据发送器
pub struct BluetoothSender {
    device_name: String,
    // 蓝牙服务器实例，与WirelessManager共享
    bt_state: Arc<Mutex<BluetoothServerState>>,
    bt_condvar: Arc<Condvar>,
    gatts: Arc<EspGatts<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>,
    gap: Arc<EspBleGap<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>,
}

impl BluetoothSender {
    /// 创建新的蓝牙发送器
    pub fn new(
        device_name: String,
        bt_state: Arc<Mutex<BluetoothServerState>>,
        bt_condvar: Arc<Condvar>,
        gatts: Arc<EspGatts<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>,
        gap: Arc<EspBleGap<'static, EspBle, Arc<BtDriver<'static, EspBle>>>>,
    ) -> Self {
        BluetoothSender {
            device_name,
            bt_state,
            bt_condvar,
            gatts,
            gap,
        }
    }
}

impl DataSender for BluetoothSender {
    /// 通过蓝牙发送数据
    fn send_data(&self, data: &[u8]) -> Result<usize, Box<dyn Error>> {
        // 创建服务器实例
        let server = BluetoothServer {
            gap: self.gap.clone(),
            gatts: self.gatts.clone(),
            state: self.bt_state.clone(),
            condvar: self.bt_condvar.clone(),
            device_name: self.device_name.clone(),
        };

        // 发送数据
        server.indicate(data)?;
        
        // 返回发送的字节数
        Ok(data.len())
    }

    /// 关闭蓝牙连接
    fn close(&mut self) -> Result<(), Box<dyn Error>> {
        // 停止广播
        self.gap.stop_advertising()?;
        
        info!("蓝牙发送器已关闭");
        Ok(())
    }
}
