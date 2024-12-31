GCC = gcc
RM = rm -rf
OS := $(shell uname)
ifeq ($(OS),Darwin)
    LIB = libsherpa-bridge.dylib
	CFLAGS = -dynamiclib
else
    LIB = libsherpa-bridge.so # 默认设置为 Linux 的库文件扩展名
	CFLAGS = -shared
endif
TEST = sherpa_test

all: clean build install

# 根据操作系统选择构建指令
build:
	$(GCC) $(CFLAGS) -o $(LIB) $(shell pkg-config --cflags sherpa-onnx) sherpa.c $(shell pkg-config --libs sherpa-onnx)

clean:
	$(RM) $(LIB) ../../stt-engine/lib/$(LIB) $(TEST)

install:
	cp $(LIB) ../../stt-engine/lib/$(LIB)

test:
	$(GCC) -o $(TEST) $(shell pkg-config --cflags sherpa-onnx) \
	test.c sherpa.c $(shell pkg-config --libs sherpa-onnx)
