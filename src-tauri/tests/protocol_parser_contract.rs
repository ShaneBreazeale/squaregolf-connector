use squaregolf_connector::core::protocol::parser::*;

#[test]
fn parses_sensor_data_like_go_connector() {
    let sensor = parse_sensor_data(&[
        "00", "01", "02", "02", "01", "0A", "00", "00", "00", "14", "00", "00", "00", "1E", "00",
        "00", "00",
    ])
    .expect("sensor data");

    assert!(sensor.ball_ready);
    assert!(sensor.ball_detected);
    assert_eq!(sensor.position_x, 10);
    assert_eq!(sensor.position_y, 20);
    assert_eq!(sensor.position_z, 30);

    assert!(parse_sensor_data(&["00", "01"]).is_err());
}

#[test]
fn parses_ball_metrics_like_go_connector() {
    let metrics = parse_shot_ball_metrics(&[
        "11", "02", "37", "64", "00", "C8", "00", "2C", "01", "E8", "03", "F4", "01", "D0", "07",
        "B8", "0B",
    ])
    .expect("ball metrics");

    assert_eq!(metrics.ball_speed_mps, 1.0);
    assert_eq!(metrics.vertical_angle, 2.0);
    assert_eq!(metrics.horizontal_angle, 3.0);
    assert_eq!(metrics.total_spin_rpm, 1000);
    assert_eq!(metrics.spin_axis, 5.0);
    assert_eq!(metrics.backspin_rpm, 2000);
    assert_eq!(metrics.sidespin_rpm, 3000);
    assert!(metrics.is_ball_speed_valid);
    assert!(metrics.is_total_spin_valid);
}

#[test]
fn ball_metrics_handle_negative_values_and_invalid_hex_like_go_connector() {
    let negative = parse_shot_ball_metrics(&[
        "11", "02", "37", "9C", "FF", "38", "FF", "D4", "FE", "18", "FC", "0C", "FE", "30", "F8",
        "48", "F4",
    ])
    .expect("negative metrics");

    assert_eq!(negative.ball_speed_mps, -1.0);
    assert_eq!(negative.vertical_angle, -2.0);
    assert_eq!(negative.horizontal_angle, -3.0);
    assert_eq!(negative.total_spin_rpm, 1000);
    assert_eq!(negative.spin_axis, -5.0);
    assert_eq!(negative.backspin_rpm, -2000);
    assert_eq!(negative.sidespin_rpm, -3000);

    let invalid_speed = parse_shot_ball_metrics(&[
        "11", "02", "37", "ZZ", "00", "C8", "00", "2C", "01", "E8", "03", "F4", "01", "D0", "07",
        "B8", "0B",
    ])
    .expect("invalid speed is non-fatal");

    assert_eq!(invalid_speed.ball_speed_mps, 0.0);
    assert!(!invalid_speed.is_ball_speed_valid);
    assert_eq!(invalid_speed.vertical_angle, 2.0);
}

#[test]
fn omni_ball_validity_bitmask_matches_go_connector() {
    let mut metrics = parse_shot_ball_metrics(&[
        "11", "02", "07", "64", "00", "C8", "00", "2C", "01", "E8", "03", "F4", "01", "D0", "07",
        "B8", "0B",
    ])
    .expect("ball metrics");

    apply_omni_ball_validity_bitmask(&mut metrics);

    assert!(metrics.is_ball_speed_valid);
    assert!(metrics.is_total_spin_valid);
    assert!(metrics.is_spin_axis_valid);
    assert!(!metrics.is_backspin_valid);
    assert!(!metrics.is_sidespin_valid);
}

#[test]
fn parses_club_metrics_like_go_connector() {
    let metrics = parse_shot_club_metrics(&[
        "00", "01", "02", "64", "00", "C8", "00", "2C", "01", "90", "01",
    ])
    .expect("club metrics");

    assert_eq!(metrics.path_angle, 1.0);
    assert_eq!(metrics.face_angle, 2.0);
    assert_eq!(metrics.attack_angle, 3.0);
    assert_eq!(metrics.dynamic_loft_angle, 4.0);
    assert!(metrics.is_path_angle_valid);
    assert!(metrics.is_face_angle_valid);
}

#[test]
fn parses_omni_club_metrics_bitmask_and_sentinel_like_go_connector() {
    let partial = parse_omni_shot_club_metrics(&[
        "11", "07", "0f", "d8", "fe", "90", "01", "38", "ff", "d0", "07", "64", "00", "c8", "ff",
        "b8", "0b", "82", "00",
    ])
    .expect("partial omni metrics");

    assert_eq!(partial.path_angle, -2.96);
    assert_eq!(partial.face_angle, 4.0);
    assert!(partial.is_path_angle_valid);
    assert!(partial.is_dynamic_loft_valid);
    assert!(!partial.is_impact_horizontal_valid);
    assert!(!partial.is_smash_factor_valid);

    let sentinel = parse_omni_shot_club_metrics(&[
        "11", "07", "ff", "00", "80", "90", "01", "38", "ff", "d0", "07", "64", "00", "c8", "ff",
        "b8", "0b", "82", "00",
    ])
    .expect("sentinel omni metrics");

    assert_eq!(sentinel.path_angle, 0.0);
    assert!(!sentinel.is_path_angle_valid);
    assert!(sentinel.is_face_angle_valid);
}

#[test]
fn parses_alignment_and_detects_omni_devices() {
    let alignment =
        parse_alignment_data(&["11", "04", "01", "00", "00", "38", "ff"]).expect("alignment data");

    assert_eq!(alignment.aim_angle, -2.0);
    assert!(alignment.is_aligned);
    assert!(parse_alignment_data(&["11", "04"]).is_err());

    assert_eq!(detect_device_type("some3033303041data"), DeviceType::Omni);
    assert_eq!(detect_device_type("aabbcc"), DeviceType::Home);
}
