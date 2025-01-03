#include "server.h"
#include <arpa/inet.h>
#include <cstring>
#include <iostream>
#include <event2/util.h>

static int gConnectionSerialNo = 0;

// 实现SherpaHandleWrapper中的函数
server::sherpa::SherpaHandleWrapper::SherpaHandleWrapper(std::shared_ptr<SherpaHandle> handle, std::shared_ptr<std::atomic<bool>> inUse)
    : handle(handle), inUse(inUse) {}

server::sherpa::SherpaHandleWrapper::~SherpaHandleWrapper() {
    // 在析构函数中，将inUse设置为false，表示当前handle不再被使用
    setInUse(false);
}

SherpaHandle server::sherpa::SherpaHandleWrapper::getHandle() const {
    return *handle;
}

void server::sherpa::SherpaHandleWrapper::setInUse(bool value) const {
    *inUse = value;
}

bool server::sherpa::SherpaHandleWrapper::isInUse() const {
    return *inUse;
}

// 实现SherpaPool中的函数
server::sherpa::SherpaPool::SherpaPool(int total_size) : total_size(total_size) {
    const char* SHERPA_TOKENS = "../sherpa-models/tokens.txt";
    const char* SHERPA_ENCODER = "../sherpa-models/encoder-epoch-20-avg-1-chunk-16-left-128.onnx";
    const char* SHERPA_DECODER = "../sherpa-models/decoder-epoch-20-avg-1-chunk-16-left-128.onnx";
    const char* SHERPA_JOINER = "../sherpa-models/joiner-epoch-20-avg-1-chunk-16-left-128.onnx";
    
    for (int i = 0; i < total_size; ++i) {
        SherpaHandle handle_ = sherpa_init(SHERPA_TOKENS, SHERPA_ENCODER, SHERPA_DECODER, SHERPA_JOINER);
        auto handleWrapper = server::sherpa::SherpaHandleWrapper(std::make_shared<SherpaHandle>(handle_), std::make_shared<std::atomic<bool>>(false));
        this->handles.push_back(handleWrapper);
    }
    printf("Initialized %d sherpa handles\n", total_size);
}

server::sherpa::SherpaPool::~SherpaPool() {
    // 在析构函数中，释放所有handle的资源
    for (auto &wrapper : handles) {
        sherpa_close(wrapper.getHandle());
    }
    printf("Closed %d sherpa handles\n", total_size);
}

server::sherpa::SherpaHandleWrapper *server::sherpa::SherpaPool::selectHandle() {
    for (auto &wrapper : handles) {
        if (!wrapper.isInUse()) {
            wrapper.setInUse(true);
            return &wrapper;
        }
    }
    return nullptr;
}

void server::sherpa::SherpaPool::releaseHandle(server::sherpa::SherpaHandleWrapper *handle) {
    handle->setInUse(false);
}

server::Server::Server(int port, AcceptCallback acceptCb, ErrorCallback errorCb)
    : port_(port), acceptCb_(std::move(acceptCb)), errorCb_(std::move(errorCb)) {
    base_ = event_base_new();
    if (!base_) {
        std::cerr << "Failed to create event base" << std::endl;
        exit(1);
    }

    struct sockaddr_in sin;
    memset(&sin, 0, sizeof(sin));
    sin.sin_family = AF_INET;
    sin.sin_addr.s_addr = htonl(INADDR_ANY);
    sin.sin_port = htons(port_);

    listener_ = evconnlistener_new_bind(base_,
        [](struct evconnlistener *listener, evutil_socket_t fd, struct sockaddr *address, int socklen, void *ctx) {
            auto server = static_cast<Server*>(ctx);
            server->handleAccept(listener, fd, address, socklen);
        },
        this, LEV_OPT_CLOSE_ON_FREE | LEV_OPT_REUSEABLE, -1, (struct sockaddr*)&sin, sizeof(sin));
    if (!listener_) {
        std::cerr << "Failed to create listener" << std::endl;
        event_base_free(base_);
        exit(1);
    }

    evconnlistener_set_error_cb(listener_, [](struct evconnlistener *listener, void *ctx) {
        auto server = static_cast<Server*>(ctx);
        server->handleError(listener);
    });
}

server::Server::~Server() {
    event_base_free(base_);
}

void server::Server::start() {
    event_base_dispatch(base_);
}

void server::Server::handleAccept(struct evconnlistener *listener, evutil_socket_t fd, struct sockaddr *address, int socklen) {
    if (acceptCb_) {
        gConnectionSerialNo++;
        // 如果gConnectionSerialNo超过int型最大值，则重置为0
        if (gConnectionSerialNo < 0) {
            gConnectionSerialNo = 0;
        }
        server::Connection *connection = new server::Connection(gConnectionSerialNo, nullptr);

        acceptCb_(listener, fd, address, socklen, connection);
    }
}

void server::Server::handleError(struct evconnlistener *listener) {
    struct event_base *base = evconnlistener_get_base(listener);

    if (errorCb_) {
        errorCb_(listener, nullptr);
    }

    printf("Error occurred in the server\n");
    event_base_loopexit(base, nullptr);
}

server::ServerBuilder& server::ServerBuilder::setPort(int port) {
    port_ = port;
    return *this;
}

server::ServerBuilder& server::ServerBuilder::setAcceptCallback(const std::function<void(struct evconnlistener*, evutil_socket_t, struct sockaddr*, int, void*)>& acceptCb) {
    acceptCb_ = acceptCb;
    return *this;
}

server::ServerBuilder& server::ServerBuilder::setErrorCallback(const std::function<void(struct evconnlistener*, void*)>& errorCb) {
    errorCb_ = errorCb;
    return *this;
}

std::unique_ptr<server::Server> server::ServerBuilder::build() {
    return std::make_unique<Server>(port_, acceptCb_, errorCb_);
}