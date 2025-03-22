#[cfg(test)]
mod tests {
    use crate::CameraDevice;
    #[test]
    fn test_camera_connection() {
        // 检测相机是否能被识别
        let mut camera = CameraDevice::new(0x04A9, 0x326F); // 更换为相机VID和PID
        if camera.connect().is_ok() {
            println!("相机连接成功！");
        }
    }
}
