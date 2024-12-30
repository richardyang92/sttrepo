#!/bin/bash

# 修正赋值语法，去掉空格，并使用命令替换来获取当前目录
ROOT=$(pwd)

# 修正环境变量的导出语法
export PKG_CONFIG_PATH=${ROOT}/sherpa/sherpa-native/shared:$PKG_CONFIG_PATH
export DYLD_LIBRARY_PATH=${ROOT}/stt-engine/lib:$DYLD_LIBRARY_PATH