use raknet::{RakNetListener, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for logs
    tracing_subscriber::fmt::init();

    // Create listener on standard Minecraft Bedrock port
    let mut listener = RakNetListener::bind("0.0.0.0:19132").await?;

    // Set custom server information
    listener.set_pong_data(
        "MCPE;Rust RakNet Server;568;1.20.0;0;10;13253860892328930865;Bedrock level;Survival;1;19132;19133;"
    );

    println!("RakNet server listening on {}", listener.local_addr()?);
    println!("Waiting for connections...");
    println!();
    println!("To test:");
    println!("  1. Open Minecraft Bedrock Edition");
    println!("  2. Go to 'Friends' tab");
    println!("  3. Look for 'Rust RakNet Server' in the LAN games section");
    println!();

    // Run the listener loop
    listener.run().await?;

    Ok(())
}
