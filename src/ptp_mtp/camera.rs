#![allow(non_snake_case)]

use std::cmp::min;
use std::slice;
use std::time::Duration;
use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};
use std::io::Cursor;

// Embassy相关导入
use embassy_usb::host::{UsbDevice, ConfigDescriptor};
use embassy_time::{Duration as EmbassyDuration, Timer};

use crate::ptp_mtp::error::Error;
use crate::ptp_mtp::standard_codes::{CommandCode, StandardCommandCode, StandardResponseCode, PtpContainerType};
use crate::ptp_mtp::device_info::{PtpDeviceInfo, PtpObjectInfo, PtpStorageInfo};
use crate::ptp_mtp::data_types::PtpRead;
use crate::camera_connection::CameraError;

/// PTP容器信息结构体
#[derive(Debug)]
struct PtpContainerInfo {
    /// 负载长度(字节)，通常与数据阶段相关
    payload_len: usize,

    /// 容器类型
    kind: PtpContainerType,

    /// 命令码或响应码，取决于容器类型
    code: u16,

    /// 此容器所属的事务ID
    tid: u32,
}

/// PTP容器信息头大小(字节)
const PTP_CONTAINER_INFO_SIZE: usize = 12;

impl PtpContainerInfo {
    /// 从数据流解析PTP容器信息
    pub fn parse<R: ReadBytesExt>(mut r: R) -> Result<PtpContainerInfo, Error> {
        let len = r.read_u32::<LittleEndian>()?;
        let kind_u16 = r.read_u16::<LittleEndian>()?;
        let kind = PtpContainerType::from_u16(kind_u16)
            .ok_or_else(|| Error::Malformed(format!("无效的消息类型 {:x}。", kind_u16)))?;
        let code = r.read_u16::<LittleEndian>()?;
        let tid = r.read_u32::<LittleEndian>()?;

        Ok(PtpContainerInfo {
            payload_len: len as usize - PTP_CONTAINER_INFO_SIZE,
            kind: kind,
            tid: tid,
            code: code,
        })
    }

    /// 检查此容器是否属于给定的事务
    pub fn belongs_to(&self, tid: u32) -> bool {
        self.tid == tid
    }
}

/// PTP相机类
pub struct PtpCamera {
    iface: u8,                      // 接口号
    ep_in: u8,                      // 输入端点
    ep_out: u8,                     // 输出端点
    _ep_int: u8,                    // 中断端点
    current_tid: u32,               // 当前事务ID
    handle: UsbDevice<'static>,     // Embassy-USB设备句柄
}

impl PtpCamera {
    /// 创建新的PTP相机实例
    /// 
    /// 使用异步API从USB设备初始化PTP相机
    pub async fn new(device: UsbDevice<'static>) -> Result<PtpCamera, Error> {
        // 获取配置描述符
        let config = device.current_config_descriptor().await;
        
        // 查找PTP/MTP接口（类代码为6）
        let mut interface_number = 0;
        let mut interface_found = false;
        let mut ep_in = 0;
        let mut ep_out = 0;
        let mut ep_int = 0;
        
        // 遍历所有接口查找PTP/MTP接口
        for iface in config.interfaces() {
            for alt_setting in iface.alt_settings() {
                if alt_setting.class_code() == 6 {  // PTP/MTP类代码
                    interface_number = iface.interface_number();
                    interface_found = true;
                    
                    // 查找端点
                    for endpoint in alt_setting.endpoints() {
                        let addr = endpoint.address();
                        
                        // 根据端点类型和方向分配
                        if endpoint.transfer_type() == embassy_usb::host::TransferType::Bulk {
                            if endpoint.direction() == embassy_usb::host::Direction::In {
                                ep_in = addr;
                            } else {
                                ep_out = addr;
                            }
                        } else if endpoint.transfer_type() == embassy_usb::host::TransferType::Interrupt
                                  && endpoint.direction() == embassy_usb::host::Direction::In {
                            ep_int = addr;
                        }
                    }
                    break;
                }
            }
            if interface_found {
                break;
            }
        }
        
        if !interface_found {
            return Err(Error::NotFound("未找到PTP/MTP接口".into()));
        }
        
        // 确保找到了必要的端点
        if ep_in == 0 || ep_out == 0 {
            return Err(Error::NotFound("未找到必要的端点".into()));
        }
        
        // 声明接口
        device.claim_interface(interface_number).await
              .map_err(|e| Error::USB(format!("无法声明接口: {:?}", e)))?;
        
        log::debug!("已找到并声明PTP/MTP接口 {}", interface_number);
        
        // 创建PTP相机实例
        Ok(PtpCamera {
            iface: interface_number,
            ep_in,
            ep_out,
            _ep_int: ep_int,
            current_tid: 0,
            handle: device,
        })
    }

    /// 执行PTP事务
    /// 包含以下阶段:
    ///  - 命令阶段
    ///  - 命令数据阶段 (可选，如果`data`为Some)
    ///  - 响应数据阶段 (可选，如果响应包含有效载荷)
    ///  - 响应状态阶段
    /// 注意: 每个阶段都涉及一个独立的USB传输，`timeout`用于每个阶段，
    /// 所以总时间可能会超过`timeout`。
    pub async fn command(
        &mut self,
        code: CommandCode,
        params: &[u32],
        data: Option<&[u8]>,
        timeout: Option<Duration>
    ) -> Result<Vec<u8>, Error> {

        // 超时为0表示无限超时
        let timeout = timeout.unwrap_or(Duration::new(0, 0));

        // 获取事务ID并增加计数器
        let tid = self.current_tid;
        self.current_tid += 1;

        // 准备请求阶段的有效载荷，包含参数
        let mut request_payload = Vec::with_capacity(params.len() * 4);
        for p in params {
            request_payload.write_u32::<LittleEndian>(*p).ok();
        }

        // 写入事务的命令阶段
        self.write_txn_phase(PtpContainerType::Command, code, tid, &request_payload, timeout).await?;

        // 如果有数据，写入数据阶段
        if let Some(data) = data {
            self.write_txn_phase(PtpContainerType::Data, code, tid, data, timeout).await?;
        }

        // 命令阶段之后是数据阶段(可选)和响应阶段
        // 读取这两个阶段，检查响应的状态，并返回数据载荷(如果有)
        let mut data_phase_payload = vec![];
        loop {
            let (container, payload) = self.read_txn_phase(timeout).await?;
            if !container.belongs_to(tid) {
                return Err(Error::Malformed(format!("事务ID不匹配，收到{}，期望{}", container.tid, tid)));
            }
            match container.kind {
                PtpContainerType::Data => {
                    data_phase_payload = payload;
                },
                PtpContainerType::Response => {
                    if container.code != StandardResponseCode::Ok {
                        return Err(Error::Response(container.code));
                    }
                    return Ok(data_phase_payload);
                },
                _ => {}
            }
        }
    }

    /// 写入事务阶段
    async fn write_txn_phase(&mut self, kind: PtpContainerType, code: CommandCode, tid: u32, payload: &[u8], timeout: Duration) -> Result<(), Error> {
        log::trace!("写入 {:?} - 0x{:04x} ({}), tid:{}", kind, code, StandardCommandCode::name(code).unwrap_or("未知"), tid);

        // 块大小，必须是端点包大小的倍数
        const CHUNK_SIZE: usize = 1024 * 1024; // 1MB

        // 第一个块包含头信息，其载荷必须被复制到临时缓冲区
        let first_chunk_payload_bytes = min(payload.len(), CHUNK_SIZE - PTP_CONTAINER_INFO_SIZE);
        let mut buf = Vec::with_capacity(first_chunk_payload_bytes + PTP_CONTAINER_INFO_SIZE);
        
        // 写入PTP头信息
        buf.write_u32::<LittleEndian>((payload.len() + PTP_CONTAINER_INFO_SIZE) as u32).ok();
        buf.write_u16::<LittleEndian>(kind as u16).ok();
        buf.write_u16::<LittleEndian>(code).ok();
        buf.write_u32::<LittleEndian>(tid).ok();
        
        // 添加载荷的第一部分
        buf.extend_from_slice(&payload[..first_chunk_payload_bytes]);
        
        // 转换为Embassy Duration
        let embassy_timeout = EmbassyDuration::from_millis(timeout.as_millis() as u64);
        
        // 使用Embassy的异步API进行批量写入
        self.handle.bulk_out(self.ep_out, &buf, embassy_timeout).await
            .map_err(|e| Error::USB(format!("批量写入失败: {:?}", e)))?;

        // 写入后续块，直接从源切片读取
        for chunk in payload[first_chunk_payload_bytes..].chunks(CHUNK_SIZE) {
            self.handle.bulk_out(self.ep_out, chunk, embassy_timeout).await
                .map_err(|e| Error::USB(format!("批量写入失败: {:?}", e)))?;
        }

        Ok(())
    }

    /// 读取事务阶段的辅助方法
    async fn read_txn_phase(&mut self, timeout: Duration) -> Result<(PtpContainerInfo, Vec<u8>), Error> {
        // 为读取分配缓冲区
        let mut buffer = [0u8; 8 * 1024]; // 8KB缓冲区
        
        // 转换为Embassy Duration
        let embassy_timeout = EmbassyDuration::from_millis(timeout.as_millis() as u64);
        
        // 使用Embassy的异步API进行批量读取
        let n = self.handle.bulk_in(self.ep_in, &mut buffer, embassy_timeout).await
            .map_err(|e| Error::USB(format!("批量读取失败: {:?}", e)))?;
            
        // 复制数据以便进一步处理
        let buf = &buffer[..n];

        // 解析容器信息
        let cinfo = PtpContainerInfo::parse(&buf[..])?;
        trace!("容器 {:?}", cinfo);

        // 没有载荷？结束了
        if cinfo.payload_len == 0 {
            return Ok((cinfo, vec![]));
        }

        // 分配足够的空间，多分配1个避免为尾部短包再读一次
        let mut payload = Vec::with_capacity(cinfo.payload_len + 1);
        payload.extend_from_slice(&buf[PTP_CONTAINER_INFO_SIZE..]);

        // 如果响应没有完全放入原始buf，或者初始读取刚好满足，可能还需要读取零长度包
        if payload.len() < cinfo.payload_len || buf.len() == unintialized_buf.len() {
            unsafe {
                let p = payload.as_mut_ptr().offset(payload.len() as isize);
                let pslice = slice::from_raw_parts_mut(p, payload.capacity() - payload.len());
                let n = self.handle.read_bulk(self.ep_in, pslice, timeout)?;
                let sz = payload.len();
                payload.set_len(sz + n);
                trace!("  bulk rx {}, ({}/{})", n, payload.len(), payload.capacity());
            }
        }

        Ok((cinfo, payload))
    }

    /// 获取对象信息
    pub fn get_objectinfo(&mut self, handle: u32, timeout: Option<Duration>) -> Result<PtpObjectInfo, Error> {
        let data = self.command(StandardCommandCode::GetObjectInfo, &[handle], None, timeout)?;
        Ok(PtpObjectInfo::decode(&data)?)
    }

    /// 获取完整对象
    pub fn get_object(&mut self, handle: u32, timeout: Option<Duration>) -> Result<Vec<u8>, Error> {
        self.command(StandardCommandCode::GetObject, &[handle], None, timeout)
    }

    /// 获取部分对象
    pub fn get_partialobject(&mut self, handle: u32, offset: u32, max: u32, timeout: Option<Duration>) -> Result<Vec<u8>, Error> {
        self.command(StandardCommandCode::GetPartialObject, &[handle, offset, max], None, timeout)
    }

    /// 删除对象
    pub fn delete_object(&mut self, handle: u32, timeout: Option<Duration>) -> Result<(), Error> {
        self.command(StandardCommandCode::DeleteObject, &[handle], None, timeout).map(|_| ())
    }

    /// 关机
    pub fn power_down(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.command(StandardCommandCode::PowerDown, &[], None, timeout).map(|_| ())
    }

    /// 获取对象句柄
    pub fn get_objecthandles(&mut self,
                             storage_id: u32,
                             handle_id: u32,
                             filter: Option<u32>,
                             timeout: Option<Duration>)
                             -> Result<Vec<u32>, Error> {
        let data = self.command(StandardCommandCode::GetObjectHandles,
                                    &[storage_id, filter.unwrap_or(0x0), handle_id],
                                    None, timeout)?;
        // 解析对象句柄数组
        let mut cur = std::io::Cursor::new(data);
        let value = cur.read_ptp_u32_vec()?;
        cur.expect_end()?;

        Ok(value)
    }

    /// 获取根目录中的对象句柄
    pub fn get_objecthandles_root(&mut self,
                                  storage_id: u32,
                                  filter: Option<u32>,
                                  timeout: Option<Duration>)
                                  -> Result<Vec<u32>, Error> {
        self.get_objecthandles(storage_id, 0xFFFFFFFF, filter, timeout)
    }

    /// 获取所有对象句柄
    pub fn get_objecthandles_all(&mut self,
                                 storage_id: u32,
                                 filter: Option<u32>,
                                 timeout: Option<Duration>)
                                 -> Result<Vec<u32>, Error> {
        self.get_objecthandles(storage_id, 0x0, filter, timeout)
    }

    /// 获取对象数量
    pub fn get_numobjects(&mut self,
                          storage_id: u32,
                          handle_id: u32,
                          filter: Option<u32>,
                          timeout: Option<Duration>)
                          -> Result<u32, Error> {
        let data = self.command(StandardCommandCode::GetNumObjects,
                                    &[storage_id, filter.unwrap_or(0x0), handle_id],
                                    None, timeout)?;

        // 解析对象数量
        let mut cur = std::io::Cursor::new(data);
        let value = cur.read_ptp_u32()?;
        cur.expect_end()?;

        Ok(value)
    }

    /// 获取存储信息
    pub fn get_storage_info(&mut self, storage_id: u32, timeout: Option<Duration>) -> Result<PtpStorageInfo, Error> {
        let data = self.command(StandardCommandCode::GetStorageInfo, &[storage_id], None, timeout)?;

        // 解析存储信息
        let mut cur = std::io::Cursor::new(data);
        let res = PtpStorageInfo::decode(&mut cur)?;
        cur.expect_end()?;

        Ok(res)
    }

    /// 获取存储ID列表
    pub fn get_storageids(&mut self, timeout: Option<Duration>) -> Result<Vec<u32>, Error> {
        let data = self.command(StandardCommandCode::GetStorageIDs, &[], None, timeout)?;

        // 解析存储ID数组
        let mut cur = std::io::Cursor::new(data);
        let value = cur.read_ptp_u32_vec()?;
        cur.expect_end()?;

        Ok(value)
    }

    /// 获取根目录对象数量
    pub fn get_numobjects_roots(&mut self,
                                storage_id: u32,
                                filter: Option<u32>,
                                timeout: Option<Duration>)
                                -> Result<u32, Error> {
        self.get_numobjects(storage_id, 0xFFFFFFFF, filter, timeout)
    }

    /// 获取所有对象数量
    pub fn get_numobjects_all(&mut self, storage_id: u32, filter: Option<u32>, timeout: Option<Duration>) -> Result<u32, Error> {
        self.get_numobjects(storage_id, 0x0, filter, timeout)
    }

    /// 获取设备信息
    pub async fn get_device_info(&mut self, timeout: Option<Duration>) -> Result<PtpDeviceInfo, Error> {
        let response = self.command(StandardCommandCode::GetDeviceInfo, &[], None, timeout).await?;

        let device_info = PtpDeviceInfo::decode(&response)?;
        debug!("设备信息 {:?}", device_info);
        Ok(device_info)
    }

    /// 打开会话
    pub async fn open_session(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        let session_id = 1; // 会话ID = 1

        let _response = self.command(StandardCommandCode::OpenSession,
                              &[session_id], // 会话ID = 1
                              None,
                              timeout).await?;
        Ok(())
    }

    /// 关闭会话
    pub async fn close_session(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        let _response = self.command(StandardCommandCode::CloseSession, &[], None, timeout).await?;
        Ok(())
    }

    /// 断开连接
    pub async fn disconnect(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.close_session(timeout).await?;
        self.handle.release_interface(self.iface).await
            .map_err(|e| Error::USB(format!("无法释放接口: {:?}", e)))?;
        Ok(())
    }
}
