#![allow(non_snake_case)]

use std::cmp::min;
use std::slice;
use std::time::Duration;
use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian};

use crate::ptp_mtp::error::Error;
use crate::ptp_mtp::standard_codes::{CommandCode, StandardCommandCode, StandardResponseCode, PtpContainerType};
use crate::ptp_mtp::device_info::{PtpDeviceInfo, PtpObjectInfo, PtpStorageInfo};
use crate::ptp_mtp::data_types::PtpRead;

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
pub struct PtpCamera<'a> {
    iface: u8,                    // 接口号
    ep_in: u8,                    // 输入端点
    ep_out: u8,                   // 输出端点
    _ep_int: u8,                  // 中断端点
    current_tid: u32,             // 当前事务ID
    handle: libusb::DeviceHandle<'a>, // USB设备句柄
}

impl<'a> PtpCamera<'a> {
    /// 创建新的PTP相机实例
    pub fn new(device: &libusb::Device<'a>) -> Result<PtpCamera<'a>, Error> {
        // 获取活动配置描述符
        let config_desc = device.active_config_descriptor()?;

        // 查找PTP/MTP接口（类代码为6）
        let interface_desc = config_desc.interfaces()
            .flat_map(|i| i.descriptors())
            .find(|x| x.class_code() == 6)
            .ok_or(libusb::Error::NotFound)?;

        debug!("找到接口 {}", interface_desc.interface_number());

        // 打开设备并声明接口
        let mut handle = device.open()?;
        handle.claim_interface(interface_desc.interface_number())?;
        handle.set_alternate_setting(interface_desc.interface_number(), interface_desc.setting_number())?;

        // 辅助函数：查找端点
        let find_endpoint = |direction, transfer_type| {
            interface_desc.endpoint_descriptors()
                .find(|ep| ep.direction() == direction && ep.transfer_type() == transfer_type)
                .map(|x| x.address())
                .ok_or(libusb::Error::NotFound)
        };

        // 创建PTP相机实例
        Ok(PtpCamera {
            iface: interface_desc.interface_number(),
            ep_in:  find_endpoint(libusb::Direction::In, libusb::TransferType::Bulk)?,
            ep_out: find_endpoint(libusb::Direction::Out, libusb::TransferType::Bulk)?,
            _ep_int: find_endpoint(libusb::Direction::In, libusb::TransferType::Interrupt)?,
            current_tid: 0,
            handle: handle,
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
    pub fn command(&mut self,
                   code: CommandCode,
                   params: &[u32],
                   data: Option<&[u8]>,
                   timeout: Option<Duration>)
                   -> Result<Vec<u8>, Error> {

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
        self.write_txn_phase(PtpContainerType::Command, code, tid, &request_payload, timeout)?;

        // 如果有数据，写入数据阶段
        if let Some(data) = data {
            self.write_txn_phase(PtpContainerType::Data, code, tid, data, timeout)?;
        }

        // 命令阶段之后是数据阶段(可选)和响应阶段
        // 读取这两个阶段，检查响应的状态，并返回数据载荷(如果有)
        let mut data_phase_payload = vec![];
        loop {
            let (container, payload) = self.read_txn_phase(timeout)?;
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
    fn write_txn_phase(&mut self, kind: PtpContainerType, code: CommandCode, tid: u32, payload: &[u8], timeout: Duration) -> Result<(), Error> {
        trace!("写入 {:?} - 0x{:04x} ({}), tid:{}", kind, code, StandardCommandCode::name(code).unwrap_or("未知"), tid);

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
        self.handle.write_bulk(self.ep_out, &buf, timeout)?;

        // 写入后续块，直接从源切片读取
        for chunk in payload[first_chunk_payload_bytes..].chunks(CHUNK_SIZE) {
            self.handle.write_bulk(self.ep_out, chunk, timeout)?;
        }

        Ok(())
    }

    /// 读取事务阶段的辅助方法
    fn read_txn_phase(&mut self, timeout: Duration) -> Result<(PtpContainerInfo, Vec<u8>), Error> {
        // 缓冲区在栈上分配，大小足以容纳大多数命令/控制数据
        // 标记为未初始化以避免为8k内存清零
        let mut unintialized_buf: [u8; 8 * 1024];
        let buf = unsafe {
            unintialized_buf = ::std::mem::uninitialized();
            let n = self.handle.read_bulk(self.ep_in, &mut unintialized_buf[..], timeout)?;
            &unintialized_buf[..n]
        };

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
    pub fn get_device_info(&mut self, timeout: Option<Duration>) -> Result<PtpDeviceInfo, Error> {
        let data = self.command(StandardCommandCode::GetDeviceInfo, &[0, 0, 0], None, timeout)?;

        let device_info = PtpDeviceInfo::decode(&data)?;
        debug!("设备信息 {:?}", device_info);
        Ok(device_info)
    }

    /// 打开会话
    pub fn open_session(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        let session_id = 3; // 会话ID，通常可以是任意非零值

        self.command(StandardCommandCode::OpenSession,
                     &vec![session_id, 0, 0],
                     None, timeout)?;

        Ok(())
    }

    /// 关闭会话
    pub fn close_session(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.command(StandardCommandCode::CloseSession, &[], None, timeout)?;
        Ok(())
    }

    /// 断开连接
    pub fn disconnect(&mut self, timeout: Option<Duration>) -> Result<(), Error> {
        self.close_session(timeout)?;
        self.handle.release_interface(self.iface)?;
        Ok(())
    }
}
