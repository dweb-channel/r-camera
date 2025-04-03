#![allow(non_snake_case)]

// 定义PTP容器类型
#[derive(Debug, PartialEq)]
#[repr(u16)]
pub enum PtpContainerType {
    Command = 1,  // 命令容器
    Data = 2,     // 数据容器
    Response = 3, // 响应容器
    Event = 4,    // 事件容器
}

impl PtpContainerType {
    /// 从u16值转换为PtpContainerType枚举
    pub fn from_u16(v: u16) -> Option<PtpContainerType> {
        use self::PtpContainerType::*;
        match v {
            1 => Some(Command),
            2 => Some(Data),
            3 => Some(Response),
            4 => Some(Event),
            _ => None
        }
    }
}

/// 响应码类型
pub type ResponseCode = u16;

/// 标准PTP响应码定义
#[allow(non_upper_case_globals)]
pub mod StandardResponseCode {
    use super::ResponseCode;

    // 定义标准PTP响应码常量
    pub const Undefined: ResponseCode = 0x2000;
    pub const Ok: ResponseCode = 0x2001;
    pub const GeneralError: ResponseCode = 0x2002;
    pub const SessionNotOpen: ResponseCode = 0x2003;
    pub const InvalidTransactionId: ResponseCode = 0x2004;
    pub const OperationNotSupported: ResponseCode = 0x2005;
    pub const ParameterNotSupported: ResponseCode = 0x2006;
    pub const IncompleteTransfer: ResponseCode = 0x2007;
    pub const InvalidStorageId: ResponseCode = 0x2008;
    pub const InvalidObjectHandle: ResponseCode = 0x2009;
    pub const DevicePropNotSupported: ResponseCode = 0x200A;
    pub const InvalidObjectFormatCode: ResponseCode = 0x200B;
    pub const StoreFull: ResponseCode = 0x200C;
    pub const ObjectWriteProtected: ResponseCode = 0x200D;
    pub const StoreReadOnly: ResponseCode = 0x200E;
    pub const AccessDenied: ResponseCode = 0x200F;
    pub const NoThumbnailPresent: ResponseCode = 0x2010;
    pub const SelfTestFailed: ResponseCode = 0x2011;
    pub const PartialDeletion: ResponseCode = 0x2012;
    pub const StoreNotAvailable: ResponseCode = 0x2013;
    pub const SpecificationByFormatUnsupported: ResponseCode = 0x2014;
    pub const NoValidObjectInfo: ResponseCode = 0x2015;
    pub const InvalidCodeFormat: ResponseCode = 0x2016;
    pub const UnknownVendorCode: ResponseCode = 0x2017;
    pub const CaptureAlreadyTerminated: ResponseCode = 0x2018;
    pub const DeviceBusy: ResponseCode = 0x2019;
    pub const InvalidParentObject: ResponseCode = 0x201A;
    pub const InvalidDevicePropFormat: ResponseCode = 0x201B;
    pub const InvalidDevicePropValue: ResponseCode = 0x201C;
    pub const InvalidParameter: ResponseCode = 0x201D;
    pub const SessionAlreadyOpen: ResponseCode = 0x201E;
    pub const TransactionCancelled: ResponseCode = 0x201F;
    pub const SpecificationOfDestinationUnsupported: ResponseCode = 0x2020;

    /// 根据响应码返回对应的名称
    pub fn name(v: ResponseCode) -> Option<&'static str> {
        match v {
            Undefined => Some("未定义"),
            Ok => Some("成功"),
            GeneralError => Some("一般错误"),
            SessionNotOpen => Some("会话未打开"),
            InvalidTransactionId => Some("无效的事务ID"),
            OperationNotSupported => Some("不支持的操作"),
            ParameterNotSupported => Some("不支持的参数"),
            IncompleteTransfer => Some("传输不完整"),
            InvalidStorageId => Some("无效的存储ID"),
            InvalidObjectHandle => Some("无效的对象句柄"),
            DevicePropNotSupported => Some("不支持的设备属性"),
            InvalidObjectFormatCode => Some("无效的对象格式代码"),
            StoreFull => Some("存储已满"),
            ObjectWriteProtected => Some("对象写保护"),
            StoreReadOnly => Some("存储只读"),
            AccessDenied => Some("访问被拒绝"),
            NoThumbnailPresent => Some("没有缩略图"),
            SelfTestFailed => Some("自检失败"),
            PartialDeletion => Some("部分删除"),
            StoreNotAvailable => Some("存储不可用"),
            SpecificationByFormatUnsupported => Some("不支持按格式指定"),
            NoValidObjectInfo => Some("无有效对象信息"),
            InvalidCodeFormat => Some("无效的代码格式"),
            UnknownVendorCode => Some("未知的厂商代码"),
            CaptureAlreadyTerminated => Some("捕获已终止"),
            DeviceBusy => Some("设备忙"),
            InvalidParentObject => Some("无效的父对象"),
            InvalidDevicePropFormat => Some("无效的设备属性格式"),
            InvalidDevicePropValue => Some("无效的设备属性值"),
            InvalidParameter => Some("无效的参数"),
            SessionAlreadyOpen => Some("会话已打开"),
            TransactionCancelled => Some("事务已取消"),
            SpecificationOfDestinationUnsupported => Some("不支持指定目标"),
            _ => None,
        }
    }
}

/// 命令码类型
pub type CommandCode = u16;

/// 标准PTP命令码定义
#[allow(non_upper_case_globals)]
pub mod StandardCommandCode {
    use super::CommandCode;

    // 定义标准PTP命令码常量
    pub const Undefined: CommandCode = 0x1000;
    pub const GetDeviceInfo: CommandCode = 0x1001;
    pub const OpenSession: CommandCode = 0x1002;
    pub const CloseSession: CommandCode = 0x1003;
    pub const GetStorageIDs: CommandCode = 0x1004;
    pub const GetStorageInfo: CommandCode = 0x1005;
    pub const GetNumObjects: CommandCode = 0x1006;
    pub const GetObjectHandles: CommandCode = 0x1007;
    pub const GetObjectInfo: CommandCode = 0x1008;
    pub const GetObject: CommandCode = 0x1009;
    pub const GetThumb: CommandCode = 0x100A;
    pub const DeleteObject: CommandCode = 0x100B;
    pub const SendObjectInfo: CommandCode = 0x100C;
    pub const SendObject: CommandCode = 0x100D;
    pub const InitiateCapture: CommandCode = 0x100E;
    pub const FormatStore: CommandCode = 0x100F;
    pub const ResetDevice: CommandCode = 0x1010;
    pub const SelfTest: CommandCode = 0x1011;
    pub const SetObjectProtection: CommandCode = 0x1012;
    pub const PowerDown: CommandCode = 0x1013;
    pub const GetDevicePropDesc: CommandCode = 0x1014;
    pub const GetDevicePropValue: CommandCode = 0x1015;
    pub const SetDevicePropValue: CommandCode = 0x1016;
    pub const ResetDevicePropValue: CommandCode = 0x1017;
    pub const TerminateOpenCapture: CommandCode = 0x1018;
    pub const MoveObject: CommandCode = 0x1019;
    pub const CopyObject: CommandCode = 0x101A;
    pub const GetPartialObject: CommandCode = 0x101B;
    pub const InitiateOpenCapture: CommandCode = 0x101C;

    /// 根据命令码返回对应的名称
    pub fn name(v: CommandCode) -> Option<&'static str> {
        match v {
            Undefined => Some("未定义"),
            GetDeviceInfo => Some("获取设备信息"),
            OpenSession => Some("打开会话"),
            CloseSession => Some("关闭会话"),
            GetStorageIDs => Some("获取存储ID"),
            GetStorageInfo => Some("获取存储信息"),
            GetNumObjects => Some("获取对象数量"),
            GetObjectHandles => Some("获取对象句柄"),
            GetObjectInfo => Some("获取对象信息"),
            GetObject => Some("获取对象"),
            GetThumb => Some("获取缩略图"),
            DeleteObject => Some("删除对象"),
            SendObjectInfo => Some("发送对象信息"),
            SendObject => Some("发送对象"),
            InitiateCapture => Some("启动捕获"),
            FormatStore => Some("格式化存储"),
            ResetDevice => Some("重置设备"),
            SelfTest => Some("自检"),
            SetObjectProtection => Some("设置对象保护"),
            PowerDown => Some("关机"),
            GetDevicePropDesc => Some("获取设备属性描述"),
            GetDevicePropValue => Some("获取设备属性值"),
            SetDevicePropValue => Some("设置设备属性值"),
            ResetDevicePropValue => Some("重置设备属性值"),
            TerminateOpenCapture => Some("终止开放捕获"),
            MoveObject => Some("移动对象"),
            CopyObject => Some("复制对象"),
            GetPartialObject => Some("获取部分对象"),
            InitiateOpenCapture => Some("启动开放捕获"),
            _ => None,
        }
    }
}
