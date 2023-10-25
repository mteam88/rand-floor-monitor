set dotenv-load

start:
    cargo run --release >> out.log 2>> out.log &

dev:
    cargo run >> out.log 2>> out.log