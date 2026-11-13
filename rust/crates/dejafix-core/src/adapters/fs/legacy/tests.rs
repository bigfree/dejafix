use std::fs;
use std::path::{Path, PathBuf};

use super::{
    clear_destination, files_with_legacy_paths, holds_data, is_symlink, live_watcher, move_entry,
    read_universal,
};

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

#[test]
fn the_walk_lists_text_files_that_record_legacy_paths() {
    let temp = tempfile::tempdir().unwrap();
    let memory = temp.path().join(".projectmem");
    write(
        &memory.join("events.jsonl"),
        "{\"location\": \".projectmem/summary.md\"}\n",
    );
    write(
        &memory.join("issues/0001-a.md"),
        "see .projectmem/plan.md\n",
    );
    write(&memory.join("AI_INSTRUCTIONS.md.bak"), ".projectmem\n");
    write(&memory.join("config.toml"), "path = \".projectmem\"\n");
    write(&memory.join("structure.json"), "{\".projectmem\": 1}\n");
    write(&memory.join("summary.md"), "nothing legacy\n");
    write(&memory.join("notes.txt"), ".projectmem\n");
    write(&memory.join("word.md"), ".projectmems\n");
    fs::write(memory.join("binary.md"), b"\xff.projectmem").unwrap();
    assert_eq!(
        files_with_legacy_paths(&memory),
        [
            "AI_INSTRUCTIONS.md.bak",
            "config.toml",
            "events.jsonl",
            "issues/0001-a.md",
            "structure.json",
        ]
        .map(PathBuf::from)
    );
}

#[cfg(unix)]
#[test]
fn the_walk_never_follows_a_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let outside = temp.path().join("outside");
    write(&outside.join("secret.md"), ".projectmem\n");
    let memory = temp.path().join(".projectmem");
    fs::create_dir_all(&memory).unwrap();
    std::os::unix::fs::symlink(&outside, memory.join("linked")).unwrap();
    std::os::unix::fs::symlink(outside.join("secret.md"), memory.join("file.md")).unwrap();
    assert_eq!(files_with_legacy_paths(&memory), Vec::<PathBuf>::new());
    assert!(is_symlink(&memory.join("linked")));
    assert!(!is_symlink(&memory));
}

#[test]
fn universal_newlines_are_read_and_invalid_text_is_none() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("a.md");
    fs::write(&path, "a\r\nb\rc\n").unwrap();
    assert_eq!(read_universal(&path).unwrap(), "a\nb\nc\n");
    fs::write(&path, b"\xff").unwrap();
    assert_eq!(read_universal(&path), None);
    assert_eq!(read_universal(&temp.path().join("missing")), None);
}

#[test]
fn data_is_any_file_below_a_folder() {
    let temp = tempfile::tempdir().unwrap();
    let empty = temp.path().join("empty");
    fs::create_dir_all(empty.join("a/b")).unwrap();
    assert!(!holds_data(&empty));
    assert!(!holds_data(&temp.path().join("missing")));
    write(&empty.join("a/b/file"), "x");
    assert!(holds_data(&empty));
    assert!(holds_data(&empty.join("a/b/file")));
}

#[cfg(unix)]
#[test]
fn a_symlink_counts_as_data_and_is_never_followed() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("target");
    fs::create_dir_all(&target).unwrap();
    let link = temp.path().join("link");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(holds_data(&link));
    let broken = temp.path().join("broken");
    std::os::unix::fs::symlink(temp.path().join("nowhere"), &broken).unwrap();
    assert!(holds_data(&broken));
    clear_destination(&link).unwrap();
    assert!(target.is_dir());
    assert!(!link.exists());
}

#[test]
fn entries_move_and_destinations_clear() {
    let temp = tempfile::tempdir().unwrap();
    let from = temp.path().join("from");
    write(&from.join("global/library_gotchas.jsonl"), "{}\n");
    write(&from.join("projects.json"), "[]");
    let to = temp.path().join("to");
    fs::create_dir_all(&to).unwrap();
    move_entry(&from.join("global"), &to.join("global")).unwrap();
    move_entry(&from.join("projects.json"), &to.join("projects.json")).unwrap();
    assert_eq!(
        fs::read_to_string(to.join("global/library_gotchas.jsonl")).unwrap(),
        "{}\n"
    );
    assert!(to.join("projects.json").is_file());
    assert!(!from.join("global").exists());
    fs::remove_file(to.join("global/library_gotchas.jsonl")).unwrap();
    fs::create_dir_all(to.join("global/empty/nested")).unwrap();
    clear_destination(&to.join("global")).unwrap();
    clear_destination(&to.join("projects.json")).unwrap();
    clear_destination(&to.join("missing")).unwrap();
    assert_eq!(fs::read_dir(&to).unwrap().count(), 0);
}

#[test]
fn a_live_watcher_is_found_and_anything_else_is_not() {
    let temp = tempfile::tempdir().unwrap();
    let legacy = temp.path();
    assert_eq!(live_watcher(legacy), None);
    let pid = std::process::id();
    let digits = pid.to_string();
    let grouped = format!("{}_{}", &digits[..1], &digits[1..]);
    for text in [format!("{pid}\n"), format!(" +{pid} "), grouped] {
        fs::write(legacy.join("watch.pid"), text).unwrap();
        assert_eq!(live_watcher(legacy), Some(pid));
    }
    for text in ["garbage", "0", "-1", "++5", "1__2", "4194999"] {
        fs::write(legacy.join("watch.pid"), text).unwrap();
        assert_eq!(live_watcher(legacy), None, "{text}");
    }
}

#[test]
fn clearing_a_destination_never_removes_a_file_that_appeared_in_it() {
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("global");
    write(&dest.join("late/file.jsonl"), "{}\n");
    assert!(clear_destination(&dest).is_err());
    assert!(dest.join("late/file.jsonl").is_file());
}
