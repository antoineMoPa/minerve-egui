.PHONY: install uninstall
install:
	cargo install --path .
uninstall:
	sh uninstall.sh
