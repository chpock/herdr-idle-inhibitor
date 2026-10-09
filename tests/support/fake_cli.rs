fn main() {
    let args:Vec<_>=std::env::args().skip(1).collect();
    if args==["session","list","--json"] {
        let path=std::env::var("HERDR_CONFIG_PATH").unwrap();
        print!("{}",std::fs::read_to_string(path).unwrap());
    } else if args==["plugin","list","--plugin","herdr-idle-inhibitor","--json"] {
        let path=std::env::var("HERDR_CONFIG_PATH").unwrap()+".plugins";
        print!("{}",std::fs::read_to_string(path).unwrap());
    } else {std::process::exit(2);}
}
