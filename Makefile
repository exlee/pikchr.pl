test-quiet:
	RUSTFLAGS="-A warnings" cargo test #-- --nocapture
test:
	cargo test -- --nocapture

test-watch:
	fd -e rs -e pl | entr -r -c make test
test-quiet-watch:
	fd -e rs -e pl | entr -r -c make test-quiet

snapshot:
	$(if $(MSG),,$(error MSG not set))
	cd crates/pikchr-pro && jj file list ./ | tar cvf - -T - | tar xvf - -C ../../../pikchr-pro
	cd ../pikchr-pro; jj commit -m "$(MSG)"
