debug: setup_dev
	cargo build
	cp target/debug/libmuzzman_module_http.so libhttp.so

release: setup_dev
	cargo build --release
	cp target/release/libmuzzman_module_http.so libhttp.so

install: release
	cp ./libhttp.so ~/.local/share/MuzzMan/modules/

clean:
	cargo clean

check:
	cargo check

setup_dev:
