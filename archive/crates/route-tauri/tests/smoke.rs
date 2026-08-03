use route_tauri::example_seed::seed_example;
use std::thread;
use tempfile::TempDir;

fn on_big_stack<F: FnOnce() + Send + 'static>(f: F) {
    thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(f)
        .expect("spawn big-stack thread")
        .join()
        .expect("big-stack thread should not panic")
}

#[test]
fn test_actual_seed() {
    on_big_stack(|| {
        let tmp = TempDir::new().unwrap();
        let project = tmp.path().join("example");
        eprintln!("calling seed_example");
        let r = seed_example(&project).expect("seed");
        eprintln!("after seed, branches = {}", r.list_branches().unwrap().len());
    });
}
