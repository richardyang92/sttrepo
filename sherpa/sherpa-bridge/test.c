#include <stdio.h>

#include "sherpa.h"

const char* SHERPA_TOKENS = "../sherpa-models/tokens.txt";
const char* SHERPA_ENCODER = "../sherpa-models/encoder-epoch-20-avg-1-chunk-16-left-128.onnx";
const char* SHERPA_DECODER = "../sherpa-models/decoder-epoch-20-avg-1-chunk-16-left-128.onnx";
const char* SHERPA_JOINER = "../sherpa-models/joiner-epoch-20-avg-1-chunk-16-left-128.onnx";

int main() {
    const char* pcm_file = "test.pcm";
    SherpaHandle handler = sherpa_init(SHERPA_TOKENS, SHERPA_ENCODER, SHERPA_DECODER, SHERPA_JOINER);

    FILE* fp = fopen(pcm_file, "rb");
    if (fp == NULL) {
        printf("can't open %s!\n", pcm_file);
        return -1;
    }

    unsigned char buff[6400];
    int read_len = 0;

    char ret[MAX_SUPPORT_TOKENS];

    do {
        read_len = fread(buff, sizeof(unsigned char), 6400, fp);
        printf("read_len: %d\n", read_len);
        if (read_len < 6400) {
            for (int j = read_len; j < 6400; j++) {
                buff[j] = 0;
            }
        }

        float sample[3200];
        for (int k = 0; k < 3200; k++) {
            sample[k] = ((int16_t) buff[2 * k + 1] << 8) | ((int16_t) buff[2 * k] & 0xff);
            sample[k] /= 32767.0;
        }

        sherpa_transcribe(handler, ret, sample, 3200);
        printf("ret: %s\n", ret);
    } while (read_len > 0);
    
    sherpa_close(handler);
    return 0;
}