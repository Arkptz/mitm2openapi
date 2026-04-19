.PHONY: phase2 gif clean

SCRIPT_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

phase2:
	@echo "=== Running Phase 2 VHS recording ==="
	@mkdir -p $(SCRIPT_DIR)out
	vhs $(SCRIPT_DIR)phase2.tape
	@echo "Phase 2 complete: out/phase2.mp4 and out/phase2.gif"

gif: phase2
	@echo "=== Building final GIF (phase2 only) ==="
	$(SCRIPT_DIR)build-gif.sh --phase2-only
	@echo "GIF built: out/phase2-opt.gif"

clean:
	rm -rf $(SCRIPT_DIR)out/
