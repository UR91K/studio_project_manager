//! Supervision tests for the out-of-process plugin scanner.
//!
//! These drive `Spawner` against `examples/stub_vst_worker.rs` rather than the real
//! `vst-meta` binary. The behaviour that matters here -- surviving a plugin that
//! crashes or hangs the scanner, and attributing the failure to the right path --
//! cannot be triggered on demand with real plugins, and a machine where it *can* be
//! triggered is precisely the machine where nobody wants to discover it works.

use std::path::PathBuf;
use std::time::Duration;

use seula::scan::plugins::Spawner;
use vst_meta::protocol::{ErrorType, Outcome};

/// Locate the stub worker that `cargo test` builds alongside these tests.
fn stub_worker() -> PathBuf {
    let name = if cfg!(windows) {
        "stub_vst_worker.exe"
    } else {
        "stub_vst_worker"
    };

    // Examples land in target/<profile>/examples/. Where the test binary sits relative to
    // that depends on the cargo version: target/<profile>/deps/ for a long time, then
    // target/<profile>/build/<package>/<hash>/out/ with the newer build-dir layout. So
    // look in `examples/` beside each ancestor of the test binary, nearest first.
    let exe = std::env::current_exe().expect("test executable path");
    let found = exe
        .ancestors()
        .skip(1)
        .map(|dir| dir.join("examples").join(name))
        .find(|path| path.is_file());

    // `cargo test` builds examples; `cargo test --test plugin_scanner_tests` does not.
    found.unwrap_or_else(|| {
        panic!(
            "stub worker not found in an `examples/` directory above {}.\nRun `cargo build \
             --example stub_vst_worker` first, or use plain `cargo test`, which builds examples.",
            exe.display()
        )
    })
}

fn spawner(timeout_secs: u64) -> Spawner {
    Spawner::with_worker(stub_worker(), Duration::from_secs(timeout_secs))
}

fn paths(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn scans_a_clean_batch_in_one_pass() {
    let candidates = paths(&["a.vst3", "b.vst3", "c.vst3"]);
    let report = spawner(30).scan(&candidates);

    assert_eq!(report.succeeded(), 3);
    assert_eq!(report.failed(), 0);
    assert_eq!(report.restarts, 0, "a healthy batch needs no restarts");
    assert!(!report.budget_exhausted);
}

#[test]
fn a_crash_is_blamed_on_the_plugin_that_caused_it() {
    let candidates = paths(&["before.vst3", "CRASH.vst3", "after.vst3"]);
    let report = spawner(30).scan(&candidates);

    assert_eq!(report.results.len(), 3, "every candidate is accounted for");

    // The plugin before the crash still reports its result.
    assert!(matches!(report.results[0].outcome, Outcome::Success { .. }));

    // The crash is attributed to the exact path that was in flight -- this is what
    // the worker's `begin` line buys us.
    assert_eq!(report.results[1].path, PathBuf::from("CRASH.vst3"));
    assert_eq!(report.results[1].error_type(), Some(ErrorType::Crashed));

    // And the scan resumes past it rather than losing the rest of the batch.
    assert_eq!(report.results[2].path, PathBuf::from("after.vst3"));
    assert!(matches!(report.results[2].outcome, Outcome::Success { .. }));

    assert_eq!(report.restarts, 1);
}

/// Regression test for `plugin-scan-hangs-after-last-plugin`: a worker that reports
/// everything and then never exits. Before ADR-0082 the supervisor waited on it forever,
/// so this runs the scan on a thread and fails rather than hanging the test run.
#[test]
fn a_worker_that_never_exits_after_done_is_killed_and_the_scan_completes() {
    let candidates = paths(&["a.vst3", "LINGER.vst3", "c.vst3"]);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(spawner(30).scan(&candidates));
    });

    let report = rx
        .recv_timeout(Duration::from_secs(20))
        .expect("the scan must not wait forever on a worker that will not exit");

    assert_eq!(report.succeeded(), 3, "every result arrived before the worker stuck");
    assert_eq!(report.failed(), 0);
    assert_eq!(report.restarts, 0, "a stuck exit is not a plugin failure");
}

#[test]
fn a_hang_is_broken_by_the_timeout_and_the_scan_continues() {
    let candidates = paths(&["before.vst3", "HANG.vst3", "after.vst3"]);
    let report = spawner(1).scan(&candidates);

    assert_eq!(report.results.len(), 3);
    assert_eq!(report.results[1].path, PathBuf::from("HANG.vst3"));
    assert_eq!(
        report.results[1].error_type(),
        Some(ErrorType::Timeout),
        "a hung plugin must be killed, not waited on"
    );
    assert!(matches!(report.results[2].outcome, Outcome::Success { .. }));
}

#[test]
fn several_bad_plugins_in_one_batch_are_each_isolated() {
    let candidates = paths(&[
        "ok1.vst3",
        "CRASH-a.vst3",
        "ok2.vst3",
        "CRASH-b.vst3",
        "ok3.vst3",
    ]);
    let report = spawner(30).scan(&candidates);

    assert_eq!(report.results.len(), 5);
    assert_eq!(report.succeeded(), 3);
    assert_eq!(report.failed(), 2);
    assert_eq!(report.restarts, 2);

    let failed: Vec<&PathBuf> = report
        .results
        .iter()
        .filter(|r| r.error_type().is_some())
        .map(|r| &r.path)
        .collect();
    assert_eq!(
        failed,
        vec![
            &PathBuf::from("CRASH-a.vst3"),
            &PathBuf::from("CRASH-b.vst3")
        ]
    );
}

#[test]
fn a_worker_that_dies_before_announcing_anything_does_not_end_the_scan() {
    // Without attributing this to the next candidate, the supervisor would read the
    // silent exit as a clean finish and drop everything after it.
    let candidates = paths(&["SILENT.vst3", "after.vst3"]);
    let report = spawner(30).scan(&candidates);

    assert_eq!(report.results.len(), 2);
    assert_eq!(report.results[0].path, PathBuf::from("SILENT.vst3"));
    assert_eq!(report.results[0].error_type(), Some(ErrorType::Crashed));
    assert!(matches!(report.results[1].outcome, Outcome::Success { .. }));
}

#[test]
fn the_restart_budget_bounds_a_pathological_library() {
    // Every candidate kills the worker. The scan must still terminate.
    let candidates = paths(&["CRASH-1.vst3", "CRASH-2.vst3", "CRASH-3.vst3"]);
    let report = spawner(30).scan(&candidates);

    assert!(report.results.iter().all(|r| r.error_type().is_some()));
    assert_eq!(report.results.len(), 3);
    assert!(!report.budget_exhausted, "3 restarts is within the budget");
}

#[test]
fn an_empty_candidate_list_is_not_an_error() {
    let report = spawner(30).scan(&[]);

    assert_eq!(report.results.len(), 0);
    assert_eq!(report.restarts, 0);
}

#[test]
fn shell_expansion_counts_records_not_just_paths() {
    let candidates = paths(&["a.vst3", "b.vst3"]);
    let report = spawner(30).scan(&candidates);

    // The stub returns one record per path; the distinction matters because a real
    // VST2 shell returns many.
    assert_eq!(report.plugin_count(), 2);
    assert_eq!(report.succeeded(), 2);
}

#[test]
fn progress_is_reported_once_per_plugin_before_it_is_attempted() {
    let candidates = paths(&["a.vst3", "b.vst3", "c.vst3"]);

    let mut seen: Vec<(usize, usize, String)> = Vec::new();
    spawner(30).scan_with_progress(&candidates, &mut |index, total, path| {
        seen.push((index, total, path.display().to_string()));
    });

    assert_eq!(
        seen,
        vec![
            (0, 3, "a.vst3".to_string()),
            (1, 3, "b.vst3".to_string()),
            (2, 3, "c.vst3".to_string()),
        ]
    );
}

#[test]
fn progress_names_the_plugin_that_kills_the_worker() {
    // The reason progress fires on `begin` rather than on a result: the plugin a user
    // most needs named is the one that never produces one.
    let candidates = paths(&["before.vst3", "CRASH.vst3", "after.vst3"]);

    let mut seen: Vec<String> = Vec::new();
    spawner(30).scan_with_progress(&candidates, &mut |_, _, path| {
        seen.push(path.display().to_string());
    });

    assert!(
        seen.contains(&"CRASH.vst3".to_string()),
        "the crashing plugin should have been announced: {:?}",
        seen
    );
}

#[test]
fn progress_indices_stay_absolute_across_a_restart() {
    // Each restart hands the supervisor a fresh batch starting at the resume point.
    // Reporting batch-relative indices would make the counter jump backwards.
    let candidates = paths(&["ok1.vst3", "CRASH.vst3", "ok2.vst3", "ok3.vst3"]);

    let mut indices: Vec<usize> = Vec::new();
    spawner(30).scan_with_progress(&candidates, &mut |index, total, _| {
        assert_eq!(total, 4, "total is the whole scan, not the current batch");
        indices.push(index);
    });

    assert_eq!(indices, vec![0, 1, 2, 3]);
}
