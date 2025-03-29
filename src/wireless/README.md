### 使用方法
 
#### 初始化无线连接管理器：

```rust
let mut wireless = WirelessManager::new(ConnectionType::Bluetooth);
wireless.initialize()?;
```

#### 启动蓝牙服务：

```rust
let config = ConnectionConfig::Bluetooth("ESP32-Camera".to_string());
wireless.connect(&config)?;
```

#### 发送数据到已订阅的客户端：

```rust
let data = [0, 1, 2, 3];
wireless.send_bluetooth_data(&data)?;
```