use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const OMNI_MANUFACTURER_DATA_HEX: &str = "3033303041";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum DeviceType {
    Home,
    Omni,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SensorData {
    pub raw_data: Vec<String>,
    pub ball_ready: bool,
    pub ball_detected: bool,
    pub position_x: i32,
    pub position_y: i32,
    pub position_z: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParsedBallMetrics {
    pub raw_data: Vec<String>,
    pub ball_speed_mps: f64,
    pub vertical_angle: f64,
    pub horizontal_angle: f64,
    pub total_spin_rpm: i16,
    pub spin_axis: f64,
    pub backspin_rpm: i16,
    pub sidespin_rpm: i16,
    pub is_ball_speed_valid: bool,
    pub is_total_spin_valid: bool,
    pub is_spin_axis_valid: bool,
    pub is_backspin_valid: bool,
    pub is_sidespin_valid: bool,
    validity_bitmask: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ParsedClubMetrics {
    pub raw_data: Vec<String>,
    pub path_angle: f64,
    pub face_angle: f64,
    pub attack_angle: f64,
    pub dynamic_loft_angle: f64,
    pub impact_horizontal: f64,
    pub impact_vertical: f64,
    pub club_speed: f64,
    pub smash_factor: f64,
    pub is_path_angle_valid: bool,
    pub is_face_angle_valid: bool,
    pub is_attack_angle_valid: bool,
    pub is_dynamic_loft_valid: bool,
    pub is_impact_horizontal_valid: bool,
    pub is_impact_vertical_valid: bool,
    pub is_club_speed_valid: bool,
    pub is_smash_factor_valid: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlignmentData {
    pub raw_data: Vec<String>,
    pub aim_angle: f64,
    pub is_aligned: bool,
}

pub fn parse_sensor_data(bytes: &[&str]) -> Result<SensorData, String> {
    if bytes.len() < 17 {
        return Err("insufficient data for parsing sensor data".to_string());
    }

    Ok(SensorData {
        raw_data: raw(bytes),
        ball_ready: bytes[3].eq_ignore_ascii_case("01") || bytes[3].eq_ignore_ascii_case("02"),
        ball_detected: bytes[4].eq_ignore_ascii_case("01"),
        position_x: parse_i32(bytes, 5).unwrap_or(0),
        position_y: parse_i32(bytes, 9).unwrap_or(0),
        position_z: parse_i32(bytes, 13).unwrap_or(0),
    })
}

pub fn parse_shot_ball_metrics(bytes: &[&str]) -> Result<ParsedBallMetrics, String> {
    if bytes.len() < 17 {
        return Err("insufficient data for parsing ball metrics".to_string());
    }

    let mut metrics = ParsedBallMetrics {
        raw_data: raw(bytes),
        ball_speed_mps: 0.0,
        vertical_angle: 0.0,
        horizontal_angle: 0.0,
        total_spin_rpm: 0,
        spin_axis: 0.0,
        backspin_rpm: 0,
        sidespin_rpm: 0,
        is_ball_speed_valid: true,
        is_total_spin_valid: true,
        is_spin_axis_valid: true,
        is_backspin_valid: true,
        is_sidespin_valid: true,
        validity_bitmask: bytes.get(2).map(|s| (*s).to_string()),
    };

    match parse_scaled_i16(bytes[3], bytes[4], 100.0) {
        Some((value, valid)) => {
            metrics.ball_speed_mps = value;
            metrics.is_ball_speed_valid = valid;
        }
        None => metrics.is_ball_speed_valid = false,
    }
    if let Some((value, _)) = parse_scaled_i16(bytes[5], bytes[6], 100.0) {
        metrics.vertical_angle = value;
    }
    if let Some((value, _)) = parse_scaled_i16(bytes[7], bytes[8], 100.0) {
        metrics.horizontal_angle = value;
    }
    match parse_i16_metric(bytes[9], bytes[10]) {
        Some((value, valid)) => {
            metrics.total_spin_rpm = value;
            metrics.is_total_spin_valid = valid;
        }
        None => metrics.is_total_spin_valid = false,
    }
    match parse_scaled_i16(bytes[11], bytes[12], 100.0) {
        Some((value, valid)) => {
            metrics.spin_axis = value;
            metrics.is_spin_axis_valid = valid;
        }
        None => metrics.is_spin_axis_valid = false,
    }
    match parse_i16_metric(bytes[13], bytes[14]) {
        Some((value, valid)) => {
            metrics.backspin_rpm = value;
            metrics.is_backspin_valid = valid;
        }
        None => metrics.is_backspin_valid = false,
    }
    match parse_i16_metric(bytes[15], bytes[16]) {
        Some((value, valid)) => {
            metrics.sidespin_rpm = value;
            metrics.is_sidespin_valid = valid;
        }
        None => metrics.is_sidespin_valid = false,
    }

    if metrics.backspin_rpm < 0 {
        metrics.total_spin_rpm = -metrics.total_spin_rpm;
    }

    if metrics.is_total_spin_valid && metrics.is_spin_axis_valid {
        let spin_axis_rad = metrics.spin_axis.to_radians();
        if !metrics.is_backspin_valid {
            metrics.backspin_rpm = (f64::from(metrics.total_spin_rpm) * spin_axis_rad.cos()) as i16;
        }
        if !metrics.is_sidespin_valid {
            metrics.sidespin_rpm = (f64::from(metrics.total_spin_rpm) * spin_axis_rad.sin()) as i16;
        }
    }

    Ok(metrics)
}

pub fn apply_omni_ball_validity_bitmask(metrics: &mut ParsedBallMetrics) {
    let Some(bitmask) = metrics
        .validity_bitmask
        .as_ref()
        .and_then(|value| u8::from_str_radix(value, 16).ok())
    else {
        return;
    };

    metrics.is_ball_speed_valid &= bitmask & 0x01 != 0;
    metrics.is_total_spin_valid &= bitmask & 0x02 != 0;
    metrics.is_spin_axis_valid &= bitmask & 0x04 != 0;
    metrics.is_backspin_valid &= bitmask & 0x10 != 0;
    metrics.is_sidespin_valid &= bitmask & 0x20 != 0;
}

pub fn parse_shot_club_metrics(bytes: &[&str]) -> Result<ParsedClubMetrics, String> {
    if bytes.len() < 11 {
        return Err("insufficient data for parsing club metrics".to_string());
    }

    let mut metrics = ParsedClubMetrics {
        raw_data: raw(bytes),
        is_path_angle_valid: true,
        is_face_angle_valid: true,
        is_attack_angle_valid: true,
        is_dynamic_loft_valid: true,
        ..Default::default()
    };

    set_scaled_field(
        &mut metrics.path_angle,
        &mut metrics.is_path_angle_valid,
        bytes[3],
        bytes[4],
        true,
    );
    set_scaled_field(
        &mut metrics.face_angle,
        &mut metrics.is_face_angle_valid,
        bytes[5],
        bytes[6],
        true,
    );
    set_scaled_field(
        &mut metrics.attack_angle,
        &mut metrics.is_attack_angle_valid,
        bytes[7],
        bytes[8],
        true,
    );
    set_scaled_field(
        &mut metrics.dynamic_loft_angle,
        &mut metrics.is_dynamic_loft_valid,
        bytes[9],
        bytes[10],
        true,
    );

    Ok(metrics)
}

pub fn parse_omni_shot_club_metrics(bytes: &[&str]) -> Result<ParsedClubMetrics, String> {
    if bytes.len() < 19 {
        return Err(format!(
            "insufficient data for parsing Omni club metrics (need 19, got {})",
            bytes.len()
        ));
    }

    let validity = u8::from_str_radix(bytes[2], 16).unwrap_or(0);
    let mut metrics = ParsedClubMetrics {
        raw_data: raw(bytes),
        ..Default::default()
    };

    let fields = [
        (3, 4, 0_u8, Field::Path),
        (5, 6, 1_u8, Field::Face),
        (7, 8, 2_u8, Field::Attack),
        (9, 10, 3_u8, Field::Loft),
        (11, 12, 4_u8, Field::ImpactHorizontal),
        (13, 14, 5_u8, Field::ImpactVertical),
        (15, 16, 6_u8, Field::ClubSpeed),
        (17, 18, 7_u8, Field::SmashFactor),
    ];

    for (low, high, bit, field) in fields {
        let bitmask_valid = validity & (1 << bit) != 0;
        if let Some((value, sentinel_valid)) = parse_scaled_i16(bytes[low], bytes[high], 100.0) {
            field.set(&mut metrics, value, bitmask_valid && sentinel_valid);
        }
    }

    Ok(metrics)
}

pub fn parse_alignment_data(bytes: &[&str]) -> Result<AlignmentData, String> {
    if bytes.len() < 7 {
        return Err(format!(
            "insufficient data for parsing alignment data (need at least 7 bytes, got {})",
            bytes.len()
        ));
    }
    let aim_angle = parse_i16(bytes[5], bytes[6])
        .map(|value| f64::from(value) / 100.0)
        .unwrap_or(0.0);
    Ok(AlignmentData {
        raw_data: raw(bytes),
        aim_angle,
        is_aligned: (-2.0..=2.0).contains(&aim_angle),
    })
}

pub fn detect_device_type(mfg_data_hex: &str) -> DeviceType {
    if !mfg_data_hex.is_empty()
        && mfg_data_hex
            .to_ascii_uppercase()
            .contains(&OMNI_MANUFACTURER_DATA_HEX.to_ascii_uppercase())
    {
        DeviceType::Omni
    } else {
        DeviceType::Home
    }
}

impl Default for ParsedClubMetrics {
    fn default() -> Self {
        Self {
            raw_data: Vec::new(),
            path_angle: 0.0,
            face_angle: 0.0,
            attack_angle: 0.0,
            dynamic_loft_angle: 0.0,
            impact_horizontal: 0.0,
            impact_vertical: 0.0,
            club_speed: 0.0,
            smash_factor: 0.0,
            is_path_angle_valid: false,
            is_face_angle_valid: false,
            is_attack_angle_valid: false,
            is_dynamic_loft_valid: false,
            is_impact_horizontal_valid: false,
            is_impact_vertical_valid: false,
            is_club_speed_valid: false,
            is_smash_factor_valid: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Field {
    Path,
    Face,
    Attack,
    Loft,
    ImpactHorizontal,
    ImpactVertical,
    ClubSpeed,
    SmashFactor,
}

impl Field {
    fn set(self, metrics: &mut ParsedClubMetrics, value: f64, valid: bool) {
        match self {
            Field::Path => {
                metrics.path_angle = value;
                metrics.is_path_angle_valid = valid;
            }
            Field::Face => {
                metrics.face_angle = value;
                metrics.is_face_angle_valid = valid;
            }
            Field::Attack => {
                metrics.attack_angle = value;
                metrics.is_attack_angle_valid = valid;
            }
            Field::Loft => {
                metrics.dynamic_loft_angle = value;
                metrics.is_dynamic_loft_valid = valid;
            }
            Field::ImpactHorizontal => {
                metrics.impact_horizontal = value;
                metrics.is_impact_horizontal_valid = valid;
            }
            Field::ImpactVertical => {
                metrics.impact_vertical = value;
                metrics.is_impact_vertical_valid = valid;
            }
            Field::ClubSpeed => {
                metrics.club_speed = value;
                metrics.is_club_speed_valid = valid;
            }
            Field::SmashFactor => {
                metrics.smash_factor = value;
                metrics.is_smash_factor_valid = valid;
            }
        }
    }
}

fn set_scaled_field(
    target: &mut f64,
    valid: &mut bool,
    low: &str,
    high: &str,
    default_valid: bool,
) {
    if let Some((value, sentinel_valid)) = parse_scaled_i16(low, high, 100.0) {
        *target = value;
        *valid = default_valid && sentinel_valid;
    } else {
        *valid = false;
    }
}

fn raw(bytes: &[&str]) -> Vec<String> {
    bytes.iter().map(|value| (*value).to_string()).collect()
}

fn parse_i32(bytes: &[&str], start: usize) -> Option<i32> {
    let decoded = decode_hex_bytes(&bytes[start..start + 4])?;
    Some(i32::from_le_bytes(decoded.try_into().ok()?))
}

fn parse_i16_metric(low: &str, high: &str) -> Option<(i16, bool)> {
    let value = parse_i16(low, high)?;
    if value == i16::MIN {
        Some((0, false))
    } else {
        Some((value, true))
    }
}

fn parse_scaled_i16(low: &str, high: &str, scale: f64) -> Option<(f64, bool)> {
    let (value, valid) = parse_i16_metric(low, high)?;
    Some((f64::from(value) / scale, valid))
}

fn parse_i16(low: &str, high: &str) -> Option<i16> {
    let decoded = decode_hex_bytes(&[low, high])?;
    Some(i16::from_le_bytes(decoded.try_into().ok()?))
}

fn decode_hex_bytes(parts: &[&str]) -> Option<Vec<u8>> {
    parts
        .iter()
        .map(|part| u8::from_str_radix(part, 16).ok())
        .collect()
}
