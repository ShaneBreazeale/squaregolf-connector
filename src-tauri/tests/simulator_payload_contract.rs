use squaregolf_connector::core::protocol::parser::{
    parse_omni_shot_club_metrics, parse_shot_ball_metrics,
};
use squaregolf_connector::simulator::open_connect::{
    ready_payload, shot_payload_from_metrics, shot_payload_with_club_metrics,
};

#[test]
fn ball_payload_matches_go_gspro_conversion() {
    let metrics = parse_shot_ball_metrics(&[
        "11", "02", "37", "64", "00", "C8", "00", "2C", "01", "E8", "03", "F4", "01", "D0", "07",
        "B8", "0B",
    ])
    .expect("ball metrics");

    let payload = shot_payload_from_metrics(&metrics, 7);

    assert_eq!(payload.device_id, "CustomLaunchMonitor");
    assert_eq!(payload.units, "Yards");
    assert_eq!(payload.api_version, "1");
    assert_eq!(payload.shot_number, 7);
    assert!(payload.shot_data_options.contains_ball_data);
    assert!(!payload.shot_data_options.contains_club_data);

    let ball = payload.ball_data.expect("ball data");
    assert!((ball.speed - 2.23694).abs() < 0.00001);
    assert_eq!(ball.spin_axis, -5.0);
    assert_eq!(ball.total_spin, 1000);
    assert_eq!(ball.back_spin, 2000);
    assert_eq!(ball.side_spin, -3000);
    assert_eq!(ball.hla, 3.0);
    assert_eq!(ball.vla, 2.0);
}

#[test]
fn club_payload_matches_go_gspro_conversion() {
    let club = parse_omni_shot_club_metrics(&[
        "11", "07", "ff", "d8", "fe", "90", "01", "38", "ff", "d0", "07", "64", "00", "c8", "ff",
        "b8", "0b", "82", "00",
    ])
    .expect("club metrics");

    let payload = shot_payload_with_club_metrics(&club, 9);

    assert!(!payload.shot_data_options.contains_ball_data);
    assert!(payload.shot_data_options.contains_club_data);
    assert_eq!(payload.shot_number, 9);

    let club = payload.club_data.expect("club data");
    assert!((club.speed - 30.0 * 2.23694).abs() < 0.00001);
    assert_eq!(club.angle_of_attack, -2.0);
    assert_eq!(club.face_to_target, 4.0);
    assert_eq!(club.loft, 20.0);
    assert_eq!(club.path, -2.96);
    assert_eq!(club.vertical_face_impact, -0.56);
    assert_eq!(club.horizontal_face_impact, 1.0);
}

#[test]
fn ready_payload_matches_go_listener_shape() {
    let payload = ready_payload(true, 3);

    assert_eq!(payload.shot_number, 3);
    assert!(!payload.shot_data_options.contains_ball_data);
    assert!(!payload.shot_data_options.contains_club_data);
    assert_eq!(
        payload.shot_data_options.launch_monitor_is_ready,
        Some(true)
    );
    assert_eq!(
        payload.shot_data_options.launch_monitor_ball_detected,
        Some(true)
    );
    assert!(payload.ball_data.is_none());
    assert!(payload.club_data.is_none());
}

#[test]
fn payload_serializes_with_gspro_field_names() {
    let payload = ready_payload(false, 1);
    let json = serde_json::to_value(payload).expect("json");

    assert_eq!(json["DeviceID"], "CustomLaunchMonitor");
    assert_eq!(json["APIversion"], "1");
    assert_eq!(json["ShotDataOptions"]["ContainsBallData"], false);
    assert_eq!(json["ShotDataOptions"]["LaunchMonitorIsReady"], false);
}
