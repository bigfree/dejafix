use super::is_fixed_drive;

#[test]
fn the_system_drive_is_fixed() {
    let drive = std::env::var("SystemDrive").unwrap();
    let letter = drive.chars().next().unwrap();
    assert!(is_fixed_drive(letter));
}

#[test]
fn a_letter_without_a_volume_is_not_fixed() {
    assert!(!is_fixed_drive('ž'));
}
