# r-camera (相机无线传输方案)

通过PTP和MTP实现边拍边传，并且通过手机连接esp32 的蓝牙/wifi，来传输边拍边传的数据.

## 目标

1. 给没有ftp无线传输方案的相机，带来无限传输的能力。
2. 卖钱，有利于打开市场，让摄像师进一步依赖像素芝士。
3. 期望解决当前不同相机复杂环境下出现的丢图，之类的疑难杂症?ca
4. 成本需要压缩到30块钱。

## 技术方案

1. 手机(web)与硬件通过 `蓝牙` 或者 `Wi-Fi` 进行通信, 实现自己的通信协议，实现 `断点续传`,`反压数据流`,`消息确认` 等功能。
2. 硬件通过 `PTP` 和 `MTP`协议与相机进行通信，实现 `文件传输`，`文件删除` 等功能。

### 开发环境

本项目采用 `esp32-c3` 作为无线传输模块,因此需要先安装该rust目标。

```bash
rustup target add riscv32imc-unknown-none-elf
```

ESP32 主要使用 ESP-IDF 或 ESP HAL 进行开发，需要安装 Xtensa 交叉编译工具链：

```bash
cargo install cargo-generate
cargo install ldproxy
cargo install espup
cargo install espflash
cargo install cargo-espflash # Optional
```

为 Espressif SoCs 安装 Rust & Clang 工具链.

```bash
espup install
```