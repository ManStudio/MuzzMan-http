debug: setup_dev
	cargo build
	cp target/debug/libmuzzman_module_http.so libhttp.so

release: setup_dev
	cargo build --release
	cp target/debug/libmuzzman_module_http.so libhttp.so

clean:
	cargo clean

check:
	cargo check

setup_dev:
