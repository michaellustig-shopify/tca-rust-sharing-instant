use instant_admin::ephemeral::create_ephemeral_app;
use sharing_instant::auth::AuthCoordinator;
use std::io::{self, Write};

fn prompt(msg: &str) -> String {
    print!("{msg}");
    io::stdout().flush().expect("flush");
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).expect("read_line");
    buf.trim().to_string()
}

#[tokio::main]
async fn main() {
    println!("=== sharing-instant auth demo ===\n");

    print!("Creating ephemeral InstantDB app... ");
    io::stdout().flush().expect("flush");
    let app = create_ephemeral_app("auth-demo")
        .await
        .expect("failed to create ephemeral app");
    println!("done ({})\n", &app.id[..8]);

    let auth = AuthCoordinator::new(&app.id);

    let email = prompt("Enter your email: ");
    println!("\nSending magic code to {email}...");
    auth.send_magic_code(&email)
        .await
        .expect("send_magic_code failed");
    println!("Check your inbox!\n");

    let code = prompt("Enter the 6-digit code: ");

    println!("\nVerifying...");
    let user = auth
        .verify_magic_code(&email, &code)
        .await
        .expect("verify_magic_code failed");

    println!("\n=== Signed in! ===");
    println!("  User ID: {}", user.id);
    println!("  Email:   {:?}", user.email);
    println!("  State:   {:?}", *auth.state().get());
    println!();

    let _ = prompt("Press Enter to sign out...");
    auth.sign_out().await.expect("sign_out");
    println!("Signed out. State: {:?}", *auth.state().get());
}
