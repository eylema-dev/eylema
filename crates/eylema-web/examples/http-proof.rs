#[path = "../tests/http.rs"]
mod proof;
fn main() {
    proof::run_native_proof();
    println!(
        "PASS: owned native HTTP/durable task, Origin/Host/session/CSRF/scoped authority, malformed/size/deadline/expiry/disconnect/cancellation/configuration boundaries"
    );
    println!(
        "NOT_RUN: real person authentication, browser integration, remote TLS, GPU/model/training, physical power-loss"
    );
}
