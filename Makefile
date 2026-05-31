MODE ?= release
BLK_MODE ?= virt
LOG ?= error

all:
	$(MAKE) -C GCore/os rv64-kernel-build-only MODE=$(MODE) BLK_MODE=$(BLK_MODE) LOG=$(LOG)

clean:
	$(MAKE) -C GCore/os clean

.PHONY: all clean
