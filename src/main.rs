use evdev::{
    uinput::VirtualDeviceBuilder, AbsInfo, AbsoluteAxisType, AttributeSet, EventType, InputEvent, Key, UinputAbsSetup
};
use serialport::SerialPort;
use std::io::{self};
use std::time::Duration;

fn main() -> io::Result<()> {
    // Replace with your Arduino's port
    let port_name = "/dev/ttyUSB0";
    let baud_rate = 9600;

    // Open the serial port
    let mut port = serialport::new(port_name, baud_rate)
        .timeout(Duration::from_millis(100))
        .open()
        .expect("Failed to open port");

    println!("Connected to {}", port_name);

    // Create an attribute set for the buttons
    let mut keys = AttributeSet::<Key>::new();
    keys.insert(Key::BTN_0); // Use BTN_0 for the joystick button

    // Configure ABS_X axis
    let abs_x_setup = UinputAbsSetup::new(
        AbsoluteAxisType::ABS_X,
        AbsInfo::new (
            0,
            -32768,
            32767,
            0,
            0,
            0,
        )
    );

    // Configure ABS_Y axis
    let abs_y_setup = UinputAbsSetup::new(
        AbsoluteAxisType::ABS_Y,
        AbsInfo::new(
            0,
            -32768,
            32767,
            0,
            0,
            0,
        )
    );

    // Build the virtual joystick device
    let mut device = VirtualDeviceBuilder::new()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to create VirtualDeviceBuilder: {}", e)))?
        .name("Virtual Joystick")
        .with_keys(&keys)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to add keys: {}", e)))?
        .with_absolute_axis(&abs_x_setup)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to add ABS_X axis: {}", e)))?
        .with_absolute_axis(&abs_y_setup)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to add ABS_Y axis: {}", e)))?
        .build()
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("Failed to build device: {}", e)))?;

    println!("Virtual joystick created!");

    let mut serial_buf: Vec<u8> = vec![0; 32]; // Buffer size for messages
    let mut incomplete_data = String::new();

    loop {
        // Read serial data from the Arduino
        if let Err(err) = handle_serial_data(&mut port, &mut device, &mut serial_buf, &mut incomplete_data) {
            eprintln!("Error in loop: {:?}", err);
        }
    }
}

// Handles serial data and emits joystick events
fn handle_serial_data(
    port: &mut Box<dyn SerialPort>,
    device: &mut evdev::uinput::VirtualDevice,
    serial_buf: &mut Vec<u8>,
    incomplete_data: &mut String,
) -> io::Result<()> {
    match port.read(serial_buf.as_mut_slice()) {
        Ok(bytes_read) => {
            incomplete_data.push_str(&String::from_utf8_lossy(&serial_buf[..bytes_read]));

            while let Some(newline_idx) = incomplete_data.find("\r\n") {
                let complete_message = incomplete_data[..newline_idx].to_string();
                *incomplete_data = incomplete_data[newline_idx + 2..].to_string();

                // Parse joystick data
                if let Some((vrx, vry, sw)) = parse_joystick_data(&complete_message) {
                    println!("VRX: {}, VRY: {}, SW: {}", vrx, vry, sw);

                    // Normalize joystick values for the virtual device
                    let x = map_to_abs_range(vrx, 0, 1023, -32768, 32767);
                    let y = map_to_abs_range(vry, 0, 1023,  32767, -32768);

                    // Emit axis events
                    device.emit(&[
                        InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, x),
                        InputEvent::new(EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, y),
                    ])?;

                    // Emit button events
                    let button_value = if sw == 0 { 1 } else { 0 }; // Active-low button
                    device.emit(&[
                        InputEvent::new(EventType::KEY, Key::BTN_0.0, button_value),
                    ])?;
                }
            }
        }
        Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => {
            println!("Waiting for data...");
        }
        Err(e) => return Err(e),
    }

    Ok(())
}

// Function to parse joystick data
fn parse_joystick_data(data: &str) -> Option<(u16, u16, u8)> {
    let parts: Vec<&str> = data.split(',').collect();
    if parts.len() == 3 {
        let vrx = parts[0].trim().parse().ok()?;
        let vry = parts[1].trim().parse().ok()?;
        let sw = parts[2].trim().parse().ok()?;
        return Some((vrx, vry, sw));
    }
    None
}

// Function to map values from one range to another
fn map_to_abs_range(value: u16, in_min: u16, in_max: u16, out_min: i32, out_max: i32) -> i32 {
    (value as i32 - in_min as i32) * (out_max - out_min) / (in_max as i32 - in_min as i32) + out_min
}
