#!/bin/bash

# 检查输入参数
if [ "$#" -ne 1 ]; then
    echo "Usage: $0 <input_wav_file>"
    exit 1
fi

# 输入文件
INPUT_WAV=$1

# 持续时间（秒）
DURATION=18

# 文件名前缀
OUTPUT_PREFIX="segment/split_"

# 检查ffmpeg是否已安装
if ! command -v ffmpeg &> /dev/null; then
    echo "ffmpeg could not be found. Please install ffmpeg first."
    exit 1
fi

# 获取音频文件的总长度（秒
LENGTH=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$INPUT_WAV")

# 转换为整数
LENGTH=${LENGTH%.*}

# 检查音频长度是否足够长
if [ "$LENGTH" -lt "$((100 * DURATION))" ]; then
    echo "The audio file is not long enough to be split into 100 parts of $DURATION seconds each."
    exit 1
fi

# 切分音频
for i in $(seq 1 100); do
    START=$(( (i - 1) * DURATION ))
    ffmpeg -i "$INPUT_WAV" -ss "$START" -t "$DURATION" -ar 16000 -ac 1 "${OUTPUT_PREFIX}part_${i}.wav"
done

echo "Split completed!"