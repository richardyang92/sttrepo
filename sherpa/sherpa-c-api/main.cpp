#include <stdio.h>
#include "server.h"
#include <unistd.h>
#include <signal.h>

#define SHERPA_POOL_SIZE 5

static server::sherpa::SherpaPool gSherpaPool = server::sherpa::SherpaPool(SHERPA_POOL_SIZE);

static void
sherpa_read_cb(struct bufferevent *bev, void *ctx) {
    /* This callback is invoked when there is data to read on bev. */
    struct evbuffer *input = bufferevent_get_input(bev);
    struct evbuffer *output = bufferevent_get_output(bev);

    if (input == nullptr || output == nullptr) {
        printf("sherpa_read_cb: no input or output buffer\n");
        return;
    }

    size_t len = evbuffer_get_length(input);

    /* Copy all the data from the input buffer to the output buffer. */
    // printf("echo_read_cb: %zu\n", len);
    // evbuffer_add_buffer(output, input);
    // 获取输入缓冲区中的数据
    auto connection = static_cast<server::Connection*>(ctx);
    if (connection == nullptr) {
        printf("sherpa_read_cb: no context\n");
        // evbuffer_drain(input, len);
        return;
    }
    auto handler = connection->sherpaWrapper;
    if (len == 0 || len % 2 != 0) {
        // printf("receive from clientId:%d, data length %lu is not valid\n", connection->connectionId, len);
        return;
    } else {
        // printf("receive from clientId:%d, data length is %lu\n", connection->connectionId, len);
        // 使用 std::unique_ptr 管理动态分配的数组
        std::unique_ptr<unsigned char[]> buff(new unsigned char[len]);
        std::unique_ptr<char[]> result(new char[MAX_SUPPORT_TOKENS + 1]);
        float* samples = connection->samples.get();

        evbuffer_remove(input, buff.get(), len);

        // 处理数据
        for (int k = 0; k < len / 2; k++) {
            int16_t value = ((int16_t)buff[2 * k + 1] << 8) | ((int16_t)buff[2 * k] & 0xff);
            samples[k] = static_cast<float>(value) / 32767.0f;
        }
        
        sherpa_transcribe(handler->getHandle(), result.get(), samples, len / 2);
        // samples使用完后要重置
        for (int k = 0; k < len / 2; k++) {
            samples[k] = 0.0f;
        }
        // 注意：这里修改了 result 指向的内存，确保不会超出分配的范围
        strcat(result.get(), "\n");
        printf("sherpa_read_cb ClientId(%d): %s\n", connection->connectionId, result.get());
        evbuffer_add(output, result.get(), strlen(result.get()));
    }
}

static void
sherpa_event_cb(struct bufferevent *bev, short events, void *ctx) {
    bool reading = (events & BEV_EVENT_READING) != 0;
    bool writing = (events & BEV_EVENT_WRITING) != 0;
    bool eof = (events & BEV_EVENT_EOF) != 0;
    bool error = (events & BEV_EVENT_ERROR) != 0;
    bool timeout = (events & BEV_EVENT_TIMEOUT) != 0;
    bool connected = (events & BEV_EVENT_CONNECTED) != 0;
    printf("Reading: %d, Writing: %d, EOF: %d, Error: %d, Timeout: %d, Connected: %d\n",
        reading, writing, eof, error, timeout, connected);
    if (eof || error || timeout) {
        auto connection = static_cast<server::Connection*>(ctx);
        if (connection != nullptr) {
            gSherpaPool.releaseHandle(connection->sherpaWrapper);
            delete connection;
        }
        bufferevent_free(bev);
    }
}

static void
accept_conn_cb(struct evconnlistener *listener,
    evutil_socket_t fd, struct sockaddr *address, int socklen, void *ctx) {
    auto connection = static_cast<server::Connection*>(ctx);
    if (connection == nullptr) {
        printf("accept_conn_cb: no context\n");
        return;
    }
    
    // printf("Trying to select a sherpa handler\n");
    server::sherpa::SherpaHandleWrapper* handler = nullptr;
    handler = gSherpaPool.selectHandle();

    if (handler == nullptr) {
        // printf("No available sherpa handler for ClientId:%d\n", connection->connectionId);
        // 关闭连接
        if (connection != nullptr) {
            delete connection;
        }
        evutil_closesocket(fd);
        return;
    } else {
        // printf("Got a sherpa handler for ClientId:%d\n", connection->connectionId);
        connection->sherpaWrapper = handler;
        printf("accept_conn_cb, connection id=%d\n", connection->connectionId);
        struct event_base *base = evconnlistener_get_base(listener);
        printf("debug: 111\n");
        struct bufferevent *bev = bufferevent_socket_new(base, fd, BEV_OPT_CLOSE_ON_FREE);
        printf("debug: 222\n");
        bufferevent_setcb(bev, sherpa_read_cb, nullptr, sherpa_event_cb, connection);
        printf("debug: 333\n");
        bufferevent_enable(bev, EV_READ | EV_WRITE);
        printf("debug: 444\n");
    }
}

static void
accept_error_cb(struct evconnlistener *listener, void *ctx) {
    int err = EVUTIL_SOCKET_ERROR();
    printf("Got an error %d (%s) on the listener. "
        "Shutting down.\n", err, evutil_socket_error_to_string(err));
}

void handle_sigpipe() {
    // 忽略 SIGPIPE 信号
    signal(SIGPIPE, SIG_IGN);
}

int main(int argc, char **argv) {
    handle_sigpipe();
    printf("Starting server...\n");
    std::unique_ptr<server::Server> server = server::ServerBuilder().setPort(8888)
        .setAcceptCallback(accept_conn_cb)
        .setErrorCallback(accept_error_cb)
        .build();
    server->start();
    printf("Server stop...\n");
    return 0;
}