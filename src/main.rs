mod api;
mod certs;
mod config;
mod dns;
mod error;
mod secrets;

fn main() {
    tracing_subscriber::fmt::init();
}
