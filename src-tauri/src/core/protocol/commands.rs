#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Handedness {
    Right = 0,
    Left = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DetectBallMode {
    Deactivate = 0,
    Activate = 1,
    ActivateAlignmentMode = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpinMode {
    Standard = 0,
    Advanced = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClubType {
    pub regular_code: &'static str,
    pub swing_stick_code: &'static str,
}

impl ClubType {
    pub const PUTTER: Self = Self {
        regular_code: "0107",
        swing_stick_code: "0103",
    };
    pub const DRIVER: Self = Self {
        regular_code: "0204",
        swing_stick_code: "0202",
    };
    pub const WOOD_3: Self = Self {
        regular_code: "0305",
        swing_stick_code: "0301",
    };
    pub const WOOD_5: Self = Self {
        regular_code: "0505",
        swing_stick_code: "0501",
    };
    pub const WOOD_7: Self = Self {
        regular_code: "0705",
        swing_stick_code: "0701",
    };
    pub const IRON_4: Self = Self {
        regular_code: "0406",
        swing_stick_code: "0400",
    };
    pub const IRON_5: Self = Self {
        regular_code: "0506",
        swing_stick_code: "0500",
    };
    pub const IRON_6: Self = Self {
        regular_code: "0606",
        swing_stick_code: "0600",
    };
    pub const IRON_7: Self = Self {
        regular_code: "0706",
        swing_stick_code: "0700",
    };
    pub const IRON_8: Self = Self {
        regular_code: "0806",
        swing_stick_code: "0900",
    };
    pub const IRON_9: Self = Self {
        regular_code: "0906",
        swing_stick_code: "0900",
    };
    pub const PITCHING_WEDGE: Self = Self {
        regular_code: "0a06",
        swing_stick_code: "0a00",
    };
    pub const APPROACH_WEDGE: Self = Self {
        regular_code: "0b06",
        swing_stick_code: "0b00",
    };
    pub const SAND_WEDGE: Self = Self {
        regular_code: "0c06",
        swing_stick_code: "0c00",
    };
    pub const ALIGNMENT_STICK: Self = Self {
        regular_code: "0008",
        swing_stick_code: "0008",
    };
}

pub fn heartbeat_command(sequence: u8) -> String {
    format!("1183{sequence:02x}0000000000")
}

pub fn detect_ball_command(sequence: u8, mode: DetectBallMode, spin_mode: SpinMode) -> String {
    format!(
        "1181{sequence:02x}0{}1{}00000000",
        mode as u8, spin_mode as u8
    )
}

pub fn club_command(sequence: u8, club: ClubType, handedness: Handedness) -> String {
    format!(
        "1182{sequence:02x}{}0{}000000",
        club.regular_code, handedness as u8
    )
}

pub fn omni_club_command(sequence: u8, club: ClubType, handedness: Handedness) -> String {
    let club_number = u8::from_str_radix(&club.regular_code[0..2], 16).unwrap_or(0);
    let club_sel = u8::from_str_radix(&club.regular_code[2..4], 16).unwrap_or(0);
    let omni_sel = club_sel.saturating_sub(4);
    format!(
        "1182{sequence:02x}{club_number:02x}{omni_sel:02x}{:02x}000000",
        handedness as u8
    )
}

pub fn swing_stick_command(sequence: u8, club: ClubType, handedness: Handedness) -> String {
    format!(
        "1182{sequence:02x}{}0{}0000",
        club.swing_stick_code, handedness as u8
    )
}

pub fn alignment_command(sequence: u8, confirm: u8, target_angle: f64) -> String {
    let angle = (target_angle * 100.0) as i32;
    let bytes = angle.to_le_bytes();
    format!(
        "1185{sequence:02x}{confirm:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3]
    )
}

pub fn start_alignment_command(sequence: u8) -> String {
    alignment_command(sequence, 0, 0.0)
}

pub fn stop_alignment_command(sequence: u8, target_angle: f64) -> String {
    alignment_command(sequence, 1, target_angle)
}

pub fn cancel_alignment_command(sequence: u8, target_angle: f64) -> String {
    alignment_command(sequence, 0, target_angle)
}

pub fn request_club_metrics_command(sequence: u8) -> String {
    format!("1187{sequence:02x}000000000000")
}

pub fn get_os_version_command(sequence: u8) -> String {
    format!("1192{sequence:02x}0000000000")
}

pub fn get_charge_command(sequence: u8) -> String {
    format!("1186{sequence:02x}0000000000")
}

pub fn omni_set_units_command(sequence: u8, speed_unit: u8, distance_unit: u8) -> String {
    let (dist_marker, dist_sub) = if distance_unit > 0 {
        (1, distance_unit)
    } else {
        (0, 0)
    };
    format!("1188{sequence:02x}{speed_unit:02x}{dist_marker:02x}{dist_sub:02x}0000")
}

pub fn omni_set_green_speed_command(sequence: u8, green_speed: u8) -> String {
    format!("1189{sequence:02x}{green_speed:02x}00000000")
}

pub fn omni_set_carry_distance_adjustment_command(sequence: u8, adjustment: i8) -> String {
    let encoded = (i16::from(adjustment) + 100) as u8;
    format!("118a{sequence:02x}{encoded:02x}00000000")
}

pub fn omni_set_handed_command(sequence: u8, handedness: Handedness) -> String {
    format!("1182{sequence:02x}0063{:02x}000000", handedness as u8)
}
