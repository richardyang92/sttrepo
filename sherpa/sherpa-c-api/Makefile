SHERPA_DIR = $(shell pwd)/../sherpa-bridge

RM = rm -rf
BIN = sherpa_c_api

CC = gcc
CXX = g++

CFLAGS = $(shell pkg-config --cflags sherpa-onnx) \
	$(shell pkg-config --cflags libevent) \
	-I${SHERPA_DIR} \
	-Wall
CXXFLAGS = $(shell pkg-config --cflags sherpa-onnx) \
	$(shell pkg-config --cflags libevent) \
	-I${SHERPA_DIR} \
	-Wall -std=c++14
LDFLAGS = $(shell pkg-config --libs sherpa-onnx) \
	$(shell pkg-config --libs libevent)

C_SOURCES = ${SHERPA_DIR}/sherpa.c
C_OBJECTS = $(C_SOURCES:.c=.o)
CPP_SOURCES = main.cpp server.cpp
CPP_OBJECTS = $(CPP_SOURCES:.cpp=.o)
OBJECTS = $(C_OBJECTS) $(CPP_OBJECTS)

all: $(BIN) clean_obj

# 链接目标程序
$(BIN): $(OBJECTS)
	$(CXX) $(CXXFLAGS) -o $@ $^ $(LDFLAGS)

# 编译 .c 文件到 .o 文件的规则
%.o: %.c
	$(CC) $(CFLAGS) -c -o $@ $<

# 编译 .cpp 文件到 .o 文件的规则
%.o: %.cpp
	$(CXX) $(CXXFLAGS) -c -o $@ $<

clean_obj:
	$(RM) $(OBJECTS)

clean:
	$(RM) $(BIN) $(OBJECTS)