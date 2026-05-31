MODE ?= release
BLK_MODE ?= virt
LOG ?= error

all:
	$(MAKE) -C GCore/os rv64-kernel-build-only MODE=$(MODE) BLK_MODE=$(BLK_MODE) LOG=$(LOG)
	cp -f GCore/kernel-rv ./kernel-rv

clean:
	$(MAKE) -C GCore/os clean
	rm -f ./kernel-rv

.PHONY: all clean
