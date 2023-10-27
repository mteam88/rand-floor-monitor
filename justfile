set dotenv-load

start:
    watch -n 10 cargo run --release >> out.log 2>> out.log &

dev:
    cargo run >> out.log 2>> out.log