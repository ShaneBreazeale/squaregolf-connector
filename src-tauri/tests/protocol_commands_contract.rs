use squaregolf_connector::core::protocol::commands::*;

#[test]
fn heartbeat_command_matches_go_connector() {
    assert_eq!(heartbeat_command(0), "1183000000000000");
    assert_eq!(heartbeat_command(15), "11830f0000000000");
    assert_eq!(heartbeat_command(255), "1183ff0000000000");
}

#[test]
fn detect_ball_command_matches_go_connector() {
    assert_eq!(
        detect_ball_command(0, DetectBallMode::Deactivate, SpinMode::Standard),
        "118100001000000000"
    );
    assert_eq!(
        detect_ball_command(5, DetectBallMode::Activate, SpinMode::Standard),
        "118105011000000000"
    );
    assert_eq!(
        detect_ball_command(10, DetectBallMode::Deactivate, SpinMode::Advanced),
        "11810a001100000000"
    );
    assert_eq!(
        detect_ball_command(255, DetectBallMode::Activate, SpinMode::Advanced),
        "1181ff011100000000"
    );
}

#[test]
fn club_command_matches_go_connector() {
    assert_eq!(
        club_command(0, ClubType::PUTTER, Handedness::Right),
        "118200010700000000"
    );
    assert_eq!(
        club_command(5, ClubType::DRIVER, Handedness::Left),
        "118205020401000000"
    );
    assert_eq!(
        club_command(10, ClubType::IRON_7, Handedness::Right),
        "11820a070600000000"
    );
    assert_eq!(
        club_command(255, ClubType::SAND_WEDGE, Handedness::Left),
        "1182ff0c0601000000"
    );
    assert_eq!(
        club_command(255, ClubType::ALIGNMENT_STICK, Handedness::Right),
        "1182ff000800000000"
    );
}

#[test]
fn swing_stick_command_matches_go_connector() {
    assert_eq!(
        swing_stick_command(0, ClubType::PUTTER, Handedness::Right),
        "1182000103000000"
    );
    assert_eq!(
        swing_stick_command(5, ClubType::DRIVER, Handedness::Left),
        "1182050202010000"
    );
    assert_eq!(
        swing_stick_command(10, ClubType::IRON_7, Handedness::Right),
        "11820a0700000000"
    );
    assert_eq!(
        swing_stick_command(255, ClubType::SAND_WEDGE, Handedness::Left),
        "1182ff0c00010000"
    );
}

#[test]
fn omni_commands_match_go_connector() {
    assert_eq!(omni_set_units_command(0, 0, 0), "1188000000000000");
    assert_eq!(omni_set_units_command(1, 0, 1), "1188010001010000");
    assert_eq!(omni_set_units_command(255, 1, 2), "1188ff0101020000");

    assert_eq!(omni_set_green_speed_command(0, 0), "1189000000000000");
    assert_eq!(omni_set_green_speed_command(2, 2), "1189020200000000");
    assert_eq!(omni_set_green_speed_command(255, 5), "1189ff0500000000");

    assert_eq!(
        omni_set_carry_distance_adjustment_command(0, -5),
        "118a005f00000000"
    );
    assert_eq!(
        omni_set_carry_distance_adjustment_command(1, 0),
        "118a016400000000"
    );
    assert_eq!(
        omni_set_carry_distance_adjustment_command(255, 7),
        "118aff6b00000000"
    );
}

#[test]
fn support_commands_match_go_connector() {
    assert_eq!(request_club_metrics_command(0), "118700000000000000");
    assert_eq!(request_club_metrics_command(15), "11870f000000000000");
    assert_eq!(request_club_metrics_command(255), "1187ff000000000000");

    assert_eq!(get_os_version_command(0), "1192000000000000");
    assert_eq!(get_charge_command(1), "1186010000000000");
    assert_eq!(
        omni_set_handed_command(2, Handedness::Left),
        "118202006301000000"
    );
}
