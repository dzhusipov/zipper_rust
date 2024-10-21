all: test build

fmt:
	cargo fmt --all -- --check

clippy:
	cargo clippy --all -- -D warnings

build-test: fmt clippy
	cargo clean
	cargo test
	cargo build
	cargo build --release
	cargo build --release --target x86_64-unknown-linux-gnu
	cargo build --release --target x86_64-unknown-linux-musl
	cargo build --release --target x86_64-pc-windows-gnu
	ls -lh target/debug
	ls -lh target/release
	ls -lh target/x86_64-unknown-linux-gnu/release
	ls -lh target/x86_64-unknown-linux-musl/release
	ls -lh target/x86_64-pc-windows-gnu/release

build:
	cargo clean
	cargo test
	cargo build
	cargo build --release
	cargo build --release --target x86_64-unknown-linux-gnu
	cargo build --release --target x86_64-unknown-linux-musl
	cargo build --release --target x86_64-pc-windows-gnu
	ls -lh target/debug
	ls -lh target/release
	ls -lh target/x86_64-unknown-linux-gnu/release
	ls -lh target/x86_64-unknown-linux-musl/release
	ls -lh target/x86_64-pc-windows-gnu/release

windows:
	cargo clean
	cargo test
	cargo build --release --target x86_64-pc-windows-gnu
	ls -lh target/x86_64-pc-windows-gnu/release

linux:
	cargo clean
	cargo test
	cargo build --release --target x86_64-unknown-linux-gnu
	ls -lh target/x86_64-unknown-linux-gnu/release

linux-musl:
	cargo clean
	cargo test
	cargo build --release --target x86_64-unknown-linux-musl
	ls -lh target/x86_64-unknown-linux-musl/release

test: build
	cargo test --all -- --nocapture

watch: build
	cargo watch -x 'test --all -- --nocapture'

run-benchmark:
	cargo run --release -p benchmark

docker:
	docker stop zipper || true
	docker rm zipper || true
	docker rmi zipper || true
	docker build -t zipper -f Dockerfile .
	docker run -d -p 8119:8119 --name zipper zipper

help:
	cat Makefile