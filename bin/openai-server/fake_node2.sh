RUST_LOG=worker=info cargo run --features metal -- --node-id node2 --layers-from 10 --layers-to 16 --model fake --http-bind 0.0.0.0:5556
