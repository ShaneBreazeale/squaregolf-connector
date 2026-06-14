use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::core::protocol::parser::{ParsedBallMetrics, ParsedClubMetrics};

const MPS_TO_MPH: f64 = 2.23694;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub struct ShotPayload {
    #[serde(rename = "DeviceID")]
    pub device_id: String,
    pub units: String,
    #[serde(rename = "APIversion")]
    pub api_version: String,
    pub shot_number: u64,
    pub shot_data_options: ShotOptions,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ball_data: Option<BallData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub club_data: Option<ClubData>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub struct ShotOptions {
    pub contains_ball_data: bool,
    pub contains_club_data: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_monitor_is_ready: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launch_monitor_ball_detected: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub struct BallData {
    pub speed: f64,
    pub spin_axis: f64,
    pub total_spin: i16,
    pub back_spin: i16,
    pub side_spin: i16,
    #[serde(rename = "HLA")]
    pub hla: f64,
    #[serde(rename = "VLA")]
    pub vla: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "PascalCase")]
pub struct ClubData {
    pub speed: f64,
    pub angle_of_attack: f64,
    pub face_to_target: f64,
    pub lie: f64,
    pub loft: f64,
    pub path: f64,
    pub speed_at_impact: f64,
    pub vertical_face_impact: f64,
    pub horizontal_face_impact: f64,
    pub closure_rate: f64,
}

pub fn shot_payload_from_metrics(metrics: &ParsedBallMetrics, shot_number: u64) -> ShotPayload {
    ShotPayload {
        device_id: "CustomLaunchMonitor".to_string(),
        units: "Yards".to_string(),
        api_version: "1".to_string(),
        shot_number,
        shot_data_options: ShotOptions {
            contains_ball_data: true,
            contains_club_data: false,
            launch_monitor_is_ready: None,
            launch_monitor_ball_detected: None,
        },
        ball_data: Some(BallData {
            speed: metrics.ball_speed_mps * MPS_TO_MPH,
            spin_axis: metrics.spin_axis * -1.0,
            total_spin: metrics.total_spin_rpm,
            back_spin: metrics.backspin_rpm,
            side_spin: metrics.sidespin_rpm * -1,
            hla: metrics.horizontal_angle,
            vla: metrics.vertical_angle,
        }),
        club_data: Some(ClubData::empty()),
    }
}

pub fn shot_payload_with_club_metrics(
    metrics: &ParsedClubMetrics,
    shot_number: u64,
) -> ShotPayload {
    ShotPayload {
        device_id: "CustomLaunchMonitor".to_string(),
        units: "Yards".to_string(),
        api_version: "1".to_string(),
        shot_number,
        shot_data_options: ShotOptions {
            contains_ball_data: false,
            contains_club_data: true,
            launch_monitor_is_ready: None,
            launch_monitor_ball_detected: None,
        },
        ball_data: None,
        club_data: Some(ClubData {
            speed: metrics.club_speed * MPS_TO_MPH,
            angle_of_attack: metrics.attack_angle,
            face_to_target: metrics.face_angle,
            lie: 0.0,
            loft: metrics.dynamic_loft_angle,
            path: metrics.path_angle,
            speed_at_impact: 0.0,
            vertical_face_impact: metrics.impact_vertical,
            horizontal_face_impact: metrics.impact_horizontal,
            closure_rate: 0.0,
        }),
    }
}

pub fn ready_payload(ready: bool, shot_number: u64) -> ShotPayload {
    ShotPayload {
        device_id: "CustomLaunchMonitor".to_string(),
        units: "Yards".to_string(),
        api_version: "1".to_string(),
        shot_number,
        shot_data_options: ShotOptions {
            contains_ball_data: false,
            contains_club_data: false,
            launch_monitor_is_ready: Some(ready),
            launch_monitor_ball_detected: Some(ready),
        },
        ball_data: None,
        club_data: None,
    }
}

impl ClubData {
    pub fn empty() -> Self {
        Self {
            speed: 0.0,
            angle_of_attack: 0.0,
            face_to_target: 0.0,
            lie: 0.0,
            loft: 0.0,
            path: 0.0,
            speed_at_impact: 0.0,
            vertical_face_impact: 0.0,
            horizontal_face_impact: 0.0,
            closure_rate: 0.0,
        }
    }
}
