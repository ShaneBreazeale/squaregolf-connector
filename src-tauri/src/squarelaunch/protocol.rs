use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;

const MPS_TO_MPH: f64 = 2.236_936_292_054_4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum SquareLaunchMessage {
    Shot(SquareLaunchShot),
    Status,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SquareLaunchShot {
    pub shot_number: u64,
    pub ball_speed_mph: f64,
    pub vertical_launch_angle_degrees: f64,
    pub horizontal_launch_angle_degrees: f64,
    pub total_spin_rpm: f64,
    pub spin_axis_degrees: f64,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(rename = "type")]
    message_type: String,
}

pub fn parse_squarelaunch_ws_message(text: &str) -> Result<SquareLaunchMessage, String> {
    let envelope =
        serde_json::from_str::<Envelope>(text).map_err(|err| format!("invalid JSON: {err}"))?;
    match envelope.message_type.as_str() {
        "shot" => parse_shot(text).map(SquareLaunchMessage::Shot),
        "status" => Ok(SquareLaunchMessage::Status),
        other => Ok(SquareLaunchMessage::Other(other.to_string())),
    }
}

fn parse_shot(text: &str) -> Result<SquareLaunchShot, String> {
    let value = serde_json::from_str::<Value>(text)
        .map_err(|err| format!("invalid SquareLaunch shot JSON: {err}"))?;
    Ok(SquareLaunchShot {
        shot_number: required_u64(&value, "shot_number")?,
        ball_speed_mph: required_f64(&value, "ball_speed_meters_per_second")? * MPS_TO_MPH,
        vertical_launch_angle_degrees: required_f64(&value, "vertical_launch_angle_degrees")?,
        horizontal_launch_angle_degrees: required_f64(&value, "horizontal_launch_angle_degrees")?,
        total_spin_rpm: required_f64(&value, "total_spin_rpm")?,
        spin_axis_degrees: required_f64(&value, "spin_axis_degrees")?,
    })
}

fn required_f64(value: &Value, field: &str) -> Result<f64, String> {
    let number = value
        .get(field)
        .ok_or_else(|| format!("missing {field}"))?
        .as_f64()
        .ok_or_else(|| format!("{field} must be a finite number"))?;
    if !number.is_finite() {
        return Err(format!("{field} must be a finite number"));
    }
    Ok(number)
}

fn required_u64(value: &Value, field: &str) -> Result<u64, String> {
    value
        .get(field)
        .ok_or_else(|| format!("missing {field}"))?
        .as_u64()
        .ok_or_else(|| format!("{field} must be an unsigned integer"))
}
