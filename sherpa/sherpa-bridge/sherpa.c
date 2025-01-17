#include "sherpa.h"

#include <stdio.h>
#include <string.h>

#define SAMPLE_RATE 16000

SherpaHandle sherpa_init(const char* tokens, const char* encoder,
    const char* decoder, const char* joiner) {
    SherpaHandle handle;
    //初始化SherpaHandle
    memset(&handle, 0, sizeof(handle));
    if (strcmp("", tokens) == 0
      || strcmp("", encoder) == 0
      || strcmp("", decoder) == 0
      || strcmp("", joiner) == 0) {
        return handle;
    }
    SherpaOnnxOnlineRecognizerConfig config;
    memset(&config, 0, sizeof(config));

    config.model_config.debug = 0;
    config.model_config.num_threads = 1;
    config.model_config.provider = "cpu";

    config.decoding_method = "greedy_search";

    config.max_active_paths = 4;

    config.feat_config.sample_rate = 16000;
    config.feat_config.feature_dim = 80;

    config.enable_endpoint = 1;
    config.rule1_min_trailing_silence = 2.4;
    config.rule2_min_trailing_silence = 1.2;
    config.rule3_min_utterance_length = 300;

    config.model_config.tokens = tokens;
    config.model_config.transducer.encoder = encoder;
    config.model_config.transducer.decoder = decoder;
    config.model_config.transducer.joiner = joiner;

    handle.recognizer = SherpaOnnxCreateOnlineRecognizer(&config);  
    handle.stream = SherpaOnnxCreateOnlineStream(handle.recognizer);  
    return handle;
}

void sherpa_transcribe(SherpaHandle handle, char* result, float* samples, int len) {
    SherpaOnnxOnlineStreamAcceptWaveform(handle.stream, SAMPLE_RATE, samples, len);
    while (SherpaOnnxIsOnlineStreamReady(handle.recognizer, handle.stream)) {
      SherpaOnnxDecodeOnlineStream(handle.recognizer, handle.stream);
    }

    const SherpaOnnxOnlineRecognizerResult *r =
        SherpaOnnxGetOnlineStreamResult(handle.recognizer, handle.stream);
    
    memset(result, 0, MAX_SUPPORT_TOKENS);
    if (strlen(r->text)) {
      strcat(result, r->text);
    }
    
    if (SherpaOnnxOnlineStreamIsEndpoint(handle.recognizer, handle.stream)) {
      SherpaOnnxOnlineStreamReset(handle.recognizer, handle.stream);
    }

    SherpaOnnxDestroyOnlineRecognizerResult(r);
}

void sherpa_reset(SherpaHandle handle) {
    float tail_paddings[4800] = { 0 };
    SherpaOnnxOnlineStreamAcceptWaveform(handle.stream, SAMPLE_RATE, tail_paddings, 4800);
    SherpaOnnxOnlineStreamInputFinished(handle.stream);
    while (SherpaOnnxIsOnlineStreamReady(handle.recognizer, handle.stream)) {
        SherpaOnnxDecodeOnlineStream(handle.recognizer, handle.stream);
    }
    SherpaOnnxOnlineStreamReset(handle.recognizer, handle.stream);
}

void sherpa_close(SherpaHandle handle) {
    float tail_paddings[4800] = { 0 };
    SherpaOnnxOnlineStreamAcceptWaveform(handle.stream, SAMPLE_RATE, tail_paddings, 4800);
    SherpaOnnxOnlineStreamInputFinished(handle.stream);
    while (SherpaOnnxIsOnlineStreamReady(handle.recognizer, handle.stream)) {
        SherpaOnnxDecodeOnlineStream(handle.recognizer, handle.stream);
    }

    const SherpaOnnxOnlineRecognizerResult *r =
        SherpaOnnxGetOnlineStreamResult(handle.recognizer, handle.stream);
    SherpaOnnxDestroyOnlineRecognizerResult(r);

    SherpaOnnxDestroyOnlineStream(handle.stream);
    SherpaOnnxDestroyOnlineRecognizer(handle.recognizer);
}