mod app;
mod archive;
mod attention;
mod cli;
mod dock;
mod icons;
mod lifecycle;
mod localization;
mod login;
mod model;
mod monitor;
mod render;
mod storage;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        std::process::exit(objc2::rc::autoreleasepool(|_| cli::run(&args)));
    }
    app::run();
}
