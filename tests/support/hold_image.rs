//! Native file-lock fixture; never invokes power or Herdr APIs.
fn main() {
    std::fs::write(std::env::args_os().nth(1).unwrap(), b"ready").unwrap();
    loop { std::thread::sleep(std::time::Duration::from_secs(1)); }
}
