mod api;
mod caddy;
mod certs;
mod compose;
mod config;
mod dns;
mod docker;
mod error;
mod feed;
mod health;
mod jetstream;
mod ozone;
mod secrets;
mod status;

fn main() {
    tracing_subscriber::fmt::init();
}
