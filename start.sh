until cargo run --release >> out.log 2>> out.log; do
    echo "Server crashed with exit code $?.  Respawning.." >&2
    sleep 1
done