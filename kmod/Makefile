obj-m += audiofuse8pre_ctl.o

KVER ?= $(shell uname -r)
KDIR ?= /lib/modules/$(KVER)/build
PWD  := $(shell pwd)

# Auto-detect Clang/LLVM-built kernels (e.g. CachyOS / Arch Clang+ThinLTO)
ifeq ($(shell grep -q '^CONFIG_CC_IS_CLANG=y' $(KDIR)/.config 2>/dev/null && echo 1),1)
LLVM_FLAGS := LLVM=1
else
LLVM_FLAGS :=
endif

all:
	$(MAKE) -C $(KDIR) M=$(PWD) $(LLVM_FLAGS) modules

clean:
	$(MAKE) -C $(KDIR) M=$(PWD) $(LLVM_FLAGS) clean

install: all
	$(MAKE) -C $(KDIR) M=$(PWD) $(LLVM_FLAGS) modules_install
	depmod -a $(KVER)

.PHONY: all clean install
