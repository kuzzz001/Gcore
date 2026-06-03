MODE ?= release
BLK_MODE ?= virt
LOG ?= error

all: rv64 la64

rv64:
	$(MAKE) -C GCore/os rv64-kernel-build-only MODE=$(MODE) BLK_MODE=$(BLK_MODE) LOG=$(LOG)
	cp -f GCore/kernel-rv ./kernel-rv

la64:
	$(MAKE) -C GCore/os la64-kernel-build-only MODE=$(MODE) BLK_MODE=$(BLK_MODE) LOG=$(LOG); \
	if [ -f GCore/kernel-la ]; then cp -f GCore/kernel-la ./kernel-la; fi

clean:
	$(MAKE) -C GCore/os clean
	rm -f ./kernel-rv ./kernel-la

.PHONY: all rv64 la64 clean
