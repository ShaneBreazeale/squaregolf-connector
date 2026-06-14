use squaregolf_connector::squarelaunch::protocol::{
    parse_squarelaunch_ws_message, SquareLaunchMessage,
};

#[test]
fn parses_squarelaunch_shot_message() {
    let msg = parse_squarelaunch_ws_message(
        r#"{
            "type": "shot",
            "timestamp_ns": 1710000000000000000,
            "shot_number": 42,
            "ball_speed_meters_per_second": 65.9,
            "vertical_launch_angle_degrees": 13.4,
            "horizontal_launch_angle_degrees": -2.1,
            "total_spin_rpm": 3120.0,
            "spin_axis_degrees": -9.5
        }"#,
    )
    .expect("valid shot");

    let SquareLaunchMessage::Shot(shot) = msg else {
        panic!("expected shot");
    };
    assert_eq!(shot.shot_number, 42);
    assert!((shot.ball_speed_mph - 65.9 * 2.236_936_292_054_4).abs() < 0.001);
    assert_eq!(shot.vertical_launch_angle_degrees, 13.4);
    assert_eq!(shot.horizontal_launch_angle_degrees, -2.1);
    assert_eq!(shot.total_spin_rpm, 3120.0);
    assert_eq!(shot.spin_axis_degrees, -9.5);
}

#[test]
fn accepts_squarelaunch_status_message() {
    let msg = parse_squarelaunch_ws_message(
        r#"{
            "type": "status",
            "uptime_seconds": 12,
            "firmware_version": "squarelaunch-sim",
            "shot_count": 3
        }"#,
    )
    .expect("valid status");

    assert!(matches!(msg, SquareLaunchMessage::Status));
}

#[test]
fn rejects_shot_missing_required_metric() {
    let err = parse_squarelaunch_ws_message(
        r#"{
            "type": "shot",
            "shot_number": 7,
            "vertical_launch_angle_degrees": 12.0,
            "horizontal_launch_angle_degrees": 1.0,
            "total_spin_rpm": 2800.0,
            "spin_axis_degrees": 4.0
        }"#,
    )
    .expect_err("missing speed should fail");

    assert!(err.contains("ball_speed_meters_per_second"));
}
