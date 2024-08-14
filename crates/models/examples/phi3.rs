use models::{get_device, phi3::Phi3Model, Session};
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    let device = get_device(false).unwrap();
    let phi3 = Phi3Model::new(&device, false).await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async move {
        phi3.chat(
            Session::new(),
            &device,
            299792458,
            500,
            "Write function max(x1, x2) in Rust",
            tx,
        )
        .await
        .unwrap();
    });

    let begin = Instant::now();
    let mut count = 0;
    while let Some(text) = rx.recv().await {
        print!("{text}");
        count += 1;
    }
    println!(
        "\n{count} tokens in {:2} seconds => speed {:2}/s",
        begin.elapsed().as_secs_f32(),
        count as f32 / begin.elapsed().as_secs_f32()
    );
}
