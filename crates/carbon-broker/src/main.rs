#[cfg(target_os = "android")]
fn main() {
    match carbon_broker::run_android_broker_service() {
        Ok(()) => {}
        Err(error) => {
            eprintln!("carbonhvd: broker service failed: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(not(target_os = "android"))]
fn main() {
    eprintln!("carbonhvd is an Android platform daemon and cannot run on this host");
    std::process::exit(78);
}
