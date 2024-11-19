mod api;
mod caddy;
mod certs;
mod config;
mod dns;
mod error;
mod feed;
mod jetstream;
mod secrets;

fn main() {
    tracing_subscriber::fmt::init();
}
