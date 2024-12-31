#include <stdio.h>
extern "C" {
#include <sherpa.h>
}
#include "server.h"

#define SHERPA_PROXYS   1

static SherpaHandle handler[SHERPA_PROXYS];

const char* SHERPA_TOKENS = "../sherpa-models/tokens.txt";
const char* SHERPA_ENCODER = "../sherpa-models/encoder-epoch-20-avg-1-chunk-16-left-128.onnx";
const char* SHERPA_DECODER = "../sherpa-models/decoder-epoch-20-avg-1-chunk-16-left-128.onnx";
const char* SHERPA_JOINER = "../sherpa-models/joiner-epoch-20-avg-1-chunk-16-left-128.onnx";

static void
echo_read_cb(struct bufferevent *bev, void *ctx)
{
    /* This callback is invoked when there is data to read on bev. */
    struct evbuffer *input = bufferevent_get_input(bev);
    struct evbuffer *output = bufferevent_get_output(bev);

    /* Copy all the data from the input buffer to the output buffer. */
    // evbuffer_add_buffer(output, input);
    // 获取输入缓冲区中的数据
    size_t len = evbuffer_get_length(input);
    
    if (len == 0 || len % 2 != 0) {
        printf("1. data length is %lu\n", len);
        return;
    } else {
        printf("2. data length is %lu\n", len);
        unsigned char *buff = new unsigned char[len];
        float* sample = new float[len / 2];
        char* result = new char[len / 2 + 1];

        evbuffer_remove(input, buff, len);
        for (int k = 0; k < len / 2; k++) {
            sample[k] = ((int16_t) buff[2 * k + 1] << 8) | ((int16_t) buff[2 * k] & 0xff);
            sample[k] /= 32767.0;
        }
        sherpa_transcribe(handler[0], result, sample, len / 2);
        strcat(result, "\n");
        evbuffer_add(output, result, strlen(result));
        delete [] result;
        delete[] sample;
        delete[] buff;
    }
}

static void
echo_event_cb(struct bufferevent *bev, short events, void *ctx)
{
    if (events & BEV_EVENT_ERROR)
        perror("Error from bufferevent");
    if (events & (BEV_EVENT_EOF | BEV_EVENT_ERROR)) {
        bufferevent_free(bev);
    }
}

static void
accept_conn_cb(struct evconnlistener *listener,
    evutil_socket_t fd, struct sockaddr *address, int socklen,
    void *ctx)
{
    /* We got a new connection! Set up a bufferevent for it. */
    struct event_base *base = evconnlistener_get_base(listener);
    struct bufferevent *bev = bufferevent_socket_new(
        base, fd, BEV_OPT_CLOSE_ON_FREE);

    bufferevent_setcb(bev, echo_read_cb, NULL, echo_event_cb, NULL);

    bufferevent_enable(bev, EV_READ|EV_WRITE);
}

static void
accept_error_cb(struct evconnlistener *listener, void *ctx)
{
    struct event_base *base = evconnlistener_get_base(listener);
    int err = EVUTIL_SOCKET_ERROR();
    fprintf(stderr, "Got an error %d (%s) on the listener. "
        "Shutting down.\n", err, evutil_socket_error_to_string(err));

    event_base_loopexit(base, NULL);
}

int main(int argc, char **argv) {
    for (int i = 0; i < SHERPA_PROXYS; ++i) {
        handler[i] = sherpa_init(SHERPA_TOKENS, SHERPA_ENCODER,
            SHERPA_DECODER, SHERPA_JOINER);
    }
    printf("sherpa init\n");

    std::unique_ptr<EventHandler> eventHandler = std::make_unique<EventHandler>();
    eventHandler->event_cb = echo_event_cb;
    eventHandler->read_cb = echo_read_cb;
    
    std::unique_ptr<Server> server = ServerBuilder().setPort(8888)
        .setAcceptCallback(accept_conn_cb)
        .setErrorCallback(accept_error_cb)
        .setEventHandler(std::move(eventHandler))
        .build();
    server->start();
    
    for (size_t i = 0; i < SHERPA_PROXYS; i++) {
        sherpa_close(handler[i]);
    }
    printf("sherpa close\n");
    return 0;
    
}