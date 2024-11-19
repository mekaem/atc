mod api;
mod caddy;
mod certs;
mod compose;
mod config;
mod dns;
mod docker;
mod error;
mod feed;
mod jetstream;
mod ozone;
mod secrets;

fn main() {
    tracing_subscriber::fmt::init();
}
