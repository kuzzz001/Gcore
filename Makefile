MODE ?= release
BLK_MODE ?= virt
LOG ?= error

all:
	$(MAKE) -C GCore/os rv64-kernel-build-only MODE=$(MODE) BLK_MODE=$(BLK_MODE) LOG=$(LOG)
	cp -f GCore/kernel-rv ./kernel-rv
	ln -sf GCore/sdcard-rv.img ./sdcard-rv.img 2>/dev/null; test -f ./sdcard-rv.img || cp -f GCore/sdcard-rv.img ./sdcard-rv.img 2>/dev/null; true

clean:
	$(MAKE) -C GCore/os clean
	rm -f ./kernel-rv ./sdcard-rv.img

.PHONY: all clean
